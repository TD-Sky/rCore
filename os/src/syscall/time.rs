use crate::timer;

pub fn sys_get_time() -> isize {
    timer::get_time_ms() as isize
}
