use alloc::ffi::CString;
use easy_fs::{DirEntry, Stat};
use enumflags2::{bitflags, BitFlags};

use crate::io::{read, write};
use crate::status2option;
use crate::syscall::*;

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

    #[inline]
    pub fn read_only() -> BitFlags<OpenFlag> {
        BitFlags::from_bits_truncate(Self::RDONLY)
    }
}

#[allow(clippy::upper_case_acronyms)]
#[bitflags]
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventFdFlag {
    SEMAPHORE = 0b0000_0000_0001,
    NONBLOCK = 0b1000_0000_0000,
}

pub fn open(path: &str, flags: BitFlags<OpenFlag>) -> Option<usize> {
    let path = CString::new(path).unwrap();
    status2option(sys_open(&path, flags.bits()))
}

pub fn close(fd: usize) -> Option<()> {
    sys_close(fd).eq(&0).then_some(())
}

pub fn pipe(pipe_fd: &mut [usize]) -> Option<()> {
    sys_pipe(pipe_fd).eq(&0).then_some(())
}

pub fn dup(fd: usize) -> Option<usize> {
    status2option(sys_dup(fd))
}

pub fn link_at(old_path: &str, new_path: &str) -> Option<()> {
    let old_path = CString::new(old_path).unwrap();
    let new_path = CString::new(new_path).unwrap();
    sys_linkat(&old_path, &new_path).eq(&0).then_some(())
}

pub fn remove(path: &str) -> Option<()> {
    let path = CString::new(path).unwrap();
    sys_unlinkat(&path).eq(&0).then_some(())
}

pub fn fstat(fd: usize) -> Option<Stat> {
    let mut stat = Stat::default();
    sys_fstat(fd, &mut stat).eq(&0).then_some(stat)
}

pub fn getdents(fd: usize, dents: &mut [DirEntry]) -> Option<usize> {
    status2option(sys_getdents(fd, dents))
}

pub fn eventfd(initval: u64, flags: BitFlags<EventFdFlag>) -> Option<usize> {
    status2option(sys_eventfd(initval, flags.bits()))
}

pub fn eventfd_read(fd: usize) -> Option<u64> {
    let mut num = [0u8; 8];
    read(fd, &mut num)?;
    Some(u64::from_ne_bytes(num))
}

pub fn eventfd_write(fd: usize, num: u64) -> Option<()> {
    write(fd, &num.to_ne_bytes())?;
    Some(())
}
