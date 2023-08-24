//! 任务相关的结构体

mod context;
mod control;
pub mod manager;
mod pid;
pub mod processor;
pub mod signal;
pub mod switch;

pub use self::context::TaskContext;
pub use self::control::{TaskControlBlock, TaskStatus};
pub use self::pid::PidHandle;
pub use self::processor::run;
use self::signal::SignalFlag;

use alloc::sync::Arc;
use enumflags2::BitFlags;
use lazy_static::lazy_static;
use log::info;

use self::switch::get_switch_time;
use crate::fs::open_file;
use crate::fs::OpenFlag;
use crate::sbi::shutdown;
use crate::stopwatch;

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

    manager::remove_task(pid);

    let mut task_inner = task.inner().exclusive_access();
    {
        let mut initproc = INITPROC.inner().exclusive_access();
        for child in &task_inner.children {
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

pub fn send_signal_to_current(signal: SignalFlag) {
    let task = processor::current_task().unwrap();
    let mut inner = task.inner().exclusive_access();
    inner.signals |= signal;
}

pub fn handle_signals() {
    loop {
        check_pending_signals();

        let is_hibernating = {
            let task = processor::current_task().unwrap();
            let inner = task.inner().exclusive_access();
            inner.is_hibernating()
        };
        if !is_hibernating {
            break;
        }

        suspend_current_and_run_next();
    }
}

pub fn check_current_signal_error() -> Option<(i32, &'static str)> {
    let task = processor::current_task().unwrap();
    let inner = task.inner().exclusive_access();
    signal::check_error(inner.signals)
}

pub fn user_time_start() {
    let task = processor::current_task().unwrap();
    let mut inner = task.inner().exclusive_access();
    inner.kernel_time += stopwatch::refresh();
}

pub fn user_time_end() {
    let task = processor::current_task().unwrap();
    let mut inner = task.inner().exclusive_access();
    inner.user_time += stopwatch::refresh();
}

fn check_pending_signals() {
    let task = processor::current_task().unwrap();
    let mut inner = task.inner().exclusive_access();

    // 剔除收到信号中全局屏蔽的部分
    let mut pending_signals = inner.signals;
    pending_signals.remove(inner.signal_mask);

    for signal in pending_signals.iter() {
        // 检查当前信号处理例程是否屏蔽了`signal`
        let masked = inner
            .handling_signal
            .map(|sn| inner.sigactions[sn as usize].mask.contains(signal))
            .unwrap_or_default();

        if !masked {
            if (SignalFlag::SIGKILL
                | SignalFlag::SIGSTOP
                | SignalFlag::SIGCONT
                | SignalFlag::SIGDEF)
                .contains(signal)
            {
                // signal is a kernel signal
                inner.kernel_signal_handler(signal);
            } else {
                // signal is a user signal
                inner.user_signal_handler((signal as u32).trailing_zeros() as usize, signal);
                return;
            }
        }
    }
}
