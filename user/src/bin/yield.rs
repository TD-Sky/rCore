#![no_std]
#![no_main]
#![feature(format_args_nl)]

#[macro_use]
extern crate user;

use user::process::getpid;
use user::thread::yield_;

#[no_mangle]
fn main() -> i32 {
    println!("Hello, I am process {}.", getpid());
    for i in 0..5 {
        yield_();
        println!("Back in process {}, iteration {}.", getpid(), i);
    }
    println!("yield pass.");
    0
}
