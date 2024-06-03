use core::arch::asm;
use core::ffi::{c_char, CStr};

use vfs::{CDirEntry, Stat};

use crate::signal::SignalAction;

const LINK: usize = 9;
const UNLINK: usize = 10;
const DUP: usize = 24;
const RMDIR: usize = 40;
const OPEN: usize = 56;
const CLOSE: usize = 57;
const PIPE: usize = 59;
const GETDENTS: usize = 61;
const READ: usize = 63;
const WRITE: usize = 64;
const GETCWD: usize = 79;
const FSTAT: usize = 80;
const RENAME: usize = 82;
const EXIT: usize = 93;
const SLEEP: usize = 101;
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
const EVENTFD: usize = 290;
const SPAWN: usize = 400;
const SPAWN_THREAD: usize = 1000;
const GETTID: usize = 1001;
const WAITTID: usize = 1002;
const MUTEX_CREATE: usize = 1010;
const MUTEX_LOCK: usize = 1011;
const MUTEX_UNLOCK: usize = 1012;
const SEMAPHORE_CREATE: usize = 1020;
const SEMAPHORE_UP: usize = 1021;
const SEMAPHORE_DOWN: usize = 1022;
const CONDVAR_CREATE: usize = 1030;
const CONDVAR_SIGNAL: usize = 1031;
const CONDVAR_WAIT: usize = 1032;
const FRAMEBUFFER: usize = 2000;
const FRAMEBUFFER_FLUSH: usize = 2001;
const GET_EVENT: usize = 3000;
const KEY_PRESSED: usize = 3001;

pub(crate) trait Status: Sized {
    fn status(self) -> Option<usize>;
    fn some(self) -> Option<()>;
}

impl Status for isize {
    fn status(self) -> Option<usize> {
        (self >= 0).then_some(self as usize)
    }

    fn some(self) -> Option<()> {
        (self == 0).then_some(())
    }
}

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

pub fn sys_open(path: &CStr, flags: u32) -> isize {
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
pub fn sys_getdents(fd: usize, dents: &mut [CDirEntry]) -> isize {
    syscall(GETDENTS, [fd, dents.as_mut_ptr() as usize, dents.len()])
}

pub fn sys_exit(exit_code: i32) -> ! {
    syscall(EXIT, [exit_code as usize, 0, 0]);
    unreachable!()
}

pub fn sys_sleep(duration_ms: usize) -> isize {
    syscall(SLEEP, [duration_ms, 0, 0])
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

pub fn sys_exec(path: &CStr, args: &[*const c_char]) -> isize {
    syscall(EXEC, [path.as_ptr() as usize, args.as_ptr() as usize, 0])
}

/// 参数
/// * `pid`: 指定等待的进程ID。若为-1，则等待任意一个进程退出
/// * `exit_code`: 退出码的指针
///
/// 结果
/// * PID => 结束子进程的ID
/// * -2 => 子进程存在，但尚未退出
/// * -1 => 发生错误，例如子进程不存在
pub fn sys_waitpid(pid: isize, exit_code: *mut i32) -> isize {
    syscall(WAITPID, [pid as usize, exit_code as usize, 0])
}

pub fn sys_eventfd(initval: u64, flags: u32) -> isize {
    syscall(EVENTFD, [initval as usize, flags as usize, 0])
}

pub fn sys_spawn(path: &CStr) -> isize {
    syscall(SPAWN, [path.as_ptr() as usize, 0, 0])
}

pub fn sys_link(oldpath: &CStr, newpath: &CStr) -> isize {
    syscall(
        LINK,
        [oldpath.as_ptr() as usize, newpath.as_ptr() as usize, 0],
    )
}

pub fn sys_unlink(path: &CStr) -> isize {
    syscall(UNLINK, [path.as_ptr() as usize, 0, 0])
}

pub fn sys_rmdir(path: &CStr) -> isize {
    syscall(RMDIR, [path.as_ptr() as usize, 0, 0])
}

/// 将当前进程所在目录的绝对路径写入缓冲区
///
/// # 结果
///
/// * >0 => 实际的路径长度
/// * <0 => 负·实际的路径长度
/// * =0 => unreachable
pub fn sys_getcwd(buf: &mut [u8], len: usize) -> isize {
    syscall(GETCWD, [buf.as_mut_ptr() as usize, len, 0])
}

pub fn sys_fstat(fd: usize, st: *mut Stat) -> isize {
    syscall(FSTAT, [fd, st as usize, 0])
}

pub fn sys_rename(oldpath: &CStr, newpath: &CStr) -> isize {
    syscall(
        RENAME,
        [oldpath.as_ptr() as usize, newpath.as_ptr() as usize, 0],
    )
}

/// 将进程中一个已经打开的文件复制一份并分配到一个新的文件描述符中
///
/// # 参数
///
/// * fd: 已打开文件的描述符
///
/// # 结果
///
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

pub fn sys_spawn_thread(entry: usize, arg: usize) -> isize {
    syscall(SPAWN_THREAD, [entry, arg, 0])
}

pub fn sys_gettid() -> isize {
    syscall(GETTID, [0, 0, 0])
}

/// 结果
/// * exit_code => 结束任务的退出码
/// * -2 => 任务存在，但尚未退出
/// * -1 => 发生错误，例如任务不存在
pub fn sys_waittid(tid: usize) -> isize {
    syscall(WAITTID, [tid, 0, 0])
}

pub fn sys_mutex_create(block: bool) -> isize {
    syscall(MUTEX_CREATE, [block as usize, 0, 0])
}

pub fn sys_mutex_lock(id: usize) -> isize {
    syscall(MUTEX_LOCK, [id, 0, 0])
}

pub fn sys_mutex_unlock(id: usize) -> isize {
    syscall(MUTEX_UNLOCK, [id, 0, 0])
}

pub fn sys_semaphore_create(permits: usize) -> isize {
    syscall(SEMAPHORE_CREATE, [permits, 0, 0])
}

pub fn sys_semaphore_up(id: usize) -> isize {
    syscall(SEMAPHORE_UP, [id, 0, 0])
}

pub fn sys_semaphore_down(id: usize) -> isize {
    syscall(SEMAPHORE_DOWN, [id, 0, 0])
}

pub fn sys_condvar_create() -> isize {
    syscall(CONDVAR_CREATE, [0, 0, 0])
}

pub fn sys_condvar_signal(id: usize) -> isize {
    syscall(CONDVAR_SIGNAL, [id, 0, 0])
}

pub fn sys_condvar_wait(id: usize, mutex_id: usize) -> isize {
    syscall(CONDVAR_WAIT, [id, mutex_id, 0])
}

pub fn sys_framebuffer() -> isize {
    syscall(FRAMEBUFFER, [0, 0, 0])
}

pub fn sys_framebuffer_flush() -> isize {
    syscall(FRAMEBUFFER_FLUSH, [0, 0, 0])
}

pub fn sys_get_event() -> isize {
    syscall(GET_EVENT, [0, 0, 0])
}

pub fn sys_key_pressed() -> isize {
    syscall(KEY_PRESSED, [0, 0, 0])
}
