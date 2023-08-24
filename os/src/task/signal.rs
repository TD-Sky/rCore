use enumflags2::{bitflags, BitFlags};

pub const COUNT: usize = 32;

#[repr(C, align(16))]
#[derive(Debug, Clone)]
pub struct SignalAction {
    pub(super) handler: usize,
    /// 例程执行期间屏蔽的信号，
    /// 若收到则记录在TCB中，例程运行结束后再行处理
    pub(super) mask: BitFlags<SignalFlag>,
    // 目前内核不支持嵌套信号处理，所以屏蔽与否效果都一样，哈哈哈
}

#[rustfmt::skip]
#[allow(clippy::upper_case_acronyms)]
#[bitflags]
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalFlag {
    SIGDEF    = 1,
    SIGHUP    = 1 << 1,
    /// 键盘触发退出进程
    SIGINT    = 1 << 2,
    /// 键盘触发退出进程
    SIGQUIT   = 1 << 3,
    /// 非法指令
    SIGILL    = 1 << 4,
    SIGTRAP   = 1 << 5,
    SIGABRT   = 1 << 6,
    SIGBUS    = 1 << 7,
    SIGFPE    = 1 << 8,
    /// 杀死进程
    SIGKILL   = 1 << 9,
    SIGUSR1   = 1 << 10,
    /// 段错误
    SIGSEGV   = 1 << 11,
    SIGUSR2   = 1 << 12,
    SIGPIPE   = 1 << 13,
    SIGALRM   = 1 << 14,
    SIGTERM   = 1 << 15,
    SIGSTKFLT = 1 << 16,
    SIGCHLD   = 1 << 17,
    /// 若进程停止，则继续执行
    SIGCONT   = 1 << 18,
    /// 停止进程
    SIGSTOP   = 1 << 19,
    /// 终端触发停止进程
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

impl Default for SignalAction {
    #[inline]
    fn default() -> Self {
        Self {
            handler: 0,
            mask: SignalFlag::SIGQUIT | SignalFlag::SIGTRAP,
        }
    }
}

/// 检查因信号引发的进程错误，返回错误码及消息
pub(super) fn check_error(signal: BitFlags<SignalFlag>) -> Option<(i32, &'static str)> {
    signal
        .contains(SignalFlag::SIGINT)
        .then_some((-2, "Killed, SIGINT=2"))
        .or_else(|| {
            signal
                .contains(SignalFlag::SIGILL)
                .then_some((-4, "Illegal Instruction, SIGILL=4"))
        })
        .or_else(|| {
            signal
                .contains(SignalFlag::SIGABRT)
                .then_some((-6, "Aborted, SIGABRT=6"))
        })
        .or_else(|| {
            signal
                .contains(SignalFlag::SIGFPE)
                .then_some((-8, "Erroneous Arithmetic Operation, SIGFPE=8"))
        })
        .or_else(|| {
            signal
                .contains(SignalFlag::SIGKILL)
                .then_some((-9, "Killed, SIGKILL=9"))
        })
        .or_else(|| {
            signal
                .contains(SignalFlag::SIGSEGV)
                .then_some((-11, "Segmentation Fault, SIGSEGV=11"))
        })
}
