#![no_std]
#![no_main]
#![feature(panic_info_message)]
#![feature(stdsimd)]
#![feature(slice_from_ptr_range)]
#![feature(const_trait_impl)]
#![feature(step_trait)]
#![feature(format_args_nl)]
#![feature(riscv_ext_intrinsics)]
#![feature(let_chains)]

extern crate alloc;

#[macro_use]
mod console;

mod collections;
mod config;
mod drivers;
mod fs;
mod lang_items;
mod logging;
mod memory;
mod sbi;
mod stack_trace;
mod sync;
mod syscall;
mod task;
mod timer;
mod trap;

#[path = "boards/qemu.rs"]
mod board;

use core::arch::global_asm;
use core::slice;

global_asm!(include_str!("entry.asm"));

extern "C" {
    fn sbss();
    fn ebss();
}

fn clear_bss() {
    unsafe {
        slice::from_mut_ptr_range(sbss as usize as *mut u8..ebss as usize as *mut u8).fill(0);
    }
}

#[no_mangle]
pub fn rust_main() -> ! {
    clear_bss();
    logging::init();
    memory::init(); // 初始化分页
    task::add_initproc(); // 启动始祖进程
    log::info!("[kernel] initproc started");
    trap::init(); // 设置好 Trap 处理入口
    trap::enable_timer_interrupt();
    timer::set_next_trigger(); // 开始定时
    task::run();

    unreachable!()
}
