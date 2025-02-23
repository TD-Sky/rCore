use enumflags2::{BitFlags, bitflags};

use crate::syscall::*;

/// Default signal handling
pub const SIGDEF: u32 = 0;
pub const SIGHUP: u32 = 1;
pub const SIGINT: u32 = 2;
pub const SIGQUIT: u32 = 3;
pub const SIGILL: u32 = 4;
pub const SIGTRAP: u32 = 5;
pub const SIGABRT: u32 = 6;
pub const SIGBUS: u32 = 7;
pub const SIGFPE: u32 = 8;
pub const SIGKILL: u32 = 9;
pub const SIGUSR1: u32 = 10;

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, Default)]
pub struct SignalAction {
    pub handler: usize,
    pub mask: BitFlags<SignalFlag>,
}

#[rustfmt::skip]
#[allow(clippy::upper_case_acronyms)]
#[bitflags]
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalFlag {
    SIGDEF    = 1,
    SIGHUP    = 1 << 1,
    SIGINT    = 1 << 2,
    SIGQUIT   = 1 << 3,
    SIGILL    = 1 << 4,
    SIGTRAP   = 1 << 5,
    SIGABRT   = 1 << 6,
    SIGBUS    = 1 << 7,
    SIGFPE    = 1 << 8,
    SIGKILL   = 1 << 9,
    SIGUSR1   = 1 << 10,
    SIGSEGV   = 1 << 11,
    SIGUSR2   = 1 << 12,
    SIGPIPE   = 1 << 13,
    SIGALRM   = 1 << 14,
    SIGTERM   = 1 << 15,
    SIGSTKFLT = 1 << 16,
    SIGCHLD   = 1 << 17,
    SIGCONT   = 1 << 18,
    SIGSTOP   = 1 << 19,
    SIGTSTP   = 1 << 20,
    SIGTTIN   = 1 << 21,
    SIGTTOU   = 1 << 22,
    SIGURG    = 1 << 23,
    SIGXCPU   = 1 << 24,
    SIGXFSZ   = 1 << 25,
    SIGVTALRM = 1 << 26,
    SIGPROF   = 1 << 27,
    SIGWINCH  = 1 << 28,
    SIGIO     = 1 << 29,
    SIGPWR    = 1 << 30,
    SIGSYS    = 1 << 31,
}

pub fn kill(pid: usize, signum: u32) -> Option<()> {
    (sys_kill(pid, signum) == 0).then_some(())
}

pub fn sigaction(signum: u32, action: &SignalAction, old_action: &mut SignalAction) -> Option<()> {
    (sys_sigaction(signum, action, old_action) == 0).then_some(())
}

pub fn sigprocmask(mask: u32) -> Option<u32> {
    match sys_sigprocmask(mask) {
        -1 => None,
        old_mask => Some(old_mask as u32),
    }
}

pub fn sigreturn() -> ! {
    sys_sigreturn()
}
