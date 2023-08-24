//! CPU状态管理

use alloc::sync::Arc;
use lazy_static::lazy_static;

use super::manager;
use super::switch::switch;
use super::TaskContext;
use super::TaskControlBlock;
use super::TaskStatus;

use crate::stopwatch;
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;

lazy_static! {
    static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::default()) };
}

pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

#[derive(Default)]
struct Processor {
    current: Option<Arc<TaskControlBlock>>,
    idle_task_ctx: TaskContext,
}

impl Processor {
    fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.clone()
    }

    fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }
}

pub fn current_user_token() -> usize {
    current_task().unwrap().inner().exclusive_access().token()
}

pub fn current_trap_ctx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner()
        .exclusive_access()
        .trap_ctx()
}

/// 启动 idle 控制流
pub fn run() {
    stopwatch::refresh();
    loop {
        let mut processor = PROCESSOR.exclusive_access();

        // 直到取得预备的新任务
        if let Some(task) = manager::fetch_task() {
            let idle_task_ctx_ptr = &mut processor.idle_task_ctx as *mut TaskContext;

            let mut task_inner = task.inner().exclusive_access();
            task_inner.task_status = TaskStatus::Running;
            let next_task_ctx_ptr = &task_inner.task_ctx as *const TaskContext;
            drop(task_inner);

            processor.current = Some(task);
            drop(processor);

            unsafe {
                switch(idle_task_ctx_ptr, next_task_ctx_ptr);
            }
            // 从 schedule 切换回来，继续循环
        }
    }
}

/// 切换回 idle 控制流
pub fn schedule(task_ctx_ptr: *mut TaskContext) {
    let processor = PROCESSOR.exclusive_access();
    let idle_task_ctx_ptr = &processor.idle_task_ctx as *const TaskContext;
    drop(processor);

    unsafe {
        switch(task_ctx_ptr, idle_task_ctx_ptr);
    }
}
