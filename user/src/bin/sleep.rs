#![no_std]
#![no_main]
#![feature(format_args_nl)]

#[macro_use]
extern crate user;

use user::process::{exit, fork, waitpid};
use user::thread::sleep;
use user::time::get_time;

fn sleepy() {
    let time: usize = 100;
    for i in 0..5 {
        sleep(time);
        println!("sleep {} x {} msecs.", i + 1, time);
    }
    exit(0);
}

#[no_mangle]
pub fn main() -> i32 {
    let current_time = get_time();
    let pid = fork();
    let mut exit_code: i32 = 0;
    if pid == 0 {
        sleepy();
    }
    assert!(waitpid(pid, &mut exit_code) == Some(pid) && exit_code == 0);
    println!("use {} msecs.", get_time() - current_time);
    println!("sleep pass.");
    0
}
