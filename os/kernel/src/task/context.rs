//! 任务的上下文，包含：
//! - 任务当前执行指令的位置
//! - 任务当前使用栈的栈顶
//! - 需保存的寄存器

use crate::trap::trap_return;

//   │   s11   │
//   │   s10   │
//   │   ...   │
//   │  s0/fp  │
//   │   sp    │
//   │   ra    │
//
/// 任务上下文
#[repr(C)]
#[derive(Debug, Default)]
pub struct TaskContext {
    pub ra: usize,
    pub sp: usize,
    pub s: [usize; 12],
}

impl TaskContext {
    pub fn new(kernel_stack_top: usize) -> Self {
        Self {
            ra: trap_return as usize,
            sp: kernel_stack_top,
            s: [0; 12],
        }
    }

    pub const fn empty() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }
}
