#![no_std]
#![no_main]
#![feature(format_args_nl)]

use core::ptr;

use user::println;
use user::process::{exec, fork, wait};
use user::thread::yield_;

#[no_mangle]
fn main() -> i32 {
    if fork() == 0 {
        // 启动shell
        exec("user_shell\0", &[ptr::null()]);
    } else {
        loop {
            let mut exit_code = 0;

            match wait(&mut exit_code) {
                None => {
                    yield_();
                }
                Some(pid) => {
                    println!(
                        "[initproc] Released a zombie process, pid={pid}, exit_code={exit_code}",
                    );
                }
            }
        }
    }

    0
}
