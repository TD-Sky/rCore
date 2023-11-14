#![no_std]
#![no_main]
#![feature(format_args_nl)]

#[macro_use]
extern crate user;

extern crate alloc;

use alloc::vec::Vec;
use core::ptr;

use user::sync::{semaphore_create, semaphore_down, semaphore_up};
use user::thread::{exit, waittid};

const SEM_MUTEX: usize = 0;
const SEM_EMPTY: usize = 1;
const SEM_AVAIL: usize = 2;
const BUFFER_SIZE: usize = 8;
static mut BUFFER: [usize; BUFFER_SIZE] = [0; BUFFER_SIZE];
static mut FRONT: usize = 0;
static mut TAIL: usize = 0;
const PRODUCER_COUNT: usize = 4;
const NUMBER_PER_PRODUCER: usize = 100;

unsafe fn producer(id: *const usize) -> ! {
    let id = *id;
    for _ in 0..NUMBER_PER_PRODUCER {
        semaphore_down(SEM_EMPTY);
        semaphore_down(SEM_MUTEX);
        BUFFER[TAIL] = id;
        TAIL = (TAIL + 1) % BUFFER_SIZE;
        semaphore_up(SEM_MUTEX);
        semaphore_up(SEM_AVAIL);
    }
    exit(0)
}

unsafe fn consumer() -> ! {
    for _ in 0..PRODUCER_COUNT * NUMBER_PER_PRODUCER {
        semaphore_down(SEM_AVAIL);
        semaphore_down(SEM_MUTEX);
        print!("{} ", BUFFER[FRONT]);
        FRONT = (FRONT + 1) % BUFFER_SIZE;
        semaphore_up(SEM_MUTEX);
        semaphore_up(SEM_EMPTY);
    }
    println!("");
    exit(0)
}

#[no_mangle]
fn main() -> i32 {
    // create semaphores
    assert_eq!(semaphore_create(1), SEM_MUTEX);
    assert_eq!(semaphore_create(BUFFER_SIZE), SEM_EMPTY);
    assert_eq!(semaphore_create(0), SEM_AVAIL);
    // create threads
    let ids: Vec<_> = (0..PRODUCER_COUNT).collect();
    let mut threads = Vec::new();
    for i in 0..PRODUCER_COUNT {
        threads.push(user::thread::spawn(
            producer as usize,
            ptr::from_ref(&ids.as_slice()[i]) as usize,
        ));
    }
    threads.push(user::thread::spawn(consumer as usize, 0));
    // wait for all threads to complete
    for thread in threads {
        waittid(thread);
    }
    println!("mpsc_sem passed!");
    0
}
