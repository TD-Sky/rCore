use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use core::mem;

use enumflags2::BitFlags;

use super::manager;
use super::signal::SignalFlag;
use super::RecycleAllocator;
use super::TaskControlBlock;
use crate::collections::SlotVec;
use crate::fs::stdio::{Stdin, Stdout};
use crate::fs::{Directory, File};
use crate::memory::{self, AddressSpace, KERNEL_SPACE};
use crate::sync::{Condvar, Mutex, Semaphore, UpCell};
use crate::trap::{trap_handler, TrapContext};

static PID_ALLOCATOR: UpCell<RecycleAllocator> = UpCell::new(RecycleAllocator::new());

#[derive(Debug)]
pub struct ProcessControlBlock {
    pid: PidHandle,
    inner: UpCell<ProcessControlBlockInner>,
}

/// 进程描述符
#[derive(Debug)]
pub struct PidHandle(usize);

#[derive(Debug)]
pub struct ProcessControlBlockInner {
    pub is_zombie: bool,
    pub address_space: AddressSpace,
    pub parent: Option<Weak<ProcessControlBlock>>,
    /// 子进程，当前进程结束时，它们将被移交给 initproc
    pub children: Vec<Arc<ProcessControlBlock>>,
    pub exit_code: i32,
    /// **文件描述符表**
    // Option 表示文件描述符是否指示着文件
    pub fd_table: SlotVec<Arc<dyn File + Send + Sync>>,
    pub signals: BitFlags<SignalFlag>,
    pub tasks: SlotVec<Arc<TaskControlBlock>>,
    task_resource_allocator: RecycleAllocator,
    pub mutex_list: SlotVec<Arc<dyn Mutex>>,
    pub semaphore_list: SlotVec<Arc<Semaphore>>,
    pub condvar_list: SlotVec<Arc<Condvar>>,
    pub cwd: Directory,
}

impl ProcessControlBlock {
    pub fn inner(&self) -> &UpCell<ProcessControlBlockInner> {
        &self.inner
    }

    pub fn pid(&self) -> usize {
        self.pid.0
    }

    pub fn new(elf_data: &[u8]) -> Arc<Self> {
        let (address_space, ustack_base, entry_point) = AddressSpace::new_user(elf_data);
        let pid_handle = alloc_pid();
        let fds: [Arc<dyn File + Send + Sync>; 3] =
            [Arc::new(Stdin), Arc::new(Stdout), Arc::new(Stdout)];

        let process = Arc::new(Self {
            pid: pid_handle,
            inner: {
                UpCell::new(ProcessControlBlockInner {
                    is_zombie: false,
                    address_space,
                    parent: None,
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table: SlotVec::from_iter(fds),
                    signals: BitFlags::empty(),
                    tasks: SlotVec::new(),
                    task_resource_allocator: RecycleAllocator::default(),
                    mutex_list: SlotVec::new(),
                    semaphore_list: SlotVec::new(),
                    condvar_list: SlotVec::new(),
                    cwd: Directory::fat_root(),
                })
            },
        });

        let task = Arc::new(TaskControlBlock::new(&process, ustack_base, false));
        let task_inner = task.inner().exclusive_access();
        let trap_ctx = task_inner.trap_ctx();
        let ustack_top = task_inner.resource.user_stack_top();
        let kstack_top = task.kernel_stack.top();
        drop(task_inner);

        *trap_ctx = TrapContext::init(
            entry_point,
            ustack_top,
            KERNEL_SPACE.exclusive_access().token(),
            kstack_top,
            trap_handler as usize,
        );

        process.inner.exclusive_access().tasks.push(task.clone());

        manager::insert_process(process.pid(), process.clone());
        manager::add_task(task);

        process
    }

    pub fn fork(self: &Arc<Self>) -> Arc<Self> {
        let mut parent_inner = self.inner().exclusive_access();
        assert_eq!(parent_inner.tasks.len(), 1);

        let child = Arc::new(Self {
            pid: alloc_pid(),
            inner: {
                UpCell::new(ProcessControlBlockInner {
                    is_zombie: false,
                    address_space: parent_inner.address_space.clone(),
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table: parent_inner.fd_table.clone(),
                    signals: BitFlags::empty(),
                    tasks: SlotVec::new(),
                    task_resource_allocator: RecycleAllocator::default(),
                    mutex_list: SlotVec::new(),
                    semaphore_list: SlotVec::new(),
                    condvar_list: SlotVec::new(),
                    cwd: parent_inner.cwd.clone(),
                })
            },
        });
        parent_inner.children.push(child.clone());

        let task = Arc::new(TaskControlBlock::new(
            &child,
            parent_inner
                .tasks
                .get(0)
                .inner()
                .exclusive_access()
                .resource
                .user_stack_base,
            true,
        ));
        child.inner.exclusive_access().tasks.push(task.clone());
        task.inner()
            .exclusive_access()
            .trap_ctx()
            .set_kernel_sp(task.kernel_stack.top());

        manager::insert_process(child.pid(), child.clone());
        manager::add_task(task);

        child
    }

    pub fn exec(self: &Arc<Self>, elf_data: &[u8], args: Vec<String>) {
        assert_eq!(self.inner.exclusive_access().tasks.len(), 1);

        let (addr_space, ustack_base, entry_point) = AddressSpace::new_user(elf_data);
        let token = addr_space.token();
        let mut process = self.inner.exclusive_access();
        process.address_space = addr_space;
        let task = process.tasks.get(0);
        // 待会 TaskResource::alloc 要访问当前进程
        drop(process);

        let mut task_inner = task.inner().exclusive_access();
        task_inner.resource.user_stack_base = ustack_base;
        task_inner.resource.alloc();
        task_inner.trap_ctx_ppn = task_inner.resource.trap_ctx_ppn();
        let mut user_sp = task_inner.resource.user_stack_top();

        log::info!("token={token:#x} original user_sp={user_sp:#x}");
        let argc = args.len();
        // 预备参数的栈空间
        user_sp -= (argc + 1) * mem::size_of::<usize>();
        let argv_base = user_sp;
        log::info!("token={token:#x} argv_base={argv_base:#x}");
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
            log::info!("token={token:#x} arg_addr={ptr:#x}");
            // 将参数写入参数指针所指之处
            memory::write_str(token, arg, **ptr as *mut u8);
        }
        *argv[argc] = 0;
        // make the user_sp aligned to 8B for k210 platform
        user_sp -= user_sp % mem::size_of::<usize>();
        log::info!("token={token:#x} align_at={user_sp:#x}");
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

        let mut trap_ctx = TrapContext::init(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            task.kernel_stack.top(),
            trap_handler as usize,
        );
        *trap_ctx.arg_mut(0) = argc;
        *trap_ctx.arg_mut(1) = argv_base;
        *task_inner.trap_ctx() = trap_ctx;
    }
}

pub fn alloc_pid() -> PidHandle {
    PidHandle(PID_ALLOCATOR.exclusive_access().alloc())
}

impl Drop for PidHandle {
    fn drop(&mut self) {
        PID_ALLOCATOR.exclusive_access().dealloc(self.0);
    }
}

impl ProcessControlBlockInner {
    pub fn user_token(&self) -> usize {
        self.address_space.token()
    }

    pub fn alloc_tid(&mut self) -> usize {
        self.task_resource_allocator.alloc()
    }

    pub fn dealloc_tid(&mut self, tid: usize) {
        self.task_resource_allocator.dealloc(tid);
    }

    pub fn insert_task(&mut self, task: Arc<TaskControlBlock>) {
        let tid = task.inner().exclusive_access().resource.tid;
        self.tasks.insert_kv(tid, task);
    }

    /// 进程结束，但仍要作为子进程等待完全释放，
    /// 故主动释放一部分资源，成为僵尸进程
    pub fn die(&mut self) {
        self.children.clear();
        self.address_space.clear();
        self.fd_table.clear();
    }
}
