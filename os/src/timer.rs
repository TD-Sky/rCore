//! RISC-V timer-related functionality
//!
//! RISC-V架构要求CPU有一个计数器用来统计处理器自上电
//! 以来经过了多少个内置时钟的时钟周期，
//! 其保存在一个64位的CSR`mtime`中。
//! 我们无需担心它会溢出，可假设它是内核全程递增的。

use alloc::collections::BinaryHeap;
use alloc::sync::Arc;
use core::cmp::{Ordering, Reverse};

use lazy_static::lazy_static;
use riscv::register::time;

use crate::config::CLOCK_FREQ;
use crate::sbi::set_timer;
use crate::sync::UPSafeCell;
use crate::task::{manager, TaskControlBlock};

const TICKS_PRE_SEC: usize = 100;
const MILLISECONDS: usize = 1000;
/* const MICROSECONDS: usize = 1_000_000; */

lazy_static! {
    static ref TIMERS: UPSafeCell<BinaryHeap<TimerCondVar>> =
        unsafe { UPSafeCell::new(BinaryHeap::new()) };
}

/// read the `mtime` register
pub fn get_time() -> usize {
    time::read()
}

/// get current time in milliseconds
pub fn get_time_ms() -> usize {
    time::read() / (CLOCK_FREQ / MILLISECONDS)
}

/// set `mtimecmp`, the next timer interrupt
pub fn set_next_trigger() {
    set_timer(get_time() + CLOCK_FREQ / TICKS_PRE_SEC);
}

pub struct TimerCondVar {
    expire_ms: usize,
    task: Arc<TaskControlBlock>,
}

impl TimerCondVar {
    pub fn new(expire_ms: usize, task: Arc<TaskControlBlock>) -> Self {
        Self { expire_ms, task }
    }
}

pub fn add_timer(timer: TimerCondVar) {
    TIMERS.exclusive_access().push(timer);
}

/// 移除传入任务的所有计时器
pub fn remove_timer(task: &Arc<TaskControlBlock>) {
    let task = Arc::as_ptr(task);
    TIMERS
        .exclusive_access()
        .retain(|t| Arc::as_ptr(&t.task) != task);
}

pub fn wakeup_timeout_tasks() {
    let current_ms = get_time_ms();
    let mut timers = TIMERS.exclusive_access();
    while let Some(timer) = timers.peek()
        && timer.expire_ms <= current_ms
    {
        let timer = timers.pop().unwrap();
        manager::wakeup_task(timer.task);
    }
}

impl PartialEq for TimerCondVar {
    fn eq(&self, other: &Self) -> bool {
        self.expire_ms == other.expire_ms
    }
}

impl Eq for TimerCondVar {}

impl PartialOrd for TimerCondVar {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimerCondVar {
    fn cmp(&self, other: &Self) -> Ordering {
        Reverse(self.expire_ms).cmp(&Reverse(other.expire_ms))
    }
}

/*
* /// get current time in microseconds
* pub fn get_time_us() -> usize {
*     time::read() / (CLOCK_FREQ / MICROSECONDS)
* }
*/
