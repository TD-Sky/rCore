use alloc::sync::Arc;
use alloc::sync::Weak;

use super::ProcessControlBlock;
use super::TaskContext;
use crate::config::{PAGE_SIZE, TRAP_CONTEXT_BASE, USER_STACK_SIZE};
use crate::memory::address::PhysPageNum;
use crate::memory::address::VirtAddr;
use crate::memory::alloc_kernel_stack;
use crate::memory::KernelStack;
use crate::memory::MapPermission;
use crate::sync::UpCell;
use crate::trap::TrapContext;

pub struct TaskControlBlock {
    // immutable
    pub process: Weak<ProcessControlBlock>,
    pub kernel_stack: KernelStack,
    // mutable
    inner: UpCell<TaskControlBlockInner>,
}

pub struct TaskControlBlockInner {
    pub resource: TaskUserResource,
    pub(super) trap_ctx_ppn: PhysPageNum,
    pub(super) ctx: TaskContext,
    pub(super) status: TaskStatus,
    pub exit_code: Option<i32>,
}

/// 线程资源：线程ID 与 用户栈
// 进程用于 分配/释放 资源
pub struct TaskUserResource {
    pub tid: usize,
    pub user_stack_base: usize,
    process: Weak<ProcessControlBlock>,
}

#[derive(PartialEq, Eq)]
pub enum TaskStatus {
    Ready,
    Running,
    Blocked,
}

impl TaskControlBlock {
    pub fn new(
        process: &Arc<ProcessControlBlock>,
        user_stack_base: usize,
        is_forking: bool,
    ) -> Self {
        let resource = TaskUserResource {
            tid: process.inner().exclusive_access().alloc_tid(),
            user_stack_base,
            process: Arc::downgrade(process),
        };

        // fork时创建新任务，不映射用户栈和trap上下文
        if !is_forking {
            resource.alloc();
        }

        let trap_ctx_ppn = resource.trap_ctx_ppn();
        let kernel_stack = alloc_kernel_stack();
        let kstack_top = kernel_stack.top();

        Self {
            process: Arc::downgrade(process),
            kernel_stack,
            inner: {
                UpCell::new(TaskControlBlockInner {
                    resource,
                    trap_ctx_ppn,
                    ctx: TaskContext::new(kstack_top),
                    status: TaskStatus::Ready,
                    exit_code: None,
                })
            },
        }
    }

    pub fn inner(&self) -> &UpCell<TaskControlBlockInner> {
        &self.inner
    }
}

impl TaskControlBlockInner {
    pub fn trap_ctx(&self) -> &'static mut TrapContext {
        self.trap_ctx_ppn.as_mut()
    }
}

impl TaskUserResource {
    pub fn alloc(&self) {
        let process = self.process.upgrade().unwrap();
        let mut inner = process.inner().exclusive_access();

        let (ustack_bottom, ustack_top) = user_stack_range(self.user_stack_base, self.tid);
        inner
            .address_space
            .insert_framed(
                ustack_bottom.into(),
                ustack_top.into(),
                MapPermission::R | MapPermission::W | MapPermission::U,
            )
            .unwrap();

        let (trap_ctx_bottom, trap_ctx_top) = trap_ctx_range(self.tid);
        inner
            .address_space
            .insert_framed(
                trap_ctx_bottom.into(),
                trap_ctx_top.into(),
                MapPermission::R | MapPermission::W,
            )
            .unwrap();
    }

    pub fn dealloc(&self) {
        let process = self.process.upgrade().unwrap();
        let mut process = process.inner().exclusive_access();

        process.dealloc_tid(self.tid);

        let ustack_bottom: VirtAddr = user_stack_range(self.user_stack_base, self.tid).0.into();
        process.address_space.remove(ustack_bottom.into()).unwrap();

        let trap_ctx_bottom: VirtAddr = trap_ctx_range(self.tid).0.into();
        process
            .address_space
            .remove(trap_ctx_bottom.into())
            .unwrap();
    }

    pub fn user_stack_top(&self) -> usize {
        user_stack_range(self.user_stack_base, self.tid).1
    }

    pub fn trap_ctx_ppn(&self) -> PhysPageNum {
        let trap_ctx_bottom: VirtAddr = trap_ctx_range(self.tid).0.into();
        self.process
            .upgrade()
            .unwrap()
            .inner()
            .exclusive_access()
            .address_space
            .translate(trap_ctx_bottom)
            .unwrap()
            .ppn()
    }

    pub fn trap_ctx_user_va(&self) -> usize {
        trap_ctx_range(self.tid).0
    }
}

fn user_stack_range(base: usize, tid: usize) -> (usize, usize) {
    let bottom = base + tid * (PAGE_SIZE + USER_STACK_SIZE);
    let top = bottom + USER_STACK_SIZE;
    (bottom, top)
}

fn trap_ctx_range(tid: usize) -> (usize, usize) {
    let bottom = TRAP_CONTEXT_BASE - tid * PAGE_SIZE;
    let top = bottom + PAGE_SIZE;
    (bottom, top)
}
