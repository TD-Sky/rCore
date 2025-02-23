use alloc::sync::Arc;
use alloc::vec::Vec;

use enumflags2::BitFlags;

use crate::fs;
use crate::fs::OpenFlag;
use crate::memory;
use crate::task::ProcessControlBlock;
use crate::task::manager;
use crate::task::processor;
use crate::task::signal::SignalAction;

pub fn sys_getpid() -> isize {
    processor::current_process().pid() as isize
}

pub fn sys_fork() -> isize {
    let current_process = processor::current_process();
    // 此时子进程的CPU状态与父进程相同，都在 sys_fork
    let sub_process = current_process.fork();
    let new_pid = sub_process.pid();

    let trap_ctx = sub_process
        .inner()
        .exclusive_access()
        .tasks
        .get(0)
        .inner()
        .exclusive_access()
        .trap_ctx();
    // 将子进程的 fork 返回值设为 0
    trap_ctx.set_syscall_result(0);

    new_pid as isize
}

pub fn sys_exec(path: *const u8, mut args: *const usize) -> isize {
    let token = processor::current_user_token();
    let path = memory::read_str(token, path);
    log::info!("Executing: {path}");

    let mut arg_vec = Vec::new();
    loop {
        let arg = *memory::read_ref(token, args) as *const u8;
        if arg.is_null() {
            break;
        }
        log::debug!("token={token:#x} arg={arg:#p}");
        arg_vec.push(memory::read_str(token, arg));
        unsafe {
            args = args.add(1);
        }
    }

    let Some(app) = fs::open(&path, OpenFlag::read_only()) else {
        return -1;
    };

    let data = app.read_all();
    let process = processor::current_process();
    let argc = arg_vec.len();
    process.exec(&data, arg_vec);

    // 返回`argc`是因为exec里`ctx.x[10]`被设成该值，
    // 需在后续写入系统调用结果(同为`ctx.x[10]`)时与其保持一致
    argc as isize
}

pub fn sys_spawn(path: *const u8) -> isize {
    let token = processor::current_user_token();
    let path = memory::read_str(token, path);

    let Some(app) = fs::open(&path, BitFlags::from_bits_truncate(OpenFlag::RDONLY)) else {
        return -1;
    };

    let sub_process = ProcessControlBlock::new(&app.read_all());
    let sub_pid = sub_process.pid();

    let current_process = processor::current_process();
    current_process
        .inner()
        .exclusive_access()
        .children
        .push(sub_process.clone());
    sub_process.inner().exclusive_access().parent = Some(Arc::downgrade(&current_process));

    sub_pid as isize
}

pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    let process = processor::current_process();
    let mut process = process.inner().exclusive_access();

    let child_idx = if pid == -1 {
        let children = &process.children;
        if children.is_empty() {
            return -1; // 没有子进程，报错
        }
        children
            .iter()
            .position(|ps| ps.inner().exclusive_access().is_zombie)
    } else if pid >= 0 {
        let Some(index) = process
            .children
            .iter()
            .position(|ps| ps.pid() == pid as usize)
        else {
            // 指定进程不存在 或 没有子进程，报错
            return -1;
        };

        // 只有僵尸子进程才返回index
        process.children[index]
            .inner()
            .exclusive_access()
            .is_zombie
            .then_some(index)
    } else {
        panic!("sys_waitpid only accept pid>=-1");
    };

    match child_idx {
        Some(index) => {
            let child = process.children.remove(index);
            assert_eq!(Arc::strong_count(&child), 1);

            // 将子进程的退出码传递给传入的 exit_code 指针
            let exit_code = child.inner().exclusive_access().exit_code;
            *memory::read_mut(process.user_token(), exit_code_ptr) = exit_code;

            // 传入的PID 或 僵尸进程的PID
            child.pid() as isize

            // 释放僵尸子进程
        }
        None => -2, // 子进程存在，但尚未退出
    }
}

#[allow(unused_variables)]
pub fn sys_sigaction(
    signum: u32,
    action: *const SignalAction,
    old_action: *mut SignalAction,
) -> isize {
    -1
}

#[allow(unused_variables)]
pub fn sys_sigprocmask(mask: u32) -> isize {
    -1
}

#[allow(unused_variables)]
pub fn sys_sigreturn() -> isize {
    -1
}

pub fn sys_kill(pid: usize, signum: u32) -> isize {
    let Some(process) = manager::get_process(pid) else {
        return -1;
    };

    let Ok(signal) = BitFlags::from_bits(1 << signum) else {
        return -1;
    };

    let mut inner = process.inner().exclusive_access();
    if inner.signals.contains(signal) {
        return -1;
    }
    inner.signals.insert(signal);

    0
}

/// 改变data段的大小
#[allow(unused_variables)]
pub fn sys_sbrk(size: i32) -> isize {
    -1
}

#[allow(unused_variables)]
pub fn sys_mmap(start: usize, len: usize, prot: u8) -> isize {
    -1
}

#[allow(unused_variables)]
pub fn sys_munmap(start: usize, len: usize) -> isize {
    -1
}
