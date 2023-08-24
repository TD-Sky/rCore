#![no_std]
#![no_main]
#![feature(format_args_nl)]

use user::fs::remove;
use user::println;

#[no_mangle]
pub fn main(_argc: usize, argv: &[&str]) -> i32 {
    for path in &argv[1..] {
        if remove(path).is_none() {
            println!("rm: {} not found", path);
        }
    }
    0
}
