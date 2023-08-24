#![no_std]
#![no_main]

#[macro_use]
extern crate user;

use user::{fork, getpid, wait};

#[no_mangle]
pub fn main() -> i32 {
    assert!(wait(&mut 0).is_none());
    println!("sys_wait without child process test passed!");
    println!("parent start, pid = {}!", getpid());

    let pid = fork().unwrap();
    if pid == 0 {
        // child process
        println!("hello child process!");

        100
    } else {
        // parent process
        let mut exit_code: i32 = 0;
        println!("ready waiting on parent process!");
        assert_eq!(Some(pid), wait(&mut exit_code));
        assert_eq!(exit_code, 100);
        println!("child process pid = {}, exit code = {}", pid, exit_code);

        0
    }
}
