//! 任务相关的结构体

mod context;
mod id;
pub mod manager;
mod process;
pub mod processor;
pub mod signal;
pub mod switch;
#[allow(clippy::module_inception)]
mod task;

pub use self::{
    context::TaskContext,
    id::RecycleAllocator,
    process::ProcessControlBlock,
    processor::run,
    switch::__switch,
    task::{TaskControlBlock, TaskStatus},
};

use alloc::sync::Arc;
use core::mem;

use enumflags2::BitFlags;
use lazy_static::lazy_static;

use self::signal::SignalFlag;
use crate::fs::open_file;
use crate::fs::OpenFlag;
use crate::sbi::shutdown;

const IDLE_PID: usize = 0;

lazy_static! {
    static ref INITPROC: Arc<ProcessControlBlock> = ProcessControlBlock::new(
        &open_file("initproc", BitFlags::from_bits_truncate(OpenFlag::RDONLY))
            .unwrap()
            .read_all()
    );
}

pub fn add_initproc() {
    lazy_static::initialize(&INITPROC);
}

pub fn suspend_current_and_run_next() {
    let task = processor::take_current_task().unwrap();

    let mut task_inner = task.inner().exclusive_access();
    task_inner.status = TaskStatus::Ready;
    let task_ctx_ptr = &mut task_inner.ctx as *mut TaskContext;
    drop(task_inner);

    manager::add_task(task);
    processor::schedule(task_ctx_ptr);
}

pub fn block_current_and_run_next() {
    let task = processor::take_current_task().unwrap();
    let mut task_inner = task.inner().exclusive_access();
    let task_ctx_ptr = &mut task_inner.ctx as *mut TaskContext;
    task_inner.status = TaskStatus::Blocked;
    drop(task_inner);

    processor::schedule(task_ctx_ptr);
}

pub fn exit_current_and_run_next(exit_code: i32) {
    let task = processor::take_current_task().unwrap();
    let mut task_inner = task.inner().exclusive_access();
    let tid = task_inner.resource.tid;

    task_inner.exit_code = Some(exit_code);
    task_inner.resource.dealloc();

    let process = task.process.upgrade().unwrap();
    drop(task_inner);
    drop(task);

    if tid == 0 {
        /* 退出主线程，即退出进程 */
        let pid = process.pid();
        if pid == IDLE_PID {
            /* 如果是 idle 控制流退出，说明要关机了 */
            log::info!("[kernel] Idle process exit with exit_code={exit_code}");
            shutdown(exit_code != 0);
        }

        manager::remove_process(pid);
        let mut process_inner = process.inner().exclusive_access();
        process_inner.is_zombie = true;
        process_inner.exit_code = exit_code;

        {
            let mut initproc = INITPROC.inner().exclusive_access();
            for child in &process_inner.children {
                child.inner().exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
                initproc.children.push(child.clone());
            }
        }

        let tasks = mem::take(&mut process_inner.tasks);
        drop(process_inner);

        for task in tasks.iter().filter_map(Option::as_ref) {
            let task_inner = task.inner().exclusive_access();
            manager::remove_task(task);
            // 若退出码为Some，说明任务自己释放了资源，毋须再次释放
            if task_inner.exit_code.is_none() {
                task_inner.resource.dealloc();
            }
        }

        process.inner().exclusive_access().die();
    }

    drop(process);
    let mut tmp_task_ctx = TaskContext::default();
    processor::schedule(&mut tmp_task_ctx as *mut TaskContext);
}

pub fn send_signal_to_current(signal: SignalFlag) {
    processor::current_process()
        .inner()
        .exclusive_access()
        .signals |= signal;
}

pub fn check_current_signal_error() -> Option<(i32, &'static str)> {
    let signals = processor::current_process()
        .inner()
        .exclusive_access()
        .signals;
    signal::check_error(signals)
}

// pub fn handle_signals() {
//     loop {
//         check_pending_signals();
//
//         let is_hibernating = {
//             let task = processor::current_task().unwrap();
//             let inner = task.inner().exclusive_access();
//             inner.is_hibernating()
//         };
//         if !is_hibernating {
//             break;
//         }
//
//         suspend_current_and_run_next();
//     }
// }

// pub fn user_time_start() {
//     let task = processor::current_task().unwrap();
//     let mut inner = task.inner().exclusive_access();
//     inner.kernel_time += stopwatch::refresh();
// }
//
// pub fn user_time_end() {
//     let task = processor::current_task().unwrap();
//     let mut inner = task.inner().exclusive_access();
//     inner.user_time += stopwatch::refresh();
// }
//
// fn check_pending_signals() {
//     let task = processor::current_task().unwrap();
//     let mut inner = task.inner().exclusive_access();
//
//     // 剔除收到信号中全局屏蔽的部分
//     let mut pending_signals = inner.signals;
//     pending_signals.remove(inner.signal_mask);
//
//     for signal in pending_signals.iter() {
//         // 检查当前信号处理例程是否屏蔽了`signal`
//         let masked = inner
//             .handling_signal
//             .map(|sn| inner.sigactions[sn as usize].mask.contains(signal))
//             .unwrap_or_default();
//
//         if !masked {
//             if (SignalFlag::SIGKILL
//                 | SignalFlag::SIGSTOP
//                 | SignalFlag::SIGCONT
//                 | SignalFlag::SIGDEF)
//                 .contains(signal)
//             {
//                 // signal is a kernel signal
//                 inner.kernel_signal_handler(signal);
//             } else {
//                 // signal is a user signal
//                 inner.user_signal_handler((signal as u32).trailing_zeros() as usize, signal);
//                 return;
//             }
//         }
//     }
// }
