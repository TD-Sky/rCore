use alloc::ffi::CString;
use alloc::format;
use alloc::vec::Vec;
use core::ptr;

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
    let path = if !path.starts_with('/') {
        &format!("/usr/bin/{path}")
    } else {
        path
    };

    let path = CString::new(path).unwrap();
    let args = args
        .into_iter()
        .map(|s| CString::new(s.as_ref()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let mut args: Vec<_> = args.iter().map(|s| s.as_c_str().as_ptr()).collect();
    args.push(ptr::null());
    match sys_exec(&path, &args) {
        -1 => None,
        _ => unreachable!(),
    }
}

pub fn spawn(path: &str) -> Option<usize> {
    let path = CString::new(path).ok()?;
    sys_spawn(&path).status()
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
