//! 存储时缺页异常：给定虚拟地址，找不到对应的物理帧

#![no_std]
#![no_main]
#![feature(format_args_nl)]

use core::ptr::null_mut;
use user::println;

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("\nstore_fault APP running...\n");
    println!("Into Test store_fault, we will insert an invalid store operation...");
    println!("Kernel should kill this application!");
    unsafe {
        null_mut::<u8>().write_volatile(1);
    }
    0
}
