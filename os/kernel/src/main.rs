#![no_std]
#![no_main]
#![feature(panic_info_message)]
#![feature(slice_from_ptr_range)]
#![feature(const_trait_impl)]
#![feature(step_trait)]
#![feature(format_args_nl)]
#![feature(riscv_ext_intrinsics)]
#![feature(let_chains)]
#![feature(const_binary_heap_constructor)]
#![feature(maybe_uninit_as_bytes)]

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
mod ptr;
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

use spin::Lazy;

use crate::drivers::{IOMode, DEV_IO_MODE, GPU_DEVICE, KEYBOARD_DEVICE, MOUSE_DEVICE, SERIAL};

global_asm!(include_str!("entry.S"));

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

    SERIAL.init();

    log::info!("init GPU");
    Lazy::force(&GPU_DEVICE);
    log::info!("init keyboard");
    Lazy::force(&KEYBOARD_DEVICE);
    log::info!("init mouse");
    Lazy::force(&MOUSE_DEVICE);

    log::info!("init trap");
    trap::init(); // 设置好 Trap 处理入口
    trap::enable_timer_interrupt();
    timer::set_next_trigger(); // 开始定时
    board::init_device();

    log::info!("add initproc");
    task::add_initproc(); // 启动始祖进程
    *DEV_IO_MODE.exclusive_access() = IOMode::Interrupt;

    log::info!("run");
    task::run();

    unreachable!()
}
