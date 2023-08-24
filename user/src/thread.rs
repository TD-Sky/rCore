use crate::syscall::*;

pub fn yield_() -> isize {
    sys_yield()
}

/// 睡眠指定ms长时间
pub fn sleep(period: usize) {
    let period = period as isize;
    let start = sys_get_time();

    while sys_get_time() < start + period {
        sys_yield();
    }
}
