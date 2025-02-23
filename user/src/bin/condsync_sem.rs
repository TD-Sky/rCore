#![no_std]
#![no_main]
#![feature(format_args_nl)]

#[macro_use]
extern crate user;

extern crate alloc;

use user::sync::{
    block_mutex, mutex_lock, mutex_unlock, semaphore_create, semaphore_down, semaphore_up,
};
use user::thread::{exit, sleep, waittid};

static A: usize = 0;

const SEM_ID: usize = 0;
const MUTEX_ID: usize = 0;

unsafe fn first() -> ! {
    sleep(10);
    println!("First work, Change A --> 1 and wakeup Second");
    mutex_lock(MUTEX_ID);
    unsafe {
        (&raw const A).cast_mut().write(1);
    }
    semaphore_up(SEM_ID);
    mutex_unlock(MUTEX_ID);
    exit(0)
}

unsafe fn second() -> ! {
    println!("Second want to continue,but need to wait A=1");
    loop {
        mutex_lock(MUTEX_ID);
        if A == 0 {
            println!("Second: A is {A}");
            mutex_unlock(MUTEX_ID);
            semaphore_down(SEM_ID);
        } else {
            mutex_unlock(MUTEX_ID);
            break;
        }
    }
    println!("A is {A}, Second can work now");
    exit(0)
}

#[unsafe(no_mangle)]
fn main() -> i32 {
    // create semaphore & mutex
    assert_eq!(semaphore_create(0), SEM_ID);
    assert_eq!(block_mutex(), MUTEX_ID);
    // create threads
    let threads = [
        user::thread::spawn(first as usize, 0),
        user::thread::spawn(second as usize, 0),
    ];
    // wait for all threads to complete
    for thread in threads {
        waittid(thread);
    }
    println!("test_condvar passed!");
    0
}
