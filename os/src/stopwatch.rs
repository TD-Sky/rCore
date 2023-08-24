//! 秒表

use lazy_static::lazy_static;

use crate::sync::UPSafeCell;
use crate::timer;

lazy_static! {
    static ref STOPWATCH: UPSafeCell<Stopwatch> = unsafe { UPSafeCell::new(Stopwatch::default()) };
}

pub fn refresh() -> usize {
    STOPWATCH.exclusive_access().refresh()
}

#[derive(Default)]
struct Stopwatch {
    current: usize,
}

impl Stopwatch {
    /// 掐一次表，并返回与上次掐表时间之间隔
    fn refresh(&mut self) -> usize {
        let pre_stop = self.current;
        self.current = timer::get_time_ms();
        self.current - pre_stop
    }
}
