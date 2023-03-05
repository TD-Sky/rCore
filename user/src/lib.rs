#![no_std]
#![feature(linkage)]
#![feature(panic_info_message)]

#[macro_use]
pub mod console;
mod lang_items;
mod syscall;

use self::syscall::*;

extern "C" {
    fn start_bss();
    fn end_bss();
}

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start() -> ! {
    clear_bss();
    exit(main());
    panic!("unreachable after sys_exit!");
}

// 弱链接会让编译器优先去 bin 目录寻找 main 。
// 若没找到，就链接此 main ，但运行时会立马报错。
#[no_mangle]
#[linkage = "weak"]
fn main() -> i32 {
    panic!("Cannot find main!");
}

fn clear_bss() {
    for addr in start_bss as usize..end_bss as usize {
        unsafe {
            (addr as *mut u8).write_volatile(0);
        }
    }
}

pub fn write(fd: usize, buffer: &[u8]) -> isize {
    sys_write(fd, buffer)
}

pub fn exit(exit_code: i32) -> isize {
    sys_exit(exit_code)
}
