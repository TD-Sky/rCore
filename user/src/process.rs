use alloc::ffi::CString;
use alloc::vec::Vec;

use crate::status2option;
use crate::syscall::*;
use crate::thread::yield_;

pub fn getpid() -> usize {
    sys_getpid() as usize
}

pub fn fork() -> usize {
    sys_fork() as usize
}

/// 结果：
/// None => 程序不存在
pub fn exec<S, I>(path: &str, args: I) -> Option<!>
where
    S: AsRef<str>,
    I: IntoIterator<Item = S>,
{
    let path = CString::new(path).unwrap();
    let args = args
        .into_iter()
        .map(|s| CString::new(s.as_ref()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let args: Vec<_> = args
        .iter()
        .map(|s| s.as_c_str().as_ptr())
        .chain([c"".as_ptr()])
        .collect();
    match sys_exec(&path, &args) {
        -1 => None,
        _ => unreachable!(),
    }
}

pub fn spawn(path: &str) -> Option<usize> {
    let path = CString::new(path).unwrap();
    status2option(sys_spawn(&path))
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
