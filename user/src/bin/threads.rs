#![no_std]
#![no_main]
#![feature(format_args_nl)]

#[macro_use]
extern crate user;

use user::thread::exit;
use user::thread::waittid;

pub fn thread_a() -> ! {
    for _ in 0..1000 {
        print!("a");
    }
    exit(1)
}

pub fn thread_b() -> ! {
    for _ in 0..1000 {
        print!("b");
    }
    exit(2)
}

pub fn thread_c() -> ! {
    for _ in 0..1000 {
        print!("c");
    }
    exit(3)
}

#[unsafe(no_mangle)]
fn main() -> i32 {
    let v = [
        user::thread::spawn(thread_a as usize, 0),
        user::thread::spawn(thread_b as usize, 0),
        user::thread::spawn(thread_c as usize, 0),
    ];
    for tid in v {
        let exit_code = waittid(tid).unwrap();
        println!("thread#{tid} exited with code {exit_code}");
    }
    println!("main thread exited.");
    0
}
