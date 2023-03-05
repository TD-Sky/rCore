#![no_std]
#![no_main]

use user::println;
use core::ptr;

#[no_mangle]
fn main() -> i32 {
    println!("Into Test store_fault, we will insert an invalid store operation...");
    println!("Kernel should kill this application!");
    unsafe {
        ptr::null_mut::<u8>().write_volatile(0);
    }
    0
}
