#![no_std]
#![no_main]
#![feature(panic_info_message)]

mod console;
mod lang_items;
mod logging;
mod sbi;

#[path = "boards/qemu.rs"]
mod board;

use crate::board::QEMU_EXIT_HANDLE;
use core::arch::global_asm;
use log::{debug, error, info, trace, warn};
use qemu_exit::QEMUExit;

global_asm!(include_str!("entry.asm"));

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

    println!("Hello, World");
    QEMU_EXIT_HANDLE.exit_success()
}

fn clear_bss() {
    unsafe {
        for addr in sbss as usize..ebss as usize {
            (addr as *mut u8).write_volatile(0)
        }
    }
}
