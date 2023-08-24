//! 加载时缺页异常：给定虚拟地址，找不到对应的物理帧

#![no_std]
#![no_main]
#![allow(clippy::invalid_null_ptr_usage)]

use core::ptr::null;
use user::println;

#[no_mangle]
fn main() -> i32 {
    println!("\nload_fault APP running...\n");
    println!("Into Test load_fault, we will insert an invalid load operation...");
    println!("Kernel should kill this application!");
    unsafe {
        let _ = null::<u8>().read_volatile();
    }
    0
}
