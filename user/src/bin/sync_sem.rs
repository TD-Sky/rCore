#![no_std]
#![no_main]
#![feature(format_args_nl)]

#[macro_use]
extern crate user;

extern crate alloc;

use user::sync::{semaphore_create, semaphore_down, semaphore_up};
use user::thread::exit;
use user::thread::{sleep, waittid};

const SEM_SYNC: usize = 0;

unsafe fn first() -> ! {
    sleep(5 * 1000);
    println!("First work and wakeup Second");
    semaphore_up(SEM_SYNC);
    exit(0)
}

unsafe fn second() -> ! {
    println!("Second want to continue,but need to wait first");
    semaphore_down(SEM_SYNC);
    println!("Second can work now");
    exit(0)
}

#[no_mangle]
fn main() -> i32 {
    // create semaphores
    assert_eq!(semaphore_create(0), SEM_SYNC);
    // create threads
    let threads = [
        user::thread::spawn(first as usize, 0),
        user::thread::spawn(second as usize, 0),
    ];
    // wait for all threads to complete
    for thread in threads.iter() {
        waittid(*thread);
    }
    println!("sync_sem passed!");
    0
}
