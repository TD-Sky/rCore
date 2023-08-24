#![no_std]
#![no_main]

use core::ptr::null;
use user::println;
use user::{mmap, munmap, ProtectFlag};

#[no_mangle]
fn main() -> i32 {
    let data = mmap(null(), 114514, ProtectFlag::R | ProtectFlag::W).unwrap();
    println!("{:#x}", data.as_ptr() as usize);
    println!("{}", data.len());
    munmap(data).unwrap();
    0
}
