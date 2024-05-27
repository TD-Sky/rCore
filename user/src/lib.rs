#![no_std]
#![feature(linkage)]
#![feature(panic_info_message)]
#![feature(never_type)]
#![feature(format_args_nl)]

#[macro_use]
pub mod console;
pub mod fs;
pub mod graph;
pub mod io;
mod lang_items;
pub mod mem;
pub mod process;
pub mod signal;
pub mod sync;
mod syscall;
pub mod thread;
pub mod time;

extern crate alloc;

use self::thread::exit;
use alloc::vec::Vec;
use buddy_system_allocator::LockedHeap;
use core::slice;

/// 分配的堆空间
const USER_HEAP_SIZE: usize = 2usize.pow(20);

static mut HEAP_SPACE: [u8; USER_HEAP_SIZE] = [0; USER_HEAP_SIZE];

#[global_allocator]
static HEAP: LockedHeap<32> = LockedHeap::empty();

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start(argc: usize, argv: usize) -> ! {
    unsafe {
        HEAP.lock()
            .init(HEAP_SPACE.as_ptr() as usize, USER_HEAP_SIZE);
    }

    let argv = argv as *const usize;
    let argv: Vec<_> = (0..argc)
        .map(|i| {
            let ptr = unsafe { argv.add(i).read_volatile() } as *const u8;
            let len = (0..)
                .find(|&i| unsafe { ptr.add(i).read_volatile() == b'\0' })
                .unwrap();
            core::str::from_utf8(unsafe { slice::from_raw_parts(ptr, len) }).unwrap()
        })
        .collect();

    exit(main(argc, &argv))
}

// 弱链接会让编译器优先去 bin 目录寻找 main 。
// 若没找到，就链接此 main ，但运行时会立马报错。
#[no_mangle]
#[linkage = "weak"]
fn main(_argc: usize, _argv: &[&str]) -> i32 {
    panic!("Cannot find main!");
}

#[inline]
fn status2option(status: isize) -> Option<usize> {
    (status >= 0).then_some(status as usize)
}
