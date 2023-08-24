use core::arch::asm;

use easy_fs::{DirEntry, Stat};

use crate::signal::SignalAction;

const DUP: usize = 24;
const UNLINKAT: usize = 35;
const LINKAT: usize = 37;
const OPEN: usize = 56;
const CLOSE: usize = 57;
const PIPE: usize = 59;
const GETDENTS: usize = 61;
const READ: usize = 63;
const WRITE: usize = 64;
const FSTAT: usize = 80;
const EXIT: usize = 93;
const YIELD: usize = 124;
const SIGACTION: usize = 134;
const SIGPROCMASK: usize = 135;
const SIGRETURN: usize = 139;
const TIME: usize = 169;
const GETPID: usize = 172;
const SBRK: usize = 214;
const KILL: usize = 219;
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

/// 将指定目录下的项填充进缓冲区`dents`
///
/// 结果
/// -1 => 读取的一定不是目录
/// count => 读取到的文件项数目
///
/// UB
/// 若读取的不是目录，则可能发生未定义行为
pub fn sys_getdents(fd: usize, dents: &mut [DirEntry]) -> isize {
    syscall(GETDENTS, [fd, dents.as_mut_ptr() as usize, dents.len()])
}

pub fn sys_exit(exit_code: i32) -> ! {
    syscall(EXIT, [exit_code as usize, 0, 0]);
    unreachable!()
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

/// 结果
/// * 0 => 当前在子进程
/// * PID => 创建的子进程ID，且当前在父进程
pub fn sys_fork() -> isize {
    syscall(FORK, [0, 0, 0])
}

pub fn sys_exec(path: &str, args: &[*const u8]) -> isize {
    syscall(EXEC, [path.as_ptr() as usize, args.as_ptr() as usize, 0])
}

/// 结果
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

/// 将进程中一个已经打开的文件复制一份并分配到一个新的文件描述符中
///
/// 参数
/// * fd: 已打开文件的描述符
///
/// 结果
/// * -1 => 出现错误，可能是`fd`无效
/// * new_fd => 文件副本的描述符
pub fn sys_dup(fd: usize) -> isize {
    syscall(DUP, [fd, 0, 0])
}

/// 为当前进程打开一个管道。
///
/// 参数
/// * pipe: 表示应用地址空间中的一个长度为2的数组，
/// 内核需要按顺序将管道读端和写端的文件描述符写入到数组中。
///
/// 结果
/// * -1 => 出现错误，可能是传入的地址不合法
/// * 0 => 正常
pub fn sys_pipe(pipe: &mut [usize]) -> isize {
    syscall(PIPE, [pipe.as_mut_ptr() as usize, 0, 0])
}

/// 从当前进程向一个进程发送一道信号。
///
/// 参数
/// * pid: 目标进程
/// * signal: 信号编号
///
/// 结果
/// * -1 => 传入参数不正确，比如指定进程或信号类型不存在
/// * 0 => 正常
pub fn sys_kill(pid: usize, signal: u32) -> isize {
    syscall(KILL, [pid, signal as usize, 0])
}

/// 结果
/// -1 => `action`,`old_action`为空指针；信号类型不存在返回
/// 0 => 正常
pub fn sys_sigaction(
    signum: u32,
    action: *const SignalAction,
    old_action: *mut SignalAction,
) -> isize {
    syscall(
        SIGACTION,
        [signum as usize, action as usize, old_action as usize],
    )
}

/// 设置当前进程的全局信号掩码。
///
/// 参数
/// * mask: 表示当前进程要设置成的全局信号掩码，
/// 代表一个信号集合，在集合中的信号始终被该进程屏蔽。
///
/// 返回值
/// -1 => 传入参数错误
/// old_mask => 之前的信号掩码
pub fn sys_sigprocmask(mask: u32) -> isize {
    syscall(SIGPROCMASK, [mask as usize, 0, 0])
}

/// 通知内核信号处理例程退出，可以恢复原先进程的执行了。
pub fn sys_sigreturn() -> ! {
    syscall(SIGRETURN, [0, 0, 0]);
    unreachable!("signal routine must return successfully")
}
