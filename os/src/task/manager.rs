//! 预备进程调度器

use alloc::collections::{BTreeMap, VecDeque};
use alloc::sync::Arc;
use lazy_static::lazy_static;

use super::TaskControlBlock;
use crate::sync::UPSafeCell;

lazy_static! {
    static ref TASK_MANAGER: UPSafeCell<TaskManager> =
        unsafe { UPSafeCell::new(TaskManager::default()) };
    static ref PID2TCB: UPSafeCell<BTreeMap<usize, Arc<TaskControlBlock>>> =
        unsafe { UPSafeCell::new(BTreeMap::new()) };
}

pub fn add_task(task: Arc<TaskControlBlock>) {
    PID2TCB.exclusive_access().insert(task.pid(), task.clone());
    TASK_MANAGER.exclusive_access().add(task);
}

#[inline]
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.exclusive_access().fetch()
}

#[inline]
pub fn get_task(pid: usize) -> Option<Arc<TaskControlBlock>> {
    PID2TCB.exclusive_access().get(&pid).cloned()
}

#[inline]
pub fn remove_task(pid: usize) {
    if PID2TCB.exclusive_access().remove(&pid).is_none() {
        panic!("Task not found in pid2task, PID={pid}");
    }
}

/// FIFO 预备进程调度器
#[derive(Default)]
struct TaskManager {
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl TaskManager {
    fn add(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }

    fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.ready_queue.pop_front()
    }
}
