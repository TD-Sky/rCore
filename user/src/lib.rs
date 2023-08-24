#![no_std]
#![feature(linkage)]
#![feature(panic_info_message)]
#![feature(never_type)]

#[macro_use]
pub mod console;

mod lang_items;

mod syscall;
use self::syscall::*;

use buddy_system_allocator::LockedHeap;
use core::ptr::NonNull;
use core::slice;
use enumflags2::{bitflags, BitFlags};

/// 16KB 的堆空间
const USER_HEAP_SIZE: usize = 0x4000;

static mut HEAP_SPACE: [u8; USER_HEAP_SIZE] = [0; USER_HEAP_SIZE];

#[global_allocator]
static HEAP: LockedHeap<32> = LockedHeap::empty();

#[bitflags]
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenFlag {
    /// 只写
    WRONLY = 0b0000_0000_0001,
    /// 读写兼备
    RDWR = 0b0000_0000_0010,
    /// 创建文件，若文件存在则清空
    CREATE = 0b0010_0000_0000,
    /// 先清空文件，再交给用户
    TRUNC = 0b0100_0000_0000,
}

impl OpenFlag {
    // enumflags2拒绝值为0的标志
    /// 只读
    pub const RDONLY: u32 = 0b0000_0000_0000;
}

#[bitflags]
#[repr(u8)]
#[derive(Clone, Copy)]
pub enum ProtectFlag {
    R = 0b0000_0001,
    W = 0b0000_0010,
    X = 0b0000_0100,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct Stat {
    pub dev: u64,
    pub inode: u64,
    pub kind: StatKind,
    pub links: u32,
    pad: [u64; 7],
}

#[bitflags]
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatKind {
    DIR = 0o040000,
    #[default]
    FILE = 0o100000,
}

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start() -> ! {
    unsafe {
        HEAP.lock()
            .init(HEAP_SPACE.as_ptr() as usize, USER_HEAP_SIZE);
    }
    exit(main())
}

// 弱链接会让编译器优先去 bin 目录寻找 main 。
// 若没找到，就链接此 main ，但运行时会立马报错。
#[no_mangle]
#[linkage = "weak"]
fn main() -> i32 {
    panic!("Cannot find main!");
}

pub fn open(path: &str, flags: BitFlags<OpenFlag>) -> Option<usize> {
    match sys_open(path, flags.bits()) {
        -1 => None,
        fd => Some(fd as usize),
    }
}

pub fn close(fd: usize) -> Option<()> {
    (sys_close(fd) > -1).then_some(())
}

pub fn read(fd: usize, buf: &mut [u8]) -> Option<usize> {
    match sys_read(fd, buf) {
        -1 => None,
        read_size => Some(read_size as usize),
    }
}

pub fn write(fd: usize, buf: &[u8]) -> Option<usize> {
    match sys_write(fd, buf) {
        -1 => None,
        write_size => Some(write_size as usize),
    }
}

pub fn link_at(old_path: &str, new_path: &str) -> Option<()> {
    (sys_linkat(old_path, new_path) == 0).then_some(())
}

pub fn remove(path: &str) -> Option<()> {
    (sys_unlinkat(path) == 0).then_some(())
}

pub fn fstat(fd: usize) -> Option<Stat> {
    let mut stat = Stat::default();
    (sys_fstat(fd, &mut stat) == 0).then_some(stat)
}

pub fn exit(exit_code: i32) -> ! {
    sys_exit(exit_code)
}

pub fn yield_() -> isize {
    sys_yield()
}

pub fn get_time() -> isize {
    sys_get_time()
}

pub fn sbrk(size: i32) -> Option<NonNull<u8>> {
    match sys_sbrk(size) {
        -1 => None,
        old_brk => NonNull::new(old_brk as usize as *mut u8),
    }
}

pub fn mmap(
    start: *const u8,
    len: usize,
    prot: impl Into<BitFlags<ProtectFlag>>,
) -> Option<&'static mut [u8]> {
    match sys_mmap(start as usize, len, prot.into().bits()) {
        -1 => None,
        mmap_start => unsafe {
            Some(slice::from_raw_parts_mut(
                mmap_start as usize as *mut u8,
                len,
            ))
        },
    }
}

pub fn munmap(area: &mut [u8]) -> Option<()> {
    match sys_munmap(area.as_mut_ptr() as usize, area.len()) {
        -1 => None,
        _ => Some(()),
    }
}

pub fn getpid() -> isize {
    sys_getpid()
}

pub fn fork() -> Option<usize> {
    match sys_fork() {
        -1 => None,
        0 => Some(0),
        subpid => Some(subpid as usize),
    }
}

/// 结果：
/// None => 程序不存在
pub fn exec(path: &str) -> Option<!> {
    match sys_exec(path) {
        -1 => None,
        _ => unreachable!(),
    }
}

/// 等待任意一个子进程结束
pub fn wait(exit_code: &mut i32) -> Option<usize> {
    loop {
        // -1 是约定参数
        match sys_waitpid(-1, exit_code) {
            -2 => {
                yield_();
            }
            -1 => return None,
            exit_pid => return Some(exit_pid as usize),
        }
    }
}

/// 等待指定子进程结束
pub fn waitpid(pid: usize, exit_code: &mut i32) -> Option<usize> {
    loop {
        // -1 是约定参数
        match sys_waitpid(pid as isize, exit_code) {
            -2 => {
                yield_();
            }
            // - 没有子进程
            // - 指定子进程存在但尚未结束
            -1 => return None,
            exit_pid => return Some(exit_pid as usize),
        }
    }
}

/// 睡眠指定ms长时间
pub fn sleep(period: usize) {
    let period = period as isize;
    let start = sys_get_time();

    while sys_get_time() < start + period {
        sys_yield();
    }
}

pub fn spawn(path: &str) -> Option<usize> {
    match sys_spawn(path) {
        -1 => None,
        pid => Some(pid as usize),
    }
}
