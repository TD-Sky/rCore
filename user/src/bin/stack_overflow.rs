#![no_std]
#![no_main]
#![feature(format_args_nl)]

#[macro_use]
extern crate user;

#[allow(unconditional_recursion)]
fn f(depth: usize) {
    if depth % 10 == 0 {
        println!("depth = {}", depth);
    }
    f(depth + 1);
}

#[no_mangle]
fn main() -> i32 {
    println!("It should trigger segmentation fault!");
    f(0);
    0
}
