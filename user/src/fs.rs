use alloc::ffi::CString;
use alloc::string::String;
use alloc::vec;
use core::cmp::Ordering;
use core::mem::MaybeUninit;

use enumflags2::{BitFlags, bitflags};
use vfs::{CDirEntry, Stat};

use crate::io::{read, write};
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
    sys_open(&path, flags.bits()).status()
}

pub fn close(fd: usize) -> Option<()> {
    sys_close(fd).some()
}

pub fn pipe(pipe_fd: &mut [usize]) -> Option<()> {
    sys_pipe(pipe_fd).some()
}

pub fn dup(fd: usize) -> Option<usize> {
    sys_dup(fd).status()
}

pub fn link(old_path: &str, new_path: &str) -> Option<()> {
    let old_path = CString::new(old_path).unwrap();
    let new_path = CString::new(new_path).unwrap();
    sys_link(&old_path, &new_path).some()
}

pub fn unlink(path: &str) -> Option<()> {
    let path = CString::new(path).unwrap();
    sys_unlink(&path).some()
}

pub fn rmdir(path: &str) -> Option<()> {
    let path = CString::new(path).unwrap();
    sys_rmdir(&path).some()
}

pub fn getcwd() -> String {
    const TRY_LEN: usize = 32;
    let mut buf = vec![0; TRY_LEN];
    let len = sys_getcwd(&mut buf, TRY_LEN);
    match len.cmp(&0) {
        Ordering::Greater => {
            buf.truncate(len as usize);
        }
        Ordering::Less => {
            let len = -len as usize;
            buf.resize(len, 0);
            sys_getcwd(&mut buf, len);
        }
        Ordering::Equal => unreachable!(),
    }
    String::from_utf8(buf).expect("Valid UTF-8 CWD")
}

pub fn chdir(path: &str) -> Option<()> {
    let path = CString::new(path).unwrap();
    sys_chdir(&path).some()
}

pub fn mkdir(path: &str) -> Option<()> {
    let path = CString::new(path).unwrap();
    sys_mkdir(&path).some()
}

pub fn fstat(fd: usize) -> Option<Stat> {
    let mut stat = MaybeUninit::zeroed();
    unsafe {
        sys_fstat(fd, stat.as_mut_ptr()).some()?;
        Some(stat.assume_init())
    }
}

pub fn rename(old_path: &str, new_path: &str) -> Option<()> {
    let old_path = CString::new(old_path).ok()?;
    let new_path = CString::new(new_path).ok()?;
    sys_rename(&old_path, &new_path).some()
}

pub fn getdents(fd: usize, dents: &mut [CDirEntry]) -> Option<usize> {
    sys_getdents(fd, dents).status()
}

pub fn eventfd(initval: u64, flags: BitFlags<EventFdFlag>) -> Option<usize> {
    sys_eventfd(initval, flags.bits()).status()
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
