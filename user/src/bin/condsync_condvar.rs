#![no_std]
#![no_main]
#![feature(format_args_nl)]

#[macro_use]
extern crate user;

extern crate alloc;

use user::sync::{
    block_mutex, condvar_create, condvar_signal, condvar_wait, mutex_lock, mutex_unlock,
};
use user::thread::{exit, sleep, waittid};

static mut A: usize = 0;

const CONDVAR_ID: usize = 0;
const MUTEX_ID: usize = 0;

unsafe fn first() -> ! {
    sleep(10);
    println!("First work, Change A --> 1 and wakeup Second");
    mutex_lock(MUTEX_ID);
    A = 1;
    condvar_signal(CONDVAR_ID);
    mutex_unlock(MUTEX_ID);
    exit(0)
}

unsafe fn second() -> ! {
    println!("Second want to continue,but need to wait A=1");
    mutex_lock(MUTEX_ID);
    // Mesa语义的实现，中间可能存在其它竞争线程，
    // 因此即使等待完成，条件也不一定满足
    while A == 0 {
        println!("Second: A is {}", A);
        condvar_wait(CONDVAR_ID, MUTEX_ID);
    }
    println!("A is {}, Second can work now", A);
    mutex_unlock(MUTEX_ID);
    exit(0)
}

#[no_mangle]
fn main() -> i32 {
    // create condvar & mutex
    assert_eq!(condvar_create(), CONDVAR_ID);
    assert_eq!(block_mutex(), MUTEX_ID);
    // create threads
    let threads = [
        user::thread::spawn(first as usize, 0),
        user::thread::spawn(second as usize, 0),
    ];
    // wait for all threads to complete
    for thread in threads.iter() {
        waittid(*thread);
    }
    println!("test_condvar passed!");
    0
}
