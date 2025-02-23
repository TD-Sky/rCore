use core::ptr::NonNull;
use core::slice;

use crate::syscall::*;

use enumflags2::{BitFlags, bitflags};

#[bitflags]
#[repr(u8)]
#[derive(Clone, Copy)]
pub enum ProtectFlag {
    R = 0b0000_0001,
    W = 0b0000_0010,
    X = 0b0000_0100,
}

pub fn sbrk(size: i32) -> Option<NonNull<u8>> {
    sys_sbrk(size)
        .status()
        .and_then(|old_brk| NonNull::new(old_brk as *mut u8))
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
