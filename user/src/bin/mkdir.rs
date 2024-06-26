#![no_std]
#![no_main]
#![feature(format_args_nl)]

use user::fs::mkdir;
use user::println;

#[no_mangle]
fn main(_argc: usize, argv: &[&str]) -> i32 {
    for path in &argv[1..] {
        if mkdir(path).is_none() {
            println!("mkdir: failed to create {path}");
        }
    }
    0
}
