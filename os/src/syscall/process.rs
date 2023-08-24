use alloc::sync::Arc;
use enumflags2::BitFlags;

use crate::fs;
use crate::fs::OpenFlag;
use crate::memory;
use crate::task;
use crate::task::manager;
use crate::task::processor;
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
    println!("[kernel] Application exited with code {}", exit_code);
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
    trap_ctx.set_a0(0);

    manager::add_task(sub_task);

    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    let token = processor::current_user_token();
    let path = memory::read_str(token, path);

    let Some(app) = fs::open_file(&path, BitFlags::from_bits_truncate(OpenFlag::RDONLY)) else {
        return -1;
    };

    let data = app.read_all();
    let task = processor::current_task().unwrap();
    task.exec(&data);
    0
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
    task.inner().exclusive_access().add_child(sub_task.clone());
    sub_task
        .inner()
        .exclusive_access()
        .set_parent(Arc::downgrade(&task));

    manager::add_task(sub_task);
    sub_pid as isize
}

pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    let task = processor::current_task().unwrap();
    let mut task_inner = task.inner().exclusive_access();

    let child_idx = if pid == -1 {
        let children = task_inner.children();
        if children.is_empty() {
            return -1; // 没有子进程，报错
        }
        children
            .iter()
            .position(|ps| ps.inner().exclusive_access().is_zombie())
    } else if pid >= 0 {
        let Some(index) = task_inner
            .children()
            .iter()
            .position(|ps| ps.pid() == pid as usize)
        else {
            // 指定进程不存在 或 没有子进程，报错
            return -1;
        };

        // 只有僵尸子进程才返回index
        task_inner.children()[index]
            .inner()
            .exclusive_access()
            .is_zombie()
            .then_some(index)
    } else {
        panic!("sys_waitpid only accept pid>=-1");
    };

    match child_idx {
        Some(index) => {
            let child = task_inner.remove_child(index);
            assert_eq!(Arc::strong_count(&child), 1);

            // 将子进程的退出码传递给传入的 exit_code 指针
            let exit_code = child.inner().exclusive_access().exit_code();
            *memory::read_mut(task_inner.token(), exit_code_ptr) = exit_code;

            // 传入的PID 或 僵尸进程的PID
            child.pid() as isize

            // 释放僵尸子进程
        }
        None => -2, // 子进程存在，但尚未退出
    }
}
