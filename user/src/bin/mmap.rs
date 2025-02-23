#![no_std]
#![no_main]
#![feature(format_args_nl)]

use core::ptr::null;
use user::mem::{ProtectFlag, mmap, munmap};
use user::println;

#[unsafe(no_mangle)]
fn main() -> i32 {
    let data = mmap(null(), 114514, ProtectFlag::R | ProtectFlag::W).unwrap();
    println!("{:#x}", data.as_ptr() as usize);
    println!("{}", data.len());
    munmap(data).unwrap();
    0
}
