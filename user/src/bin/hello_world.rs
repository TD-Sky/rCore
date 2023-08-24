#![no_std]
#![no_main]

#[macro_use]
extern crate user;

use user::getpid;

#[no_mangle]
pub fn main() -> i32 {
    println!("pid {}: Hello world from user mode program!", getpid());
    0
}
