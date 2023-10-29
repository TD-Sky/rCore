use alloc::collections::VecDeque;
use alloc::sync::Arc;
use core::sync::atomic;
use core::sync::atomic::AtomicU64;
use enumflags2::bitflags;
use enumflags2::BitFlags;

use super::File;
use crate::memory::UserBuffer;
use crate::sync::UPSafeCell;
use crate::task;
use crate::task::manager;
use crate::task::processor;
use crate::task::TaskControlBlock;

#[allow(clippy::upper_case_acronyms)]
#[bitflags]
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventFdFlag {
    SEMAPHORE = 0b0000_0000_0001,
    NONBLOCK = 0b1000_0000_0000,
}

pub fn new(count: u64, flags: BitFlags<EventFdFlag>) -> Arc<dyn File + Send + Sync> {
    let semaphore = flags.contains(EventFdFlag::SEMAPHORE);
    let non_block = flags.contains(EventFdFlag::NONBLOCK);

    let count = AtomicU64::new(count);
    match (semaphore, non_block) {
        (false, false) => Arc::new(EventFdContext {
            count,
            wait_queue: unsafe { UPSafeCell::new(VecDeque::new()) },
        }),
        (false, true) => Arc::new(NonBlockEventFdContext { count }),
        (true, false) => Arc::new(SemEventFdContext {
            count,
            wait_queue: unsafe { UPSafeCell::new(VecDeque::new()) },
        }),
        (true, true) => Arc::new(SemNonBlockEventFdContext { count }),
    }
}

struct EventFdContext {
    count: AtomicU64,
    wait_queue: UPSafeCell<VecDeque<Arc<TaskControlBlock>>>,
}

struct NonBlockEventFdContext {
    count: AtomicU64,
}

struct SemEventFdContext {
    count: AtomicU64,
    wait_queue: UPSafeCell<VecDeque<Arc<TaskControlBlock>>>,
}

struct SemNonBlockEventFdContext {
    count: AtomicU64,
}

impl File for EventFdContext {
    fn writable(&self) -> bool {
        true
    }

    fn readable(&self) -> bool {
        true
    }

    fn read(&self, mut buf: UserBuffer) -> usize {
        loop {
            let count = self.count.swap(0, atomic::Ordering::Acquire);
            if count > 0 {
                write_u64(count, &mut buf);
                break 0;
            } else {
                wait(&self.wait_queue);
            }
        }
    }

    fn write(&self, mut buf: UserBuffer) -> usize {
        let added = read_u64(&mut buf);

        if added == 0 {
            // 异常
            return usize::MAX;
        }

        let mut count = self.count.load(atomic::Ordering::Acquire);
        loop {
            if let Some(new) = count.checked_add(added) {
                let Err(current) = self.count.compare_exchange(
                    count,
                    new,
                    atomic::Ordering::AcqRel,
                    atomic::Ordering::Acquire,
                ) else {
                    if let Some(task) = self.wait_queue.exclusive_access().pop_front() {
                        manager::wakeup_task(task);
                    }
                    break 0;
                };

                count = current;
            } else {
                wait(&self.wait_queue);
                count = self.count.load(atomic::Ordering::Acquire);
            }
        }
    }

    fn stat(&self) -> easy_fs::Stat {
        unimplemented!()
    }
}

impl File for NonBlockEventFdContext {
    fn writable(&self) -> bool {
        true
    }

    fn readable(&self) -> bool {
        true
    }

    fn read(&self, mut buf: UserBuffer) -> usize {
        let count = self.count.swap(0, atomic::Ordering::Acquire);
        if count > 0 {
            write_u64(count, &mut buf);
            0
        } else {
            usize::MAX
        }
    }

    fn write(&self, mut buf: UserBuffer) -> usize {
        let added = read_u64(&mut buf);

        if added == 0 {
            // 异常
            return usize::MAX;
        }

        let count = self.count.load(atomic::Ordering::Acquire);
        let Some(new) = count.checked_add(added) else {
            return usize::MAX;
        };

        if self
            .count
            .compare_exchange(
                count,
                new,
                atomic::Ordering::AcqRel,
                atomic::Ordering::Acquire,
            )
            .is_ok()
        {
            0
        } else {
            usize::MAX
        }
    }

    fn stat(&self) -> easy_fs::Stat {
        unimplemented!()
    }
}

impl File for SemEventFdContext {
    fn writable(&self) -> bool {
        true
    }

    fn readable(&self) -> bool {
        true
    }

    fn read(&self, mut buf: UserBuffer) -> usize {
        let mut count = self.count.load(atomic::Ordering::Acquire);

        // 若资源派发完，则去排队
        if count == 0 {
            wait(&self.wait_queue);
            write_u64(1, &mut buf);
            return 0;
        }

        // 尝试获取到一个资源，直到成功为止。
        // 若中途发现资源用光，则去排队。
        while let Err(current) = self.count.compare_exchange(
            count,
            count - 1,
            atomic::Ordering::AcqRel,
            atomic::Ordering::Acquire,
        ) {
            if current == 0 {
                wait(&self.wait_queue);
                write_u64(1, &mut buf);
                return 0;
            }
            count = current;
        }

        0
    }

    fn write(&self, _buf: UserBuffer) -> usize {
        if let Some(task) = self.wait_queue.exclusive_access().pop_front() {
            manager::wakeup_task(task);
        } else {
            self.count.fetch_add(1, atomic::Ordering::Release);
        }

        0
    }

    fn stat(&self) -> easy_fs::Stat {
        unimplemented!()
    }
}

impl File for SemNonBlockEventFdContext {
    fn writable(&self) -> bool {
        true
    }

    fn readable(&self) -> bool {
        true
    }

    fn read(&self, mut buf: UserBuffer) -> usize {
        let mut count = self.count.load(atomic::Ordering::Acquire);

        if count == 0 {
            return usize::MAX;
        }

        // 尝试获取到一个资源，直到成功为止。
        while let Err(current) = self.count.compare_exchange(
            count,
            count - 1,
            atomic::Ordering::AcqRel,
            atomic::Ordering::Acquire,
        ) {
            if current == 0 {
                return usize::MAX;
            }
            count = current;
        }

        write_u64(1, &mut buf);
        0
    }

    fn write(&self, _buf: UserBuffer) -> usize {
        self.count.fetch_add(1, atomic::Ordering::Release);
        0
    }

    fn stat(&self) -> easy_fs::Stat {
        unimplemented!()
    }
}

fn wait(queue: &UPSafeCell<VecDeque<Arc<TaskControlBlock>>>) {
    queue
        .exclusive_access()
        .push_back(processor::current_task().unwrap());
    task::block_current_and_run_next();
}

fn read_u64(buf: &mut UserBuffer) -> u64 {
    let mut num = [0u8; 8];
    for (i, &byte) in num.iter_mut().zip(buf.iter()) {
        *i = byte;
    }
    u64::from_ne_bytes(num)
}

fn write_u64(num: u64, buf: &mut UserBuffer) {
    let num = num.to_ne_bytes();
    for (i, byte) in buf.iter_mut().zip(num) {
        *i = byte;
    }
}
