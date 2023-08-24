#![no_std]
#![no_main]

#[macro_use]
extern crate user;

use user::exit;
use user::fork;
use user::yield_;
use user::{wait, waitpid};

const MAGIC: i32 = -0x10384;

#[no_mangle]
pub fn main() -> i32 {
    println!("I am the parent. Forking the child...");
    let pid = fork().unwrap();
    if pid == 0 {
        println!("I am the child.");
        for _ in 0..7 {
            yield_();
        }
        exit(MAGIC);
    } else {
        println!("I am parent, fork a child pid {}", pid);
    }

    println!("I am the parent, waiting now..");
    let mut xstate: i32 = 0;
    assert!(waitpid(pid, &mut xstate) == Some(pid) && xstate == MAGIC);
    // 等待所有子进程退出
    assert!(wait(&mut xstate).is_none());
    println!("waitpid {} ok.", pid);
    println!("exit pass.");

    0
}
