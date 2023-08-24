use crate::sync::UPSafeCell;
use alloc::vec::Vec;
use lazy_static::lazy_static;

pub fn alloc() -> PidHandle {
    PID_ALLOCATOR.exclusive_access().alloc()
}

lazy_static! {
    static ref PID_ALLOCATOR: UPSafeCell<PidAllocator> =
        unsafe { UPSafeCell::new(PidAllocator::default()) };
}

#[derive(Default)]
struct PidAllocator {
    current: usize,
    recycled: Vec<usize>,
}

pub struct PidHandle(usize);

impl Drop for PidHandle {
    fn drop(&mut self) {
        PID_ALLOCATOR.exclusive_access().dealloc(self.0);
    }
}

impl PidAllocator {
    fn alloc(&mut self) -> PidHandle {
        if let Some(pid) = self.recycled.pop() {
            PidHandle(pid)
        } else {
            let current = self.current;
            self.current += 1;
            PidHandle(current)
        }
    }

    fn dealloc(&mut self, pid: usize) {
        assert!(pid < self.current);
        assert!(
            !self.recycled.iter().any(|&dpid| dpid == pid),
            "pid={} has been deallocated!",
            pid
        );
        self.recycled.push(pid);
    }
}

impl PidHandle {
    pub fn as_raw(&self) -> usize {
        self.0
    }
}
