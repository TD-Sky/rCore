//! 应用程序通过 ecall 进入到内核状态时，
//! 操作系统保存被打断的应用程序的 Trap上下文 到内核栈上；
//! 在操作系统完成系统调用服务后，
//! 需要恢复被打断的应用程序的 Trap上下文，
//! 并通 sret 让应用程序继续执行。

use riscv::register::sstatus;
use riscv::register::sstatus::Sstatus;
use riscv::register::sstatus::SPP;

// |  trap_handler |
// |   kernel_sp   |
// |   kernel_satp |
// |     sepc      |
// |   sstatus     |
// |     x31       |
// |     ...       |
// |     ...       |
// |    x2/sp      |
// |     x1        |
// |     x0        |
#[repr(C)]
pub struct TrapContext {
    /// 所有通用寄存器，x0 ~ x31
    x: [usize; 32],
    /// 中断使能 及 各种杂七杂八的状态
    sstatus: Sstatus,
    /// Supervisor Exception PC, 指向出现异常的指令
    pub(super) sepc: usize,
    /* 以下都是地址，因为太多了 sscratch 放不下，就放上下文里了 */
    /// 内核页表
    kernel_satp: usize,
    /// 内核栈
    kernel_sp: usize,
    /// Trap处理函数
    trap_handler: usize,
}

impl TrapContext {
    fn set_sp(&mut self, sp: usize) {
        self.x[2] = sp;
    }
}

impl TrapContext {
    /// 捏造上下文以复用`__restore`
    pub fn init(
        entry: usize,
        sp: usize,
        kernel_satp: usize,
        kernel_sp: usize,
        trap_handler: usize,
    ) -> Self {
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
            kernel_satp,
            kernel_sp,
            trap_handler,
        };

        ctx.set_sp(sp);
        ctx
    }

    pub fn set_kernel_sp(&mut self, kernel_sp: usize) {
        self.kernel_sp = kernel_sp;
    }

    /// 凭借ABI索引访问参数寄存器
    pub fn a(&self, n: usize) -> usize {
        self.x[n + 10]
    }

    /// 设置系统调用的结果
    pub fn set_a0(&mut self, a0: usize) {
        self.x[10] = a0;
    }
}
