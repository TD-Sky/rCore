use alloc::collections::VecDeque;
use alloc::sync::Arc;
use core::sync::atomic;
use core::sync::atomic::AtomicBool;

use super::UpCell;
use crate::task;
use crate::task::manager;
use crate::task::processor;
use crate::task::TaskControlBlock;

pub trait Mutex: Send + Sync {
    fn lock(&self);
    fn unlock(&self);
}

pub struct SpinMutex {
    locked: AtomicBool,
}

pub struct BlockMutex {
    locked: AtomicBool,
    wait_queue: UpCell<VecDeque<Arc<TaskControlBlock>>>,
}

impl Mutex for SpinMutex {
    fn lock(&self) {
        while self.locked.swap(true, atomic::Ordering::Acquire) {
            task::suspend_current_and_run_next();
        }
    }

    fn unlock(&self) {
        self.locked.store(false, atomic::Ordering::Release);
    }
}

impl Mutex for BlockMutex {
    fn lock(&self) {
        if self.locked.swap(true, atomic::Ordering::Acquire) {
            // 也许你觉得lock里可以随意访问独占引用很迷惑，但是目前
            //
            // 1. 我们不会在内核里使用`BlockMutex`
            // 2. RISC-V 规定从用户态陷入内核态之后所有内核态中断默认被自动屏蔽，
            //    系统调用的执行不会被打断。
            // 3. 系统是单核运行，不会有多个CPU同时执行系统调用。
            //
            // 所以这么做是安全的
            self.wait_queue
                .exclusive_access()
                .push_back(processor::current_task().unwrap());
            task::block_current_and_run_next();
        }
    }

    fn unlock(&self) {
        // 必须是上锁状态
        assert!(self.locked.load(atomic::Ordering::Acquire));
        if let Some(waiting_task) = self.wait_queue.exclusive_access().pop_front() {
            // 存在等候者，唤醒之，锁转移到其手上
            manager::wakeup_task(waiting_task);
        } else {
            // 没有等候者，直接解锁
            self.locked.store(false, atomic::Ordering::Release);
        }
    }
}

impl SpinMutex {
    pub fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
        }
    }
}

impl BlockMutex {
    pub fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
            wait_queue: UpCell::new(VecDeque::new()),
        }
    }
}
