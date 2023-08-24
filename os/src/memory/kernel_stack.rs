//! 内核空间的用户内核栈

use super::address::VirtAddr;
use super::MapPermission;
use super::KERNEL_SPACE;
use crate::config::{KERNEL_STACK_SIZE, PAGE_SIZE, TRAMPOLINE};
use crate::task::PidHandle;

pub struct KernelStack {
    pid: usize,
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        let (kernel_stack_bottom, _) = KernelStack::range(self.pid);
        KERNEL_SPACE
            .exclusive_access()
            .remove(VirtAddr::from(kernel_stack_bottom).into())
            .unwrap();
    }
}

impl KernelStack {
    pub fn new(pd: &PidHandle) -> Self {
        let pid = pd.as_raw();

        let (kernel_stack_bottom, kernel_stack_top) = KernelStack::range(pid);
        KERNEL_SPACE
            .exclusive_access()
            .insert_framed(
                kernel_stack_bottom.into(),
                kernel_stack_top.into(),
                MapPermission::R | MapPermission::W,
            )
            .unwrap();

        KernelStack { pid }
    }

    pub fn top(&self) -> usize {
        KernelStack::range(self.pid).1
    }
}

impl KernelStack {
    /// 内核空间的高256G，存放着所有应用的内核栈
    fn range(app_id: usize) -> (usize, usize) {
        // 加上 PAGE_SIZE 就是空出了保护页
        let top = TRAMPOLINE - app_id * (KERNEL_STACK_SIZE + PAGE_SIZE);
        let bottom = top - KERNEL_STACK_SIZE;
        (bottom, top)
    }
}
