#![no_std]
#![no_main]
#![feature(format_args_nl)]

#[macro_use]
extern crate user;
extern crate alloc;

use alloc::vec::Vec;
use user::thread::{self, exit, waittid};

struct Argument {
    pub ch: char,
    pub rc: i32,
}

fn thread_print(arg: *const Argument) -> ! {
    let arg = unsafe { &*arg };
    for _ in 0..1000 {
        print!("{}", arg.ch);
    }
    exit(arg.rc)
}

#[no_mangle]
fn main() -> i32 {
    let mut v = Vec::new();
    let args = [
        Argument { ch: 'a', rc: 1 },
        Argument { ch: 'b', rc: 2 },
        Argument { ch: 'c', rc: 3 },
    ];
    for arg in &args {
        v.push(thread::spawn(
            thread_print as usize,
            arg as *const _ as usize,
        ));
    }
    for tid in v {
        let exit_code = waittid(tid).unwrap();
        println!("thread#{tid} exited with code {exit_code}");
    }
    println!("main thread exited.");
    0
}
