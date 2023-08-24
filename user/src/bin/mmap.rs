#![no_std]
#![no_main]
#![feature(format_args_nl)]

use core::ptr::null;
use user::mem::{mmap, munmap, ProtectFlag};
use user::println;

#[no_mangle]
fn main() -> i32 {
    let data = mmap(null(), 114514, ProtectFlag::R | ProtectFlag::W).unwrap();
    println!("{:#x}", data.as_ptr() as usize);
    println!("{}", data.len());
    munmap(data).unwrap();
    0
}
