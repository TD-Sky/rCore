use crate::board::QEMU_EXIT_HANDLE;
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use core::arch::riscv64;
use core::mem;
use core::slice;
use lazy_static::lazy_static;
use qemu_exit::QEMUExit;

const MAX_APP_NUM: usize = 16;
const APP_BASE_ADDRESS: usize = 0x80400000;
const APP_SIZE_LIMIT: usize = 0x20000;

const USER_STACK_SIZE: usize = 4096 * 2; // 8K
const KERNEL_STACK_SIZE: usize = 4096 * 2; // 8K

extern "C" {
    fn _num_app();
    fn __restore(ctx_addr: usize);
}

lazy_static! {
    static ref APP_MANAGER: UPSafeCell<AppManager> = unsafe {
        let num_app_ptr = _num_app as usize as *const usize; // .quad <数量>
        let num_app = num_app_ptr.read_volatile(); // 不经优化，不经缓存的内存I/O
        let mut app_start: [usize; MAX_APP_NUM + 1] = [0; MAX_APP_NUM + 1];
        // 越过<数量>，读取指定数量的元素
        // num_app + 1 是因为要带上最后一个APP的结束地址
        let app_start_raw: &[usize] = slice::from_raw_parts(num_app_ptr.add(1), num_app + 1);
        app_start[..=num_app].copy_from_slice(app_start_raw);

        UPSafeCell::new(AppManager {
            num_app,
            current_app: 0,
            app_start,
        })
    };
}

static KERNEL_STACK: KernelStack = KernelStack {
    data: [0; KERNEL_STACK_SIZE],
};

static USER_STACK: UserStack = UserStack {
    data: [0; USER_STACK_SIZE],
};

struct AppManager {
    num_app: usize,
    current_app: usize,
    app_start: [usize; MAX_APP_NUM + 1],
}

// 为什么要对齐到4KB？
#[repr(align(4096))]
struct KernelStack {
    data: [u8; KERNEL_STACK_SIZE],
}

// 为什么要对齐到4KB？
#[repr(align(4096))]
struct UserStack {
    data: [u8; USER_STACK_SIZE],
}

pub fn init() {
    APP_MANAGER.exclusive_access().print_app_info();
}

pub fn run_next_app() -> ! {
    let mut app_manager = APP_MANAGER.exclusive_access();
    let current_app = app_manager.current_app();

    unsafe {
        // 从数据区将APP复制到指令区
        app_manager.load_app(current_app);
    }

    app_manager.move_next();

    drop(app_manager);

    // 这里是为了复用 __restore 以达到跳到 APP_BASE_ADDRESS 运行APP，
    // 所以才构造一个 TrapContext，
    // 如此一来 sret 就能把里面的内容抽出来给APP运行用
    let ctx = TrapContext::app_init_context(APP_BASE_ADDRESS, USER_STACK.get_sp());
    let ctx_ptr = KERNEL_STACK.push_context(ctx);
    unsafe {
        __restore(ctx_ptr as *const TrapContext as usize);
    }

    panic!("Unreachable in batch::run_current_app!");
}

impl KernelStack {
    /// 由于在 RISC-V 中栈是向下增长的，我们只需返回数组的结尾地址
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + KERNEL_STACK_SIZE
    }

    /// 把 Trap 环境压到内核栈上，但是通过指针手段
    fn push_context(&self, ctx: TrapContext) -> &'static mut TrapContext {
        let ctx_ptr = (self.get_sp() - mem::size_of::<TrapContext>()) as *mut TrapContext;

        unsafe {
            *ctx_ptr = ctx;
            ctx_ptr.as_mut().unwrap()
        }
    }
}

impl UserStack {
    // 对于实例来说其实是个常量
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + USER_STACK_SIZE
    }
}

impl AppManager {
    fn print_app_info(&self) {
        println!("[kernel] num_app = {}", self.num_app);

        for i in 0..self.num_app {
            println!(
                "[kernel] app_{} [{:#x}, {:#x})",
                i,
                self.app_start[i],
                self.app_start[i + 1]
            );
        }
    }

    fn current_app(&self) -> usize {
        self.current_app
    }

    fn move_next(&mut self) {
        self.current_app += 1;
    }

    unsafe fn load_app(&self, app_id: usize) {
        if app_id >= self.num_app {
            println!("All applications completed!");
            QEMU_EXIT_HANDLE.exit_success();
        }

        println!("[kernel] Loading app_{}", app_id);

        // 按字节清零APP的区域
        slice::from_raw_parts_mut(APP_BASE_ADDRESS as *mut u8, APP_SIZE_LIMIT).fill(0);
        let app_src = slice::from_raw_parts(
            self.app_start[app_id] as *const u8, // 从数据区读出APP
            self.app_start[app_id + 1] - self.app_start[app_id], // 一个APP的结束即另一个APP的开始
        );
        let app_dst = slice::from_raw_parts_mut(APP_BASE_ADDRESS as *mut u8, app_src.len());
        app_dst.copy_from_slice(app_src); // 粘到指令区

        // Memory fence about fetching the instruction memory
        // It is guaranteed that a subsequent instruction fetch must
        // observes all previous writes to the instruction memory.
        // Therefore, fence.i must be executed after we have loaded
        // the code of the next app into the instruction memory.
        // See also: riscv non-priv spec chapter 3, 'Zifencei' extension.
        //
        // CPU 对物理内存所做的缓存又分成 数据缓存 和 指令缓存 两部分，
        // 分别在 CPU 访存和取指的时候使用。
        // 在取指的时候，对于一个指令地址，CPU 会先去 指令缓存 里面看一下
        // 它是否在某个已缓存的缓存行内，如果在的话它就会直接从高速缓存中
        // 拿到指令而不是通过总线访问内存。
        // 通常情况下，CPU 会认为程序的代码段不会发生变化，因此 指令缓存
        // 是一种只读缓存。
        // 但在这里，OS 将修改会被 CPU 取指的内存区域，这会使得 指令缓存
        // 中含有与内存中不一致的内容。
        // 因此，OS 在这里必须使用取指屏障指令 fence.i ，它的功能是保证
        // **在它之后的取指过程必须能够看到在它之前的所有对于取指内存区域的修改**，
        // 这样才能保证 CPU 访问的应用代码是最新的而不是 指令缓存 中过时的内容。
        riscv64::fence_i();
    }
}
