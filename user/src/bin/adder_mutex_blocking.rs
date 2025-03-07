#![no_std]
#![no_main]
#![feature(format_args_nl)]

#[macro_use]
extern crate user;
extern crate alloc;

use alloc::vec::Vec;

use user::sync::*;
use user::thread::{self, exit, waittid};
use user::time::get_time;

static A: usize = 0;
const PER_THREAD_DEFAULT: usize = 10000;
const THREAD_COUNT_DEFAULT: usize = 16;
static PER_THREAD: usize = 0;

unsafe fn critical_section(t: &mut usize) {
    let a = &raw const A;
    let cur = unsafe { a.read_volatile() };
    for _ in 0..500 {
        *t = (*t) * (*t) % 10007;
    }
    unsafe {
        a.cast_mut().write_volatile(cur + 1);
    }
}

unsafe fn f() -> ! {
    let mut t = 2usize;
    for _ in 0..PER_THREAD {
        mutex_lock(0);
        unsafe {
            critical_section(&mut t);
        }
        mutex_unlock(0);
    }
    exit(t as i32)
}

#[unsafe(no_mangle)]
fn main(argc: usize, argv: &[&str]) -> i32 {
    let mut thread_count = THREAD_COUNT_DEFAULT;
    let mut per_thread = PER_THREAD_DEFAULT;
    if argc >= 2 {
        thread_count = argv[1].parse().unwrap();
        if argc >= 3 {
            per_thread = argv[2].parse().unwrap();
        }
    }
    unsafe {
        (&raw const PER_THREAD).cast_mut().write(per_thread);
    }

    let start = get_time();
    assert_eq!(block_mutex(), 0);
    let mut v = Vec::new();
    for _ in 0..thread_count {
        v.push(thread::spawn(f as usize, 0) as usize);
    }
    for tid in v {
        waittid(tid);
    }
    println!("time cost is {}ms", get_time() - start);
    assert_eq!(A, PER_THREAD * thread_count);
    0
}
