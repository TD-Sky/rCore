use core::mem;

use alloc::sync::Arc;
use alloc::vec::Vec;
use enumflags2::BitFlags;
use log::debug;

use crate::fs;
use crate::fs::OpenFlag;
use crate::memory;
use crate::task;
use crate::task::manager;
use crate::task::processor;
use crate::task::signal::{self, SignalAction, SignalFlag};
use crate::task::TaskControlBlock;
use crate::timer;

// 切换任务，用户模式的上下文依旧是由 trap_handler 保存
pub fn sys_yield() -> isize {
    task::suspend_current_and_run_next();
    0
}

pub fn sys_get_time() -> isize {
    timer::get_time_ms() as isize
}

pub fn sys_exit(exit_code: i32) -> ! {
    println!("[kernel] Application exited with code {exit_code}");
    task::exit_current_and_run_next(exit_code);
    unreachable!()
}

/// 改变data段的大小
pub fn sys_sbrk(size: i32) -> isize {
    processor::current_task()
        .unwrap()
        .change_program_brk(size)
        .map(|old_brk| old_brk as isize)
        .unwrap_or(-1)
}

pub fn sys_mmap(start: usize, len: usize, prot: u8) -> isize {
    processor::current_task()
        .unwrap()
        .mmap(start, len, prot)
        .map(|va| va as isize)
        .unwrap_or(-1)
}

pub fn sys_munmap(start: usize, len: usize) -> isize {
    match processor::current_task().unwrap().munmap(start, len) {
        Some(_) => 0,
        None => -1,
    }
}

pub fn sys_getpid() -> isize {
    processor::current_task().unwrap().pid() as isize
}

pub fn sys_fork() -> isize {
    let current_task = processor::current_task().unwrap();
    // 此时子进程的CPU状态与父进程相同，都在 sys_fork
    let sub_task = current_task.fork();
    let new_pid = sub_task.pid();

    let trap_ctx = sub_task.inner().exclusive_access().trap_ctx();
    // 将子进程的 fork 返回值设为 0
    trap_ctx.set_syscall_result(0);

    manager::add_task(sub_task);

    new_pid as isize
}

pub fn sys_exec(path: *const u8, mut args: *const usize) -> isize {
    let token = processor::current_user_token();
    let path = memory::read_str(token, path);

    let mut arg_vec = Vec::new();
    loop {
        let arg = *memory::read_ref(token, args) as *const u8;
        debug!("token={token} arg={arg:#p}");
        if arg.is_null() {
            break;
        }
        arg_vec.push(memory::read_str(token, arg));
        unsafe {
            args = args.add(1);
        }
    }

    let Some(app) = fs::open_file(&path, OpenFlag::read_only()) else {
        return -1;
    };

    let data = app.read_all();
    let task = processor::current_task().unwrap();
    let argc = arg_vec.len();
    task.exec(&data, arg_vec);

    // 返回`argc`是因为exec里`ctx.x[10]`被设成该值，
    // 需在后续写入系统调用结果(同为`ctx.x[10]`)时与其保持一致
    argc as isize
}

pub fn sys_spawn(path: *const u8) -> isize {
    let token = processor::current_user_token();
    let path = memory::read_str(token, path);

    let Some(app) = fs::open_file(&path, BitFlags::from_bits_truncate(OpenFlag::RDONLY)) else {
        return -1;
    };

    let sub_task = Arc::new(TaskControlBlock::new(&app.read_all()));
    let sub_pid = sub_task.pid();

    let task = processor::current_task().unwrap();
    task.inner()
        .exclusive_access()
        .children
        .push(sub_task.clone());
    sub_task.inner().exclusive_access().parent = Some(Arc::downgrade(&task));

    manager::add_task(sub_task);
    sub_pid as isize
}

pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    let task = processor::current_task().unwrap();
    let mut task_inner = task.inner().exclusive_access();

    let child_idx = if pid == -1 {
        let children = &task_inner.children;
        if children.is_empty() {
            return -1; // 没有子进程，报错
        }
        children
            .iter()
            .position(|ps| ps.inner().exclusive_access().is_zombie())
    } else if pid >= 0 {
        let Some(index) = task_inner
            .children
            .iter()
            .position(|ps| ps.pid() == pid as usize)
        else {
            // 指定进程不存在 或 没有子进程，报错
            return -1;
        };

        // 只有僵尸子进程才返回index
        task_inner.children[index]
            .inner()
            .exclusive_access()
            .is_zombie()
            .then_some(index)
    } else {
        panic!("sys_waitpid only accept pid>=-1");
    };

    match child_idx {
        Some(index) => {
            let child = task_inner.children.remove(index);
            assert_eq!(Arc::strong_count(&child), 1);

            // 将子进程的退出码传递给传入的 exit_code 指针
            let exit_code = child.inner().exclusive_access().exit_code;
            *memory::read_mut(task_inner.token(), exit_code_ptr) = exit_code;

            // 传入的PID 或 僵尸进程的PID
            child.pid() as isize

            // 释放僵尸子进程
        }
        None => -2, // 子进程存在，但尚未退出
    }
}

pub fn sys_sigaction(
    signum: u32,
    action: *const SignalAction,
    old_action: *mut SignalAction,
) -> isize {
    if signum as usize >= signal::COUNT {
        return -1;
    }

    let token = processor::current_user_token();
    let task = processor::current_task().unwrap();
    let mut inner = task.inner().exclusive_access();

    let Ok(signal) = BitFlags::<SignalFlag>::from_bits(1 << signum) else {
        return -1;
    };

    if (SignalFlag::SIGKILL | SignalFlag::SIGSTOP).contains(signal)
        || action.is_null()
        || old_action.is_null()
    {
        return -1;
    }

    let prev_action = mem::replace(
        &mut inner.sigactions[signum as usize],
        memory::read_ref(token, action).clone(),
    );
    *memory::read_mut(token, old_action) = prev_action;

    0
}

pub fn sys_sigprocmask(mask: u32) -> isize {
    let Some(task) = processor::current_task() else {
        return -1;
    };
    let mut inner = task.inner().exclusive_access();

    let old_mask = &mut inner.signal_mask;
    let Ok(new_mask) = BitFlags::from_bits(mask) else {
        return -1;
    };

    mem::replace(old_mask, new_mask).bits() as isize
}

pub fn sys_sigreturn() -> isize {
    let Some(task) = processor::current_task() else {
        return -1;
    };

    let mut inner = task.inner().exclusive_access();
    inner.handling_signal = None;
    let trap_ctx = inner.trap_ctx();
    *trap_ctx = inner.trap_ctx_backup.take().unwrap();
    trap_ctx.arg(0) as isize
}

pub fn sys_kill(pid: usize, signum: u32) -> isize {
    let Some(task) = manager::get_task(pid) else {
        return -1;
    };

    let Ok(signal) = BitFlags::from_bits(1 << signum) else {
        return -1;
    };

    let mut inner = task.inner().exclusive_access();
    if inner.signals.contains(signal) {
        return -1;
    }
    inner.signals.insert(signal);

    0
}
