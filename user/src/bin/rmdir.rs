#![no_std]
#![no_main]
#![feature(format_args_nl)]

use user::fs::rmdir;
use user::println;

#[unsafe(no_mangle)]
fn main(_argc: usize, argv: &[&str]) -> i32 {
    for path in &argv[1..] {
        if rmdir(path).is_none() {
            println!("rm: {path} not found, or isn't empty directory");
        }
    }
    0
}
