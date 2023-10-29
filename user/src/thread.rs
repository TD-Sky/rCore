use crate::syscall::*;

pub fn yield_() -> isize {
    sys_yield()
}

pub fn exit(exit_code: i32) -> ! {
    sys_exit(exit_code)
}

/// 睡眠指定ms长时间
pub fn sleep(duration_ms: usize) {
    sys_sleep(duration_ms);
}

pub fn spawn(entry: usize, arg: usize) -> usize {
    sys_spawn_thread(entry, arg) as usize
}

pub fn gettid() -> usize {
    sys_gettid() as usize
}

pub fn waittid(tid: usize) -> Option<i32> {
    loop {
        match sys_waittid(tid) {
            -2 => {
                yield_();
            }
            -1 => break None,
            exit_code => break Some(exit_code as i32),
        }
    }
}
