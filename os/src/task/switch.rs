use super::TaskContext;
use crate::timer;
use core::arch::global_asm;

global_asm!(include_str!("switch.S"));

extern "C" {
    fn __switch(current_task_ctx_ptr: *mut TaskContext, next_task_ctx_ptr: *const TaskContext);
}

/// 切换的开始时间
static mut SWITCH_TIME_START: usize = 0;
/// 切换的总时间
static mut SWITCH_TIME_COUNT: usize = 0;

pub unsafe fn switch(
    current_task_ctx_ptr: *mut TaskContext,
    next_task_ctx_ptr: *const TaskContext,
) {
    SWITCH_TIME_START = timer::get_time_us();
    __switch(current_task_ctx_ptr, next_task_ctx_ptr);
    // 任务切换通过`__switch`执行，也就是说，
    // 从一个任务的内核栈去往另一个任务的内核栈；
    //
    // 目标内核栈有两种情况：
    // 1. 任务尚未开始，终点则是`crate::trap::first_restore`
    // 2. 任务执行途中，终点则是另一个任务的switch函数之末
    //
    // 在情况2里，另一个任务会执行以下指令，因 SWITCH_TIME_COUNT
    // 是单例，故能成功记录上下文切换时间。
    SWITCH_TIME_COUNT += timer::get_time_us() - SWITCH_TIME_START;
}

pub fn get_switch_time() -> usize {
    unsafe { SWITCH_TIME_COUNT }
}

pub fn update_switch_time() {
    unsafe {
        SWITCH_TIME_COUNT += timer::get_time_us() - SWITCH_TIME_START;
    }
}
