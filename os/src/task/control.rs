use core::cmp::Ordering;

use alloc::sync::{Arc, Weak};
use alloc::vec;
use alloc::vec::Vec;
use enumflags2::BitFlags;

use crate::config::{MMAP_OFFSET_FROM, PAGE_SIZE, TRAP_CONTEXT};
use crate::fs::File;
use crate::fs::{stdio::*, OSInode};
use crate::memory::address::{PhysPageNum, VirtAddr};
use crate::memory::kernel_stack::KernelStack;
use crate::memory::{AddressSpace, KERNEL_SPACE};
use crate::stopwatch;
use crate::sync::UPSafeCell;
use crate::trap::trap_handler;
use crate::trap::TrapContext;

use super::pid;
use super::PidHandle;
use super::TaskContext;

pub struct TaskControlBlock {
    pid: PidHandle,
    kernel_stack: KernelStack,
    inner: UPSafeCell<TaskControlBlockInner>,
}

pub struct TaskControlBlockInner {
    // 进程
    pub(super) task_status: TaskStatus,
    pub(super) parent: Option<Weak<TaskControlBlock>>,
    /// 子进程，当前进程结束时，它们将被移交给 initproc
    pub(super) children: Vec<Arc<TaskControlBlock>>,
    exit_code: i32,
    // 内存
    pub(super) task_ctx: TaskContext,
    address_space: AddressSpace,
    trap_ctx_ppn: PhysPageNum,
    /// 应用数据的大小
    base_size: usize,
    heap_bottom: usize,
    mmap_bottom: usize,
    program_brk: usize,
    /// **文件描述符表**
    // Option 表示文件描述符是否指示着文件
    fd_table: Vec<Option<Arc<dyn File + Send + Sync>>>,
    // 计时
    pub(super) user_time: usize,
    pub(super) kernel_time: usize,
}

#[derive(PartialEq, Eq)]
pub enum TaskStatus {
    Ready,
    Running,
    Zombie,
}

impl TaskControlBlock {
    pub fn new(elf_data: &[u8]) -> Self {
        let (address_space, user_sp, entry_point) = AddressSpace::new_user(elf_data);
        let trap_ctx_ppn = address_space
            .translate(VirtAddr::from(TRAP_CONTEXT))
            .unwrap()
            .ppn();

        let pid_handle = pid::alloc();
        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.top();

        let trap_ctx: &mut TrapContext = trap_ctx_ppn.as_mut();
        *trap_ctx = TrapContext::init(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );

        Self {
            pid: pid_handle,
            kernel_stack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    task_status: TaskStatus::Ready,
                    parent: None,
                    children: Vec::new(),
                    exit_code: 0,
                    task_ctx: TaskContext::new(kernel_stack_top),
                    address_space,
                    trap_ctx_ppn,
                    base_size: user_sp,
                    heap_bottom: user_sp,
                    mmap_bottom: user_sp + MMAP_OFFSET_FROM,
                    program_brk: user_sp,
                    user_time: 0,
                    kernel_time: 0,
                    fd_table: vec![
                        // 0 -> stdin
                        Some(Arc::new(Stdin)),
                        // 1 -> stdout
                        Some(Arc::new(Stdout)),
                        // 2 -> stderr
                        Some(Arc::new(Stdout)),
                    ],
                })
            },
        }
    }

    pub fn inner(&self) -> &UPSafeCell<TaskControlBlockInner> {
        &self.inner
    }

    pub fn pid(&self) -> usize {
        self.pid.as_raw()
    }

    pub fn fork(self: &Arc<Self>) -> Arc<Self> {
        let mut parent_inner = self.inner().exclusive_access();

        let address_space = parent_inner.address_space.clone();

        let pid = pid::alloc();
        let kernel_stack = KernelStack::new(&pid);
        let kernel_stack_top = kernel_stack.top();

        let trap_ctx_ppn = address_space
            .translate(VirtAddr::from(TRAP_CONTEXT))
            .unwrap()
            .ppn();
        let trap_ctx: &mut TrapContext = trap_ctx_ppn.as_mut();
        trap_ctx.set_kernel_sp(kernel_stack_top);

        let tcb = Arc::new(TaskControlBlock {
            pid,
            kernel_stack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    task_status: TaskStatus::Ready,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: 0,
                    task_ctx: TaskContext::new(kernel_stack_top),
                    address_space,
                    trap_ctx_ppn,
                    base_size: parent_inner.base_size,
                    heap_bottom: parent_inner.heap_bottom,
                    mmap_bottom: parent_inner.mmap_bottom,
                    program_brk: parent_inner.program_brk,
                    user_time: parent_inner.user_time,
                    kernel_time: parent_inner.kernel_time,
                    fd_table: parent_inner.fd_table.clone(),
                })
            },
        });

        parent_inner.children.push(tcb.clone());

        tcb
    }

    pub fn exec(&self, elf_data: &[u8]) {
        let (addr_space, user_sp, entry_point) = AddressSpace::new_user(elf_data);

        let trap_ctx_ppn = addr_space
            .translate(VirtAddr::from(TRAP_CONTEXT))
            .unwrap()
            .ppn();
        let trap_ctx = trap_ctx_ppn.as_mut();
        *trap_ctx = TrapContext::init(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            self.kernel_stack.top(),
            trap_handler as usize,
        );

        let mut task_inner = self.inner().exclusive_access();
        task_inner.address_space = addr_space;
        task_inner.base_size = user_sp;
        task_inner.heap_bottom = user_sp;
        task_inner.mmap_bottom = user_sp + MMAP_OFFSET_FROM;
        task_inner.program_brk = user_sp;
        task_inner.user_time = 0;
        task_inner.kernel_time = 0;
        task_inner.trap_ctx_ppn = trap_ctx_ppn;
    }

    /// 给定区域映射一部分虚拟内存
    pub fn mmap(&self, start: usize, len: usize, prot: u8) -> Option<usize> {
        let mut task_inner = self.inner().exclusive_access();

        let suggested_start = if start > task_inner.mmap_bottom {
            // start未跟页大小对齐，失败
            if start % PAGE_SIZE != 0 {
                return None;
            }

            start
        } else {
            task_inner.mmap_bottom
        };

        // 只取关键的三位；
        // sys_mmap 的标志位不是 MapPermission 的子集，要左移才行
        let prot = BitFlags::from_bits_truncate((prot << 1) & 0b0000_1110);
        // 不包含有效标志位，失败
        if prot.is_empty() {
            return None;
        }

        // 若范围包含已映射页面，则失败
        task_inner
            .address_space
            .insert_mmap(suggested_start.into(), len, prot)
            .map(VirtAddr::into)
            .ok()
    }

    /// 解除mmap型的映射
    pub fn munmap(&self, start: usize, _len: usize) -> Option<()> {
        // start未跟页大小对齐，失败
        if start % PAGE_SIZE != 0 {
            return None;
        }

        let mut task_inner = self.inner().exclusive_access();
        task_inner
            .address_space
            .remove_mmap(VirtAddr::from(start).into())
            .ok()
    }

    /// 改变堆顶(高位)的位置
    pub fn change_program_brk(&self, size: i32) -> Option<usize> {
        let mut task_inner = self.inner().exclusive_access();
        let heap_bottom = task_inner.heap_bottom;

        let old_brk = task_inner.program_brk;
        let new_brk = old_brk.checked_add_signed(size as isize).unwrap_or(0);

        if new_brk < heap_bottom {
            return None;
        }

        match size.cmp(&0) {
            Ordering::Less => task_inner
                .address_space
                .shrink_to(VirtAddr::from_raw(heap_bottom), VirtAddr::from_raw(new_brk))
                .ok(),
            Ordering::Greater => task_inner
                .address_space
                .expand_to(VirtAddr::from_raw(heap_bottom), VirtAddr::from_raw(new_brk))
                .ok(),
            Ordering::Equal => Some(()), // 不做任何操作
        }
        .map(|()| {
            task_inner.program_brk = new_brk;
            old_brk
        })
    }
}

impl TaskControlBlockInner {
    #[inline]
    pub fn token(&self) -> usize {
        self.address_space.token()
    }

    /// 通过物理页号访问上下文，故可以跨越地址空间
    #[inline]
    pub fn trap_ctx(&self) -> &'static mut TrapContext {
        self.trap_ctx_ppn.as_mut()
    }

    #[inline]
    pub fn exit_code(&self) -> i32 {
        self.exit_code
    }

    #[inline]
    pub fn children(&self) -> &[Arc<TaskControlBlock>] {
        &self.children
    }

    #[inline]
    pub fn is_zombie(&self) -> bool {
        self.task_status == TaskStatus::Zombie
    }

    #[inline]
    pub fn set_parent(&mut self, parent: Weak<TaskControlBlock>) {
        self.parent = Some(parent);
    }

    #[inline]
    pub fn add_child(&mut self, child: Arc<TaskControlBlock>) {
        self.children.push(child);
    }

    #[inline]
    pub fn remove_child(&mut self, index: usize) -> Arc<TaskControlBlock> {
        self.children.remove(index)
    }

    /// 进程结束，但仍要作为子进程等待完全释放，
    /// 故主动释放一部分资源，成为僵尸进程
    pub fn die(&mut self, exit_code: i32) {
        self.task_status = TaskStatus::Zombie;
        self.exit_code = exit_code;
        self.children.clear();
        self.address_space.clear();
        self.kernel_time += stopwatch::refresh();
    }

    #[inline]
    pub fn fd_table(&self) -> &[Option<Arc<dyn File + Send + Sync>>] {
        &self.fd_table
    }

    /// 为 inode 分配文件描述符并返回
    pub fn alloc_fd(&mut self, inode: Arc<OSInode>) -> usize {
        let fd = self
            .fd_table
            .iter()
            .position(Option::is_none)
            .unwrap_or_else(|| {
                self.fd_table.push(None);
                self.fd_table.len() - 1
            });
        self.fd_table[fd] = Some(inode);
        fd
    }

    #[inline]
    pub fn dealloc_fd(&mut self, fd: usize) -> Option<Arc<dyn File + Send + Sync>> {
        // 该线性表只增不减，留空位置以便复用
        self.fd_table[fd].take()
    }
}
