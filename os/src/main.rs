#![no_std]
#![no_main]
#![feature(panic_info_message)]
#![feature(stdsimd)]
#![feature(slice_from_ptr_range)]

#[macro_use]
mod console;

mod batch;
mod lang_items;
mod logging;
mod sbi;
mod sync;
mod syscall;
mod trap;

#[path = "boards/qemu.rs"]
mod board;

use core::arch::global_asm;
use log::{debug, error, info, trace, warn};

global_asm!(include_str!("entry.asm"));
global_asm!(include_str!("link_app.S"));

extern "C" {
    fn stext();
    fn etext();
    fn srodata();
    fn erodata();
    fn sdata();
    fn edata();
    fn sbss();
    fn ebss();
    fn boot_stack_lower_bound();
    fn boot_stack_top();
}

fn clear_bss() {
    unsafe {
        core::slice::from_mut_ptr_range(sbss as usize as *mut u8..ebss as usize as *mut u8).fill(0);
    }
}

#[no_mangle]
pub fn rust_main() -> ! {
    clear_bss();

    logging::init();

    trace!(
        "[kernel] .text [{:#x}, {:#x})",
        stext as usize,
        etext as usize
    );
    debug!(
        "[kernel] .rodata [{:#x}, {:#x})",
        srodata as usize, erodata as usize
    );
    info!(
        "[kernel] .data [{:#x}, {:#x})",
        sdata as usize, edata as usize
    );
    warn!(
        "[kernel] boot_stack top=bottom={:#x}, lower_bound={:#x}",
        boot_stack_top as usize, boot_stack_lower_bound as usize
    );
    error!("[kernel] .bss [{:#x}, {:#x})", sbss as usize, ebss as usize);

    trap::init(); // 设置好 Trap 处理入口
    batch::init(); // 打印所有APP信息
    batch::run_next_app(); // 开始加载并运行APP
}
