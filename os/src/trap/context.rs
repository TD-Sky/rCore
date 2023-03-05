//! 应用程序通过 ecall 进入到内核状态时，
//! 操作系统保存被打断的应用程序的 Trap环境 到内核栈上；
//! 在操作系统完成系统调用服务后，
//! 需要恢复被打断的应用程序的 Trap环境，
//! 并通 sret 让应用程序继续执行。

use riscv::register::sstatus;
use riscv::register::sstatus::Sstatus;
use riscv::register::sstatus::SPP;

// C布局，精确尺寸为 34 字节
//
// |     sepc      |
// |   sstatus     |
// |     x31       |
// |     ...       |
// |     ...       |
// | sscratch / x2 |
// |     x1        |
// |     x0        |
#[repr(C)]
pub struct TrapContext {
    pub x: [usize; 32],   // 所有通用寄存器，x0 ~ x31
    pub sstatus: Sstatus, // 中断使能 及 各种杂七杂八的状态
    pub sepc: usize,      // Supervisor Exception PC, 指向出现异常的指令
}

impl TrapContext {
    fn set_sp(&mut self, sp: usize) {
        self.x[2] = sp;
    }
}

impl TrapContext {
    // 这个方法是专门给 run_next_app 内复用 __restore 用的
    pub fn app_init_context(entry: usize, sp: usize) -> Self {
        // 因为稍后 __restore 的时候会加载 TrapContext 的 sstatus，
        // 所以改变 CSR 这个也没关系
        unsafe {
            sstatus::set_spp(SPP::User);
        }
        let sstatus = sstatus::read();
        let mut ctx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry,
        };

        ctx.set_sp(sp);
        ctx
    }
}
