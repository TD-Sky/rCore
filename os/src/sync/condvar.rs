use alloc::collections::VecDeque;
use alloc::sync::Arc;

use crate::task;
use crate::task::manager;
use crate::task::processor;
use crate::task::TaskControlBlock;

use super::Mutex;
use super::UPSafeCell;

pub struct Condvar {
    wait_queue: UPSafeCell<VecDeque<Arc<TaskControlBlock>>>,
}

impl Condvar {
    pub fn new() -> Self {
        Self {
            wait_queue: unsafe { UPSafeCell::new(VecDeque::new()) },
        }
    }

    pub fn signal(&self) {
        while let Some(task) = self.wait_queue.exclusive_access().pop_front() {
            manager::wakeup_task(task);
        }
    }

    pub fn wait(&self, mutex: Arc<dyn Mutex>) {
        mutex.unlock();
        self.wait_queue
            .exclusive_access()
            .push_back(processor::current_task().unwrap());
        task::block_current_and_run_next();
        mutex.lock();
    }
}
