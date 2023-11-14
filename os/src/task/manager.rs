//! 预备进程调度器

use alloc::collections::{BTreeMap, VecDeque};
use alloc::sync::Arc;

use super::{ProcessControlBlock, TaskControlBlock, TaskStatus};
use crate::sync::UpCell;
use crate::timer;

static TASK_MANAGER: UpCell<TaskManager> = UpCell::new(TaskManager::new());
static PID2TCB: UpCell<BTreeMap<usize, Arc<ProcessControlBlock>>> = UpCell::new(BTreeMap::new());

pub fn add_task(task: Arc<TaskControlBlock>) {
    TASK_MANAGER.exclusive_access().add(task);
}

#[inline]
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.exclusive_access().fetch()
}

#[inline]
pub fn remove_task(task: &Arc<TaskControlBlock>) {
    timer::remove_timer(task);
    TASK_MANAGER.exclusive_access().remove(task);
}

pub fn wakeup_task(task: Arc<TaskControlBlock>) {
    task.inner().exclusive_access().status = TaskStatus::Ready;
    add_task(task);
}

pub fn get_process(pid: usize) -> Option<Arc<ProcessControlBlock>> {
    PID2TCB.exclusive_access().get(&pid).cloned()
}

pub fn insert_process(pid: usize, process: Arc<ProcessControlBlock>) {
    PID2TCB.exclusive_access().insert(pid, process);
}

pub fn remove_process(pid: usize) {
    if PID2TCB.exclusive_access().remove(&pid).is_none() {
        panic!("no process pid={pid} in PID-to-process");
    }
}

/// FIFO 预备进程调度器
#[derive(Default)]
struct TaskManager {
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl TaskManager {
    const fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
        }
    }

    fn add(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }

    fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.ready_queue.pop_front()
    }

    fn remove(&mut self, task: &Arc<TaskControlBlock>) {
        let task = Arc::as_ptr(task);

        if let Some((id, _)) = self
            .ready_queue
            .iter()
            .enumerate()
            .find(|(_, t)| task == Arc::as_ptr(t))
        {
            self.ready_queue.remove(id);
        }
    }
}
