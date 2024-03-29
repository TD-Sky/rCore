//! Constants used in rCore

pub use crate::board::{CLOCK_FREQ, MEMORY_END};

pub const USER_STACK_SIZE: usize = 4096;
pub const KERNEL_STACK_SIZE: usize = 4096 * 2;
pub const KERNEL_HEAP_SIZE: usize = 0x300000;

/// 物理页大小，十六进制表示方便地址转页号的计算
pub const PAGE_SIZE: usize = 0x1000;
/// 物理页内寻址的位数
pub const PAGE_SIZE_BITS: usize = 12;

/// 跳板地址
pub const TRAMPOLINE: usize = usize::MAX - PAGE_SIZE + 1;
/// Trap上下文地址的计算起点
pub const TRAP_CONTEXT_BASE: usize = TRAMPOLINE - PAGE_SIZE;

/// 显存所在的虚地址
pub const FRAMEBUFFER_VA: usize = 0x1000_0000;

pub static IMG_MOUSE: &[u8] = include_bytes!("../assets/mouse.bmp");

/*
* /// mmap距离堆底的偏移量，8G
* pub const MMAP_OFFSET_FROM: usize = 8 * 2usize.pow(30);
*/
