//! File and filesystem-related syscalls

use core::mem;

use easy_fs::DirEntry;
use easy_fs::Stat;
use enumflags2::BitFlags;

use crate::fs;
use crate::fs::PipeRingBuffer;
use crate::memory;
use crate::memory::UserBuffer;
use crate::task::processor;

/// try to write `buf` with length `len` to the file with `fd`
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    let process = processor::current_process();
    let process = process.inner().exclusive_access();
    let token = process.user_token();

    if fd >= process.fd_table.len() {
        return -1;
    }

    let Some(file) = &process.fd_table[fd] else {
        return -1;
    };

    if !file.writable() {
        return -1;
    }

    let file = file.clone();
    drop(process);

    file.write(UserBuffer::new(token, buf as *mut u8, len)) as isize
}

/// try to read bytes with length `len` from the file with `fd` to `buf`
pub fn sys_read(fd: usize, buf: *mut u8, len: usize) -> isize {
    let process = processor::current_process();
    let process = process.inner().exclusive_access();
    let token = process.user_token();

    if fd >= process.fd_table.len() {
        return -1;
    }

    let Some(file) = &process.fd_table[fd] else {
        return -1;
    };

    if !file.readable() {
        return -1;
    }

    let file = file.clone();
    drop(process);

    file.read(UserBuffer::new(token, buf, len)) as isize
}

pub fn sys_open(path: *const u8, flags: u32) -> isize {
    let token = processor::current_user_token();
    let path = memory::read_str(token, path);

    let Some(inode) = fs::open_file(&path, BitFlags::from_bits(flags).unwrap()) else {
        return -1;
    };

    let process = processor::current_process();
    let mut process = process.inner().exclusive_access();
    process.fd_table.insert(inode) as isize
}

pub fn sys_close(fd: usize) -> isize {
    let process = processor::current_process();
    let mut inner = process.inner().exclusive_access();

    if fd >= inner.fd_table.len() {
        return -1;
    }

    match inner.fd_table.remove(fd) {
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
    let process = processor::current_process();
    let inner = process.inner().exclusive_access();
    let fd_table = &inner.fd_table;

    if fd >= fd_table.len() {
        log::error!("fd={fd} is outbound");
        return -1;
    }

    match fd_table[fd].as_ref().map(|file| file.stat()) {
        Some(stat) => {
            *memory::read_mut(inner.user_token(), st) = stat;
            0
        }
        None => {
            log::error!("no such file: fd={fd}");
            -1
        }
    }
}

pub fn sys_pipe(pipe: *mut usize) -> isize {
    let process = processor::current_process();
    let mut process = process.inner().exclusive_access();
    let token = process.user_token();

    let (pipe_read, pipe_write) = PipeRingBuffer::make_pipe();
    let read_fd = process.fd_table.insert(pipe_read);
    let write_fd = process.fd_table.insert(pipe_write);
    *memory::read_mut(token, pipe) = read_fd;
    *memory::read_mut(token, unsafe { pipe.add(1) }) = write_fd;

    0
}

// 若读取的对象不是目录，则会产生未定义行为
pub fn sys_getdents(fd: usize, dents: *mut DirEntry, len: usize) -> isize {
    let process = processor::current_process();
    let process = process.inner().exclusive_access();
    let token = process.user_token();

    if fd >= process.fd_table.len() {
        return -1;
    }

    let Some(dir) = &process.fd_table[fd] else {
        return -1;
    };

    if !dir.readable() {
        return -1;
    }

    let dir = dir.clone();
    drop(process);

    let read_byte_count = dir.read(UserBuffer::new(
        token,
        dents.cast(),
        len * mem::size_of::<DirEntry>(),
    ));

    if read_byte_count % mem::size_of::<DirEntry>() != 0 {
        // 读取的字节流没跟 DirEntry 对齐，
        // 说明对象一定不是目录
        return -1;
    }

    (read_byte_count / mem::size_of::<DirEntry>()) as isize
}

pub fn sys_dup(fd: usize) -> isize {
    let process = processor::current_process();
    let mut inner = process.inner().exclusive_access();

    if fd >= inner.fd_table.len() {
        return -1;
    }

    let Some(inode) = inner.fd_table[fd].clone() else {
        return -1;
    };

    inner.fd_table.insert(inode) as isize
}

pub fn sys_eventfd(initval: u64, flags: u32) -> isize {
    let event_fd = fs::eventfd::new(initval, BitFlags::from_bits_truncate(flags));
    let process = processor::current_process();
    let mut process = process.inner().exclusive_access();
    process.fd_table.insert(event_fd) as isize
}
