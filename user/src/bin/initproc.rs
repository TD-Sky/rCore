#![no_std]
#![no_main]

use user::println;
use user::yield_;
use user::{exec, fork, wait};

#[no_mangle]
fn main() -> i32 {
    if fork().unwrap() == 0 {
        // 启动shell
        exec("user_shell\0");
    } else {
        loop {
            let mut exit_code = 0;

            match wait(&mut exit_code) {
                None => {
                    yield_();
                }
                Some(pid) => {
                    println!(
                        "[initproc] Released a zombie process, pid={}, exit_code={}",
                        pid, exit_code
                    );
                }
            }
        }
    }

    0
}
