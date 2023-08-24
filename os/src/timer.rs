//! RISC-V timer-related functionality
//!
//! RISC-V架构要求CPU有一个计数器用来统计处理器自上电
//! 以来经过了多少个内置时钟的时钟周期，
//! 其保存在一个64位的CSR`mtime`中。
//! 我们无需担心它会溢出，可假设它是内核全程递增的。

use crate::config::CLOCK_FREQ;
use crate::sbi::set_timer;
use riscv::register::time;

const TICKS_PRE_SEC: usize = 100;
const MILLISECONDS: usize = 1000;
const MICROSECONDS: usize = 1_000_000;

/// read the `mtime` register
pub fn get_time() -> usize {
    time::read()
}

/// get current time in milliseconds
pub fn get_time_ms() -> usize {
    time::read() / (CLOCK_FREQ / MILLISECONDS)
}

/// get current time in microseconds
pub fn get_time_us() -> usize {
    time::read() / (CLOCK_FREQ / MICROSECONDS)
}

/// set `mtimecmp`, the next timer interrupt
pub fn set_next_trigger() {
    set_timer(get_time() + CLOCK_FREQ / TICKS_PRE_SEC);
}
