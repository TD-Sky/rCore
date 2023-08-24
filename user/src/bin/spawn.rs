#![no_std]
#![no_main]
#![feature(format_args_nl)]

use user::process::{spawn, waitpid};

#[macro_use]
extern crate user;

#[no_mangle]
fn main() -> i32 {
    let child = "matrix\0";
    let sub_pid = spawn(child).unwrap();
    let mut xstate = 0;

    println!("spawn new process pid={}", sub_pid);
    assert_eq!(waitpid(sub_pid, &mut xstate), Some(sub_pid));
    assert_eq!(xstate, 0);

    0
}
