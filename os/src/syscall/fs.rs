//! File and filesystem-related syscalls

use easy_fs::Stat;
use enumflags2::BitFlags;
use log::debug;
use log::error;

use crate::fs;
use crate::memory;
use crate::memory::Buffer;
use crate::task::processor;

/// try to write `buf` with length `len` to the file with `fd`
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    let token = processor::current_user_token();
    let task = processor::current_task().unwrap();
    let inner = task.inner().exclusive_access();

    if fd >= inner.fd_table().len() {
        return -1;
    }

    let Some(file) = &inner.fd_table()[fd] else {
        return -1;
    };

    if !file.writable() {
        return -1;
    }

    let file = file.clone();
    drop(inner);

    file.write(Buffer::new(memory::read_bytes(token, buf, len))) as isize
}

/// try to read bytes with length `len` from the file with `fd` to `buf`
pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    let token = processor::current_user_token();
    let task = processor::current_task().unwrap();
    let inner = task.inner().exclusive_access();

    if fd >= inner.fd_table().len() {
        return -1;
    }

    let Some(file) = &inner.fd_table()[fd] else {
        return -1;
    };

    if !file.readable() {
        return -1;
    }

    let file = file.clone();
    drop(inner);

    file.read(Buffer::new(memory::read_bytes(token, buf, len))) as isize
}

pub fn sys_open(path: *const u8, flags: u32) -> isize {
    let token = processor::current_user_token();
    let task = processor::current_task().unwrap();
    let path = memory::read_str(token, path);

    let Some(inode) = fs::open_file(&path, BitFlags::from_bits(flags).unwrap()) else {
        return -1;
    };

    let mut inner = task.inner().exclusive_access();
    inner.alloc_fd(inode) as isize
}

pub fn sys_close(fd: usize) -> isize {
    let task = processor::current_task().unwrap();
    let mut inner = task.inner().exclusive_access();

    if fd >= inner.fd_table().len() {
        return -1;
    }

    match inner.dealloc_fd(fd) {
        Some(_) => 0,
        None => -1,
    }
}

pub fn sys_linkat(oldpath: *const u8, newpath: *const u8) -> isize {
    let token = processor::current_user_token();
    let oldpath = memory::read_str(token, oldpath);
    let newpath = memory::read_str(token, newpath);

    match fs::link_at(&oldpath, &newpath) {
        Some(_) => 0,
        None => -1,
    }
}

pub fn sys_unlinkat(path: *const u8) -> isize {
    let token = processor::current_user_token();
    let path = memory::read_str(token, path);

    match fs::unlink_at(&path) {
        Some(_) => 0,
        None => -1,
    }
}

pub fn sys_fstat(fd: usize, st: *mut Stat) -> isize {
    debug!("fd={fd}");

    let task = processor::current_task().unwrap();
    let inner = task.inner().exclusive_access();
    let fd_table = inner.fd_table();

    if fd >= fd_table.len() {
        error!("fd={fd} is outbound");
        return -1;
    }

    match fd_table[fd].as_ref().map(|file| file.stat()) {
        Some(stat) => {
            *memory::read_mut(inner.token(), st) = stat;
            0
        }
        None => {
            error!("no such file: fd={fd}");
            -1
        }
    }
}
