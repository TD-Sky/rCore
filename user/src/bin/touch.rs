#![no_std]
#![no_main]
#![feature(format_args_nl)]

use user::fs::{OpenFlag, open};
use user::println;

#[unsafe(no_mangle)]
fn main(_argc: usize, argv: &[&str]) -> i32 {
    for path in &argv[1..] {
        if open(path, OpenFlag::CREATE.into()).is_none() {
            println!("touch: error when touched `{path}`")
        }
    }
    0
}
