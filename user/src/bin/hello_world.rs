#![no_std]
#![no_main]
#![feature(format_args_nl)]

#[macro_use]
extern crate user;
use user::process::getpid;

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("pid {}: Hello world from user mode program!", getpid());
    0
}
