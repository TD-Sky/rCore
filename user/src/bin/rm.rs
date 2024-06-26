#![no_std]
#![no_main]
#![feature(format_args_nl)]

use user::fs::unlink;
use user::println;

#[no_mangle]
fn main(_argc: usize, argv: &[&str]) -> i32 {
    for path in &argv[1..] {
        if unlink(path).is_none() {
            println!("rm: {path} not found, or isn't file");
        }
    }
    0
}
