#![no_std]
#![no_main]
#![feature(format_args_nl)]

#[macro_use]
extern crate user;
extern crate alloc;

use alloc::vec::Vec;
use core::cell::UnsafeCell;
use lazy_static::lazy_static;
use user::sync::{
    condvar_create, condvar_signal, condvar_wait, mutex_lock, mutex_unlock, spin_mutex,
};
use user::thread::{exit, waittid};

const THREAD_NUM: usize = 3;

/// 同步屏障
///
/// 只有全部线程都完成上一阶段之后，它们才能够进入下一阶段
struct Barrier {
    mutex_id: usize,
    condvar_id: usize,
    count: UnsafeCell<usize>,
}

impl Barrier {
    pub fn new() -> Self {
        Self {
            mutex_id: spin_mutex(),
            condvar_id: condvar_create(),
            count: UnsafeCell::new(0),
        }
    }

    pub fn block(&self) {
        mutex_lock(self.mutex_id);
        let count = self.count.get();
        // SAFETY: Here, the accesses of the count is in the
        // critical section protected by the mutex.
        unsafe {
            *count += 1;
        }
        if unsafe { *count } == THREAD_NUM {
            condvar_signal(self.condvar_id);
        } else {
            condvar_wait(self.condvar_id, self.mutex_id);
            condvar_signal(self.condvar_id);
        }
        mutex_unlock(self.mutex_id);
    }
}

unsafe impl Sync for Barrier {}

lazy_static! {
    static ref BARRIER_AB: Barrier = Barrier::new();
    static ref BARRIER_BC: Barrier = Barrier::new();
}

fn thread_fn() {
    for _ in 0..300 {
        print!("a");
    }
    BARRIER_AB.block();
    for _ in 0..300 {
        print!("b");
    }
    BARRIER_BC.block();
    for _ in 0..300 {
        print!("c");
    }
    exit(0)
}

#[no_mangle]
fn main() -> i32 {
    let mut v: Vec<usize> = Vec::new();
    for _ in 0..THREAD_NUM {
        v.push(user::thread::spawn(thread_fn as usize, 0));
    }
    for tid in v {
        waittid(tid);
    }
    println!("\nOK!");
    0
}
