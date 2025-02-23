#![no_std]
#![no_main]
#![feature(format_args_nl)]

extern crate alloc;

#[macro_use]
extern crate user;
use user::fs::getcwd;

#[unsafe(no_mangle)]
fn main() -> i32 {
    let cwd = getcwd();
    println!("{cwd}");
    0
}
