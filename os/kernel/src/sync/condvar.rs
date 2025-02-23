use alloc::collections::VecDeque;
use alloc::sync::Arc;

use crate::task;
use crate::task::TaskContext;
use crate::task::TaskControlBlock;
use crate::task::manager;
use crate::task::processor;

use super::Mutex;
use super::UpCell;

#[derive(Debug)]
pub struct Condvar {
    wait_queue: UpCell<VecDeque<Arc<TaskControlBlock>>>,
}

impl Condvar {
    pub const fn new() -> Self {
        Self {
            wait_queue: UpCell::new(VecDeque::new()),
        }
    }

    pub fn signal(&self) {
        while let Some(task) = self.wait_queue.exclusive_access().pop_front() {
            manager::wakeup_task(task);
        }
    }

    pub fn wait_with_mutex(&self, mutex: Arc<dyn Mutex>) {
        mutex.unlock();
        self.wait_queue
            .exclusive_access()
            .push_back(processor::current_task().unwrap());
        task::block_current_and_run_next();
        mutex.lock();
    }

    /// 唤醒一个睡眠的任务，仅给内核使用
    pub fn wait(&self) -> *mut TaskContext {
        self.wait_queue
            .exclusive_access()
            .push_back(processor::current_task().unwrap());
        task::block_current()
    }
}
