//! 内核空间的用户内核栈

use lazy_static::lazy_static;

use super::address::VirtAddr;
use super::MapPermission;
use super::KERNEL_SPACE;
use crate::config::{KERNEL_STACK_SIZE, PAGE_SIZE, TRAMPOLINE};
use crate::sync::UPSafeCell;
use crate::task::RecycleAllocator;

lazy_static! {
    static ref KSTACK_ALLOCATOR: UPSafeCell<RecycleAllocator> =
        unsafe { UPSafeCell::new(RecycleAllocator::default()) };
}

pub struct KernelStack(usize);

/// 分配任务的内核栈
pub fn alloc_kernel_stack() -> KernelStack {
    let kid = KSTACK_ALLOCATOR.exclusive_access().alloc();
    let (bottom, top) = KernelStack::range(kid);
    KERNEL_SPACE
        .exclusive_access()
        .insert_framed(
            bottom.into(),
            top.into(),
            MapPermission::R | MapPermission::W,
        )
        .unwrap();
    KernelStack(kid)
}

pub fn kernel_token() -> usize {
    KERNEL_SPACE.exclusive_access().token()
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        let kernel_stack_bottom: VirtAddr = KernelStack::range(self.0).0.into();
        KERNEL_SPACE
            .exclusive_access()
            .remove(kernel_stack_bottom.into())
            .unwrap();
        KSTACK_ALLOCATOR.exclusive_access().dealloc(self.0);
    }
}

impl KernelStack {
    pub fn top(&self) -> usize {
        KernelStack::range(self.0).1
    }
}

impl KernelStack {
    /// 内核空间的高256G，存放着所有应用的内核栈
    fn range(kid: usize) -> (usize, usize) {
        // 加上 PAGE_SIZE 就是空出了保护页
        let top = TRAMPOLINE - kid * (KERNEL_STACK_SIZE + PAGE_SIZE);
        let bottom = top - KERNEL_STACK_SIZE;
        (bottom, top)
    }
}
