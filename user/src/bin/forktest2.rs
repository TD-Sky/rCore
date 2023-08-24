#![no_std]
#![no_main]
#![feature(format_args_nl)]

#[macro_use]
extern crate user;
use user::process::*;
use user::thread::sleep;
use user::time::get_time;

static NUM: usize = 30;

#[no_mangle]
pub fn main() -> i32 {
    for _ in 0..NUM {
        let pid = fork();
        if pid == 0 {
            let current_time = get_time();
            let sleep_length =
                (current_time as i32 as isize) * (current_time as i32 as isize) % 1000 + 1000;
            println!("pid {} sleep for {} ms", getpid(), sleep_length);
            sleep(sleep_length as usize);
            println!("pid {} OK!", getpid());
            exit(0);
        }
    }

    let mut exit_code: i32 = 0;
    for _ in 0..NUM {
        assert!(wait(&mut exit_code).is_some());
        assert_eq!(exit_code, 0);
    }
    assert!(wait(&mut exit_code).is_none());
    println!("forktest2 test passed!");
    0
}
