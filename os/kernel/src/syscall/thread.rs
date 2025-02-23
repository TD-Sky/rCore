use alloc::sync::Arc;

use crate::memory;
use crate::task;
use crate::task::TaskControlBlock;
use crate::task::manager;
use crate::task::processor;
use crate::timer;
use crate::timer::TimerCondVar;
use crate::trap::TrapContext;
use crate::trap::trap_handler;

// 切换任务，用户模式的上下文依旧是由 trap_handler 保存
pub fn sys_yield() -> isize {
    task::suspend_current_and_run_next();
    0
}

pub fn sys_sleep(ms: usize) -> isize {
    let expire_ms = timer::get_time_ms() + ms;
    let task = processor::current_task().unwrap();
    timer::add_timer(TimerCondVar::new(expire_ms, task));
    task::block_current_and_run_next();
    0
}

pub fn sys_exit(exit_code: i32) -> ! {
    task::exit_current_and_run_next(exit_code);
    unreachable!()
}

pub fn sys_spawn_thread(entry: usize, arg: usize) -> isize {
    let task = processor::current_task().unwrap();
    let process = task.process.upgrade().unwrap();
    let new_task = Arc::new(TaskControlBlock::new(
        &process,
        task.inner().exclusive_access().resource.user_stack_base,
        false,
    ));

    manager::add_task(new_task.clone());
    process
        .inner()
        .exclusive_access()
        .insert_task(new_task.clone());

    let new_task_inner = new_task.inner().exclusive_access();
    let new_task_trap_ctx = new_task_inner.trap_ctx();
    *new_task_trap_ctx = TrapContext::init(
        entry,
        new_task_inner.resource.user_stack_top(),
        memory::kernel_token(),
        new_task.kernel_stack.top(),
        trap_handler as usize,
    );

    new_task_trap_ctx.set_syscall_result(arg);
    new_task_inner.resource.tid as isize
}

pub fn sys_gettid() -> isize {
    processor::current_task()
        .unwrap()
        .inner()
        .exclusive_access()
        .resource
        .tid as isize
}

pub fn sys_waittid(tid: usize) -> isize {
    let task = processor::current_task().unwrap();
    let process = task.process.upgrade().unwrap();
    let task = task.inner().exclusive_access();
    let mut process = process.inner().exclusive_access();

    // a thread cannot wait for itself
    if task.resource.tid == tid {
        return -1;
    }

    let Some(waited_task) = &process.tasks[tid] else {
        return -1;
    };
    let Some(exit_code) = waited_task.inner().exclusive_access().exit_code else {
        return -2;
    };
    // 资源已经在`task::exit_current_and_run_next`里面释放了
    process.tasks.remove(tid);
    exit_code as isize
}
