//! 应用Trap进内核时，硬件会设置一些CSR，在S特权级下跳转到__alltraps
//!
//! 嵌套Trap：
//! 指处理一个Trap（可能是中断或异常）的过程中再次发生Trap。
//! 在内核开发时我们需要仔细权衡哪些嵌套Trap应当被允许存在，
//! 哪些嵌套Trap又应该被禁止，这会关系到内核的执行模型。
//!
//! 嵌套中断：嵌套Trap的一个特例。
//! 默认情况下，在软件开始响应中断前，
//! 硬件会自动禁用所有同特权级中断，
//! 故而不会再次触发中断导致嵌套中断了。
//!
//! NOTE: stvec(Supervisor Trap Vector)：当异常发生时，PC应该跳转的地址

mod context;
pub use self::context::TrapContext;

use core::arch::asm;
use core::arch::global_asm;

use riscv::register::scause;
use riscv::register::scause::Exception;
use riscv::register::scause::Interrupt;
use riscv::register::scause::Trap;
use riscv::register::sie;
use riscv::register::stval;
use riscv::register::stvec;
use riscv::register::utvec::TrapMode;

use crate::config::TRAMPOLINE;
use crate::config::TRAP_CONTEXT;
use crate::syscall::syscall;
use crate::task;
use crate::task::processor;
use crate::task::signal::SignalFlag;
use crate::task::switch::update_switch_time;
use crate::timer;

global_asm!(include_str!("trap.S"));

extern "C" {
    fn __alltraps();
    fn __restore();
}

pub fn init() {
    set_kernel_trap_entry();
}

#[no_mangle]
fn trap_from_kernel() -> ! {
    panic!("A trap from kernel!");
}

fn set_kernel_trap_entry() {
    unsafe {
        stvec::write(trap_from_kernel as usize, TrapMode::Direct);
    }
}

/// 设置 stvec 为跳板地址
fn set_user_trap_entry() {
    unsafe {
        stvec::write(TRAMPOLINE, TrapMode::Direct);
    }
}

// 这个 TrapContext 是在汇编里手动构造的
#[no_mangle]
pub fn trap_handler() -> ! {
    set_kernel_trap_entry();
    task::user_time_end();
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
            // Trap上下文不在内核地址空间内，要间接获取
            let ctx = processor::current_trap_ctx();
            // ecall 指令触发的异常，
            // 希望 sepc 可以指向 ecall 的下一条指令
            // (RISC-V 64 指令长度不超过 32 位)
            ctx.sepc += 4;
            let result = syscall(ctx.arg(7), [ctx.arg(0), ctx.arg(1), ctx.arg(2)]);

            // 原来的Trap上下文在 sys_exec 时被回收，需获取新的Trap上下文
            let ctx = processor::current_trap_ctx();
            ctx.set_syscall_result(result as usize);
        }

        // 某些异常会令内核给进程发送信号，
        // 这就是异步信号的由来，即异步异常的传染
        Trap::Exception(
            Exception::StoreFault
            | Exception::StorePageFault
            | Exception::LoadFault
            | Exception::LoadPageFault
            | Exception::InstructionFault
            | Exception::InstructionPageFault,
        ) => {
            task::send_signal_to_current(SignalFlag::SIGSEGV);
        }

        Trap::Exception(Exception::IllegalInstruction) => {
            task::send_signal_to_current(SignalFlag::SIGILL);
        }

        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            timer::set_next_trigger();
            task::suspend_current_and_run_next();
        }

        _ => panic!(
            "Unsupported trap {:?}, stval = {:#x}!",
            scause.cause(),
            stval
        ),
    }

    task::handle_signals();

    if let Some((errno, msg)) = task::check_current_signal_error() {
        log::error!("[kernel] {msg}");
        task::exit_current_and_run_next(errno);
    }

    task::user_time_start();
    trap_return();
}

/// timer interrupt enabled
pub fn enable_timer_interrupt() {
    // 我们并没有将应用初始 Trap上下文 的 sstatus 的 SPIE 设为1，
    // 这将意味着CPU在用户态执行应用的时候 sstatus 的 SIE 为0。
    //
    // 根据定义来说，此时的CPU会屏蔽 S态 所有中断，
    // 自然也包括 S特权级 时钟中断。
    // 但是可以观察到应用经历一个时间片后仍被正常打断，
    // 这是因为当CPU在 U态 接收到一个 S态 时钟中断时会被抢占，
    // 这时无论 SIE 位是否被设置都会进入 Trap 处理流程。
    unsafe {
        sie::set_stimer();
    }
}

/// 结束Trap处理环节，跳转到恢复函数
#[no_mangle]
pub fn trap_return() -> ! {
    set_user_trap_entry();

    // TRAMPOLINE 运行时地址
    // __restore  编译时地址
    // __alltraps 编译时地址
    //
    // TRAMPOLINE加上二者的差得到运行时的__restore
    let restore_va = TRAMPOLINE + (__restore as usize - __alltraps as usize);
    let trap_ctx_ptr = TRAP_CONTEXT;
    let user_satp = processor::current_user_token();

    // 在内核中进行的一些操作可能导致一些
    // 原先存放某个应用代码的物理页帧
    // 如今用来存放数据或者是其他应用的代码，
    // i-cache中可能还保存着该物理页帧的错误快照，
    // 故需要清除缓存。
    unsafe {
        asm!(
            "fence.i",
            "jr {restore_va}",
            restore_va = in(reg) restore_va,
            in("a0") trap_ctx_ptr,
            in("a1") user_satp,
            options(noreturn)
        );
    }
}

/// 新任务由此处进入用户态
#[no_mangle]
pub fn first_restore() -> ! {
    update_switch_time();
    trap_return()
}
