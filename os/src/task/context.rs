//! 任务的上下文，包含：
//! - 任务当前执行指令的位置
//! - 任务当前使用栈的栈顶
//! - 需保存的寄存器

use crate::trap::first_restore;

// |   s11   |
// |   s10   |
// |   ...   |
// |   s0    |
// |   sp    |
// |   ra    |
#[repr(C)]
#[derive(Default)]
pub struct TaskContext {
    pub ra: usize,
    pub sp: usize,
    pub s: [usize; 12],
}

impl TaskContext {
    pub fn new(kernel_stack_top: usize) -> Self {
        Self {
            ra: first_restore as usize,
            sp: kernel_stack_top,
            s: [0; 12],
        }
    }
}
