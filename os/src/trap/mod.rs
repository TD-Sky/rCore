mod context;
pub use self::context::TrapContext;

use crate::batch::run_next_app;
use crate::syscall::syscall;
use core::arch::global_asm;
use riscv::register::scause;
use riscv::register::scause::Exception;
use riscv::register::scause::Trap;
use riscv::register::stval;
use riscv::register::stvec;
use riscv::register::utvec::TrapMode;

global_asm!(include_str!("trap.S"));

extern "C" {
    fn __alltraps();
}

/// 初始化 CSR `stvec` 为 `__alltraps` 的入口
pub fn init() {
    unsafe {
        // Supervisor Trap Vector
        // 当异常发生时，PC应该跳转的地址
        stvec::write(__alltraps as usize, TrapMode::Direct);
    }
}

// 这个 TrapContext 是在汇编里手动构造的
#[no_mangle]
pub fn trap_handler(ctx: &mut TrapContext) -> &mut TrapContext {
    // Supervisor Exception Casue
    // 记录发生的异常
    let scause = scause::read();

    // Supervisor Trap Value
    // | 地址异常 => 该地址
    // | 非法指令异常 => 该指令
    // | _ => 0
    let stval = stval::read();

    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            // ecall 指令触发的异常，
            // 希望 sepc 可以指向 ecall 的下一条指令
            // (RISC-V 64 指令长度不超过 32 位)
            ctx.sepc += 4;
            ctx.x[10] = syscall(ctx.x[17], [ctx.x[10], ctx.x[11], ctx.x[12]]) as usize;
        }

        Trap::Exception(Exception::StoreFault) | Trap::Exception(Exception::StorePageFault) => {
            println!("[kernel] PageFault in application, kernel killed it.");
            run_next_app();
        }

        Trap::Exception(Exception::IllegalInstruction) => {
            println!("[kernel] IllegalInstruction in application, kernel killed it.");
            run_next_app();
        }

        _ => panic!(
            "Unsupported trap {:?}, stval = {:#x}!",
            scause.cause(),
            stval
        ),
    }

    ctx
}
