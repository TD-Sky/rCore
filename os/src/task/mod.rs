//! 任务相关的结构体

pub mod switch;
use switch::get_switch_time;

mod context;
pub use self::context::TaskContext;

mod control;
pub use control::TaskControlBlock;
pub use control::TaskStatus;

pub mod pid;
use enumflags2::BitFlags;
pub use pid::PidHandle;

pub mod processor;
pub use processor::run;

pub mod manager;

use crate::fs::open_file;
use crate::fs::OpenFlag;
use crate::sbi::shutdown;
use crate::stopwatch;

use alloc::sync::Arc;
use lazy_static::lazy_static;
use log::info;

const IDLE_PID: usize = 0;

lazy_static! {
    static ref INITPROC: Arc<TaskControlBlock> = Arc::new(TaskControlBlock::new(
        &open_file("initproc", BitFlags::from_bits_truncate(OpenFlag::RDONLY))
            .unwrap()
            .read_all()
    ));
}

pub fn add_initproc() {
    manager::add_task(INITPROC.clone())
}

pub fn suspend_current_and_run_next() {
    let task = processor::take_current_task().unwrap();

    let mut task_inner = task.inner().exclusive_access();
    task_inner.task_status = TaskStatus::Ready;
    task_inner.kernel_time += stopwatch::refresh();
    let task_ctx_ptr = &mut task_inner.task_ctx as *mut TaskContext;
    drop(task_inner);

    manager::add_task(task);
    processor::schedule(task_ctx_ptr);
}

pub fn exit_current_and_run_next(exit_code: i32) {
    let task = processor::take_current_task().unwrap();
    let pid = task.pid();

    // 如果是 idle 控制流退出，说明要关机了
    if pid == IDLE_PID {
        println!("[kernel] Idle process exit with exit_code={}", exit_code);
        info!("task switch time: {}us", get_switch_time());

        shutdown(exit_code != 0);
    }

    let mut task_inner = task.inner().exclusive_access();
    {
        let mut initproc = INITPROC.inner().exclusive_access();
        for child in task_inner.children() {
            child.inner().exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
            initproc.children.push(child.clone());
        }
    }
    task_inner.die(exit_code);

    info!(
        "task pid={} dead. user_time={}ms, kernel_time={}ms",
        pid, task_inner.user_time, task_inner.kernel_time
    );

    drop(task_inner);
    drop(task);

    let mut tmp_task_ctx = TaskContext::default();
    processor::schedule(&mut tmp_task_ctx as *mut TaskContext);
}

pub fn user_time_start() {
    let task = processor::current_task().unwrap();
    let mut task_inner = task.inner().exclusive_access();
    task_inner.kernel_time += stopwatch::refresh();
}

pub fn user_time_end() {
    let task = processor::current_task().unwrap();
    let mut task_inner = task.inner().exclusive_access();
    task_inner.user_time += stopwatch::refresh();
}
