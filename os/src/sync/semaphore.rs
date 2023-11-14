use alloc::collections::VecDeque;
use alloc::sync::Arc;
use core::sync::atomic;
use core::sync::atomic::AtomicUsize;

use super::UpCell;
use crate::task;
use crate::task::manager;
use crate::task::processor;
use crate::task::TaskControlBlock;

pub struct Semaphore {
    permits: AtomicUsize,
    wait_queue: UpCell<VecDeque<Arc<TaskControlBlock>>>,
}

impl Semaphore {
    pub fn new(permits: usize) -> Self {
        Self {
            permits: AtomicUsize::new(permits),
            wait_queue: UpCell::new(VecDeque::new()),
        }
    }

    /// Verhogen 增加
    pub fn up(&self) {
        if let Some(task) = self.wait_queue.exclusive_access().pop_front() {
            // 转让当前任务的资源
            manager::wakeup_task(task);
        } else {
            // 释放当前任务的资源
            self.permits.fetch_add(1, atomic::Ordering::Release);
        }
    }

    /// Proberen 尝试
    pub fn down(&self) {
        let mut permits = self.permits.load(atomic::Ordering::Acquire);

        // 若资源派发完，则去排队
        if permits == 0 {
            self.wait_current();
            return;
        }

        // 尝试获取到一个资源，直到成功为止。
        // 若中途发现资源用光，则去排队。
        while let Err(current) = self.permits.compare_exchange(
            permits,
            permits - 1,
            atomic::Ordering::AcqRel,
            atomic::Ordering::Acquire,
        ) {
            if current == 0 {
                self.wait_current();
                break;
            }
            permits = current;
        }
    }
}

impl Semaphore {
    fn wait_current(&self) {
        self.wait_queue
            .exclusive_access()
            .push_back(processor::current_task().unwrap());
        task::block_current_and_run_next();
    }
}
