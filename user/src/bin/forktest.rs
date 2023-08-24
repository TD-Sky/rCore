#![no_std]
#![no_main]

#[macro_use]
extern crate user;

use user::exit;
use user::fork;
use user::wait;

const MAX_CHILD: usize = 30;

#[no_mangle]
pub fn main() -> i32 {
    for i in 0..MAX_CHILD {
        let pid = fork().unwrap();
        if pid == 0 {
            println!("I am child {}", i);
            exit(0);
        } else {
            println!("forked child pid = {}", pid);
        }
        assert!(pid > 0);
    }
    let mut exit_code: i32 = 0;

    for _ in 0..MAX_CHILD {
        if wait(&mut exit_code).is_none() {
            panic!("wait stopped early");
        }
    }

    if wait(&mut exit_code).is_some() {
        panic!("wait got too many");
    }

    println!("forktest pass.");

    0
}
