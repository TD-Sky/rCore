use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::mem;

use enumflags2::BitFlags;
use log::info;

use crate::config::{MMAP_OFFSET_FROM, PAGE_SIZE, TRAP_CONTEXT};
use crate::fs::stdio::*;
use crate::fs::File;
use crate::memory::address::{PhysPageNum, VirtAddr};
use crate::memory::kernel_stack::KernelStack;
use crate::memory::{self, AddressSpace, KERNEL_SPACE};
use crate::stopwatch;
use crate::sync::UPSafeCell;
use crate::trap::trap_handler;
use crate::trap::TrapContext;

use super::signal::{SignalAction, SignalFlag};
use super::PidHandle;
use super::TaskContext;
use super::{pid, signal};

pub struct TaskControlBlock {
    pid: PidHandle,
    kernel_stack: KernelStack,
    inner: UPSafeCell<TaskControlBlockInner>,
}

pub struct TaskControlBlockInner {
    // 进程
    pub(super) task_status: TaskStatus,
    pub parent: Option<Weak<TaskControlBlock>>,
    /// 子进程，当前进程结束时，它们将被移交给 initproc
    pub children: Vec<Arc<TaskControlBlock>>,
    pub exit_code: i32,
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
    pub fd_table: Vec<Option<Arc<dyn File + Send + Sync>>>,
    // 信号
    pub signals: BitFlags<SignalFlag>,
    pub signal_mask: BitFlags<SignalFlag>,
    pub handling_signal: Option<u32>,
    pub sigactions: [SignalAction; signal::COUNT],
    pub(super) killed: bool,
    pub(super) frozen: bool,
    pub trap_ctx_backup: Option<TrapContext>,
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
                    fd_table: vec![
                        // 0 -> stdin
                        Some(Arc::new(Stdin)),
                        // 1 -> stdout
                        Some(Arc::new(Stdout)),
                        // 2 -> stderr
                        Some(Arc::new(Stdout)),
                    ],
                    signals: BitFlags::empty(),
                    signal_mask: BitFlags::empty(),
                    handling_signal: None,
                    sigactions: Default::default(),
                    killed: false,
                    frozen: false,
                    trap_ctx_backup: None,
                    user_time: 0,
                    kernel_time: 0,
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
                    fd_table: parent_inner.fd_table.clone(),
                    signals: BitFlags::empty(),
                    signal_mask: parent_inner.signal_mask,
                    handling_signal: None,
                    sigactions: parent_inner.sigactions.clone(),
                    killed: false,
                    frozen: false,
                    trap_ctx_backup: None,
                    user_time: parent_inner.user_time,
                    kernel_time: parent_inner.kernel_time,
                })
            },
        });

        parent_inner.children.push(tcb.clone());

        tcb
    }

    pub fn exec(&self, elf_data: &[u8], args: Vec<String>) {
        let (addr_space, mut user_sp, entry_point) = AddressSpace::new_user(elf_data);

        let token = addr_space.token();
        info!("token={token:#x} original user_sp={user_sp:#x}");
        let argc = args.len();
        // 预备参数的栈空间
        user_sp -= (argc + 1) * mem::size_of::<usize>();
        let argv_base = user_sp;
        info!("token={token:#x} argv_base={argv_base:#x}");
        // 参数指针列表，指向用户栈，起始于`argv_base`，
        // 多拾取一个槽位用于放置空指针作为列表终止符
        let mut argv: Vec<&'static mut usize> = (0..=argc)
            .map(|i| {
                memory::read_mut(
                    token,
                    (argv_base + i * mem::size_of::<usize>()) as *mut usize,
                )
            })
            .collect();
        for (arg, ptr) in args.iter().zip(&mut argv[..argc]) {
            // 压栈
            user_sp -= arg.len() + 1;
            // 第一次解指针跨越了Vec
            **ptr = user_sp;
            info!("token={token:#x} arg_addr={ptr:#x}");
            // 将参数写入参数指针所指之处
            memory::write_str(token, arg, **ptr as *mut u8);
        }
        *argv[argc] = 0;
        // make the user_sp aligned to 8B for k210 platform
        user_sp -= user_sp % mem::size_of::<usize>();
        info!("token={token:#x} align_at={user_sp:#x}");
        /*
         * 参数栈空间
         *
         *              HighAddr
         *            |    0    |
         *            | argv[n] |
         *            |  ....   |
         *            | argv[1] |
         *            | argv[0] |
         *            ----------- <- argv_base
         *            |  '\0'   |
         *            |  '\a'   |
         *            |  '\a'   |
         * argv[0] -> -----------
         *            |  '\0'   |
         *            |  '\b'   |
         * argv[1] -> -----------
         *            |  ....   |
         *            |  Align  |
         *            ----------- <- user_sp
         *              LowAddr
         */

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
        *trap_ctx.arg_mut(0) = argc;
        *trap_ctx.arg_mut(1) = argv_base;

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
    pub fn is_zombie(&self) -> bool {
        self.task_status == TaskStatus::Zombie
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

    /// 为 inode 分配文件描述符并返回
    pub fn alloc_fd(&mut self, inode: Arc<dyn File + Send + Sync>) -> usize {
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

    /// 休眠状态：
    /// 进程停止执行但尚未被杀死，等待SIGCONT的通知
    #[inline]
    pub fn is_hibernating(&self) -> bool {
        self.frozen && !self.killed
    }

    pub fn kernel_signal_handler(&mut self, signal: SignalFlag) {
        if signal == SignalFlag::SIGSTOP {
            self.frozen = true;
            self.signals ^= signal;
        } else if signal == SignalFlag::SIGCONT {
            if self.signals.contains(signal) {
                self.signals ^= signal;
                self.frozen = false;
            }
        } else {
            self.killed = true;
        }
    }

    pub fn user_signal_handler(&mut self, signum: usize, signal: SignalFlag) {
        let handler = self.sigactions[signum].handler;
        // 检查进程是否提供了该信号的处理例程
        if handler != 0 {
            // user handler

            // handle flag
            self.handling_signal = Some(signum as u32);
            // 清除该信号位
            self.signals ^= signal;

            // backup trapframe
            let trap_ctx = self.trap_ctx();
            self.trap_ctx_backup = Some(trap_ctx.clone());

            // 修改 sepc 为预设的例程地址，令Trap返回用户态后跳转到例程入口
            trap_ctx.set_sepc(handler);

            // 修改 a0 寄存器，使信号类型作为参数传入例程
            *trap_ctx.arg_mut(0) = signum;
        } else {
            // default action
            println!("[K] task/user_signal_handler: default action: ignore it or kill process");
        }
    }
}
