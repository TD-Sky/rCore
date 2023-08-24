use core::arch::asm;

use crate::Stat;

const UNLINKAT: usize = 35;
const LINKAT: usize = 37;
const OPEN: usize = 56;
const CLOSE: usize = 57;
const READ: usize = 63;
const WRITE: usize = 64;
const FSTAT: usize = 80;
const EXIT: usize = 93;
const YIELD: usize = 124;
const TIME: usize = 169;
const GETPID: usize = 172;
const SBRK: usize = 214;
const MUNMAP: usize = 215;
const FORK: usize = 220;
const EXEC: usize = 221;
const MMAP: usize = 222;
const WAITPID: usize = 260;
const SPAWN: usize = 400;

fn syscall(id: usize, args: [usize; 3]) -> isize {
    let mut ret;
    unsafe {
        asm!(
            "ecall",
            inlateout("x10") args[0] => ret,
            in("x11") args[1],
            in("x12") args[2],
            in("x17") id
        );
    }

    ret
}

pub fn sys_open(path: &str, flags: u32) -> isize {
    syscall(OPEN, [path.as_ptr() as usize, flags as usize, 0])
}

pub fn sys_close(fd: usize) -> isize {
    syscall(CLOSE, [fd, 0, 0])
}

pub fn sys_read(fd: usize, buffer: &mut [u8]) -> isize {
    syscall(READ, [fd, buffer.as_mut_ptr() as usize, buffer.len()])
}

pub fn sys_write(fd: usize, buffer: &[u8]) -> isize {
    syscall(WRITE, [fd, buffer.as_ptr() as usize, buffer.len()])
}

pub fn sys_exit(exit_code: i32) -> ! {
    syscall(EXIT, [exit_code as usize, 0, 0]);
    panic!("sys_exit never returns!")
}

pub fn sys_yield() -> isize {
    syscall(YIELD, [0, 0, 0])
}

pub fn sys_get_time() -> isize {
    syscall(TIME, [0, 0, 0])
}

pub fn sys_sbrk(size: i32) -> isize {
    // 有符号数转无符号数，会直接写补码，
    // 因此再转回有符号数是无损的
    syscall(SBRK, [size as usize, 0, 0])
}

pub fn sys_mmap(start: usize, len: usize, prot: u8) -> isize {
    syscall(MMAP, [start, len, prot as usize])
}

pub fn sys_munmap(start: usize, len: usize) -> isize {
    syscall(MUNMAP, [start, len, 0])
}

pub fn sys_getpid() -> isize {
    syscall(GETPID, [0, 0, 0])
}

/// 结果：
/// * 0 => 当前在子进程
/// * PID => 创建的子进程ID，且当前在父进程
pub fn sys_fork() -> isize {
    syscall(FORK, [0, 0, 0])
}

pub fn sys_exec(path: &str) -> isize {
    syscall(EXEC, [path.as_ptr() as usize, 0, 0])
}

/// 结果：
/// * PID => 结束子进程的ID
/// * -2 => 子进程存在，但尚未退出
/// * -1 => 发生错误，例如子进程不存在
pub fn sys_waitpid(pid: isize, exit_code: *mut i32) -> isize {
    syscall(WAITPID, [pid as usize, exit_code as usize, 0])
}

pub fn sys_spawn(path: &str) -> isize {
    syscall(SPAWN, [path.as_ptr() as usize, 0, 0])
}

pub fn sys_linkat(oldpath: &str, newpath: &str) -> isize {
    syscall(
        LINKAT,
        [oldpath.as_ptr() as usize, newpath.as_ptr() as usize, 0],
    )
}

pub fn sys_unlinkat(path: &str) -> isize {
    syscall(UNLINKAT, [path.as_ptr() as usize, 0, 0])
}

pub fn sys_fstat(fd: usize, st: &mut Stat) -> isize {
    syscall(FSTAT, [fd, st as *mut Stat as usize, 0])
}
