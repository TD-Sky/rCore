use alloc::collections::VecDeque;
use alloc::vec::Vec;

use super::address::{PhysAddr, PhysPageNum};
use crate::config::MEMORY_END;
use crate::sync::UpCell;

extern "C" {
    fn ekernel();
}

static FRAME_ALLOCATOR: UpCell<StackFrameAllocator> = UpCell::new(StackFrameAllocator::new());

pub fn init() {
    FRAME_ALLOCATOR.exclusive_access().init(
        PhysAddr::from(ekernel as usize).ceil(),
        PhysAddr::from(MEMORY_END).floor(),
    );
}

pub fn alloc() -> Option<Frame> {
    FRAME_ALLOCATOR.exclusive_access().alloc().map(Frame::new)
}

pub fn alloc_continuous(len: usize) -> Option<Vec<Frame>> {
    FRAME_ALLOCATOR
        .exclusive_access()
        .alloc_continuous(len)
        .map(|pages| pages.into_iter().map(Frame::new).collect())
}

pub fn dealloc(ppn: PhysPageNum) {
    FRAME_ALLOCATOR.exclusive_access().dealloc(ppn);
}

/// 物理页帧分配器
///
/// 物理页帧的管理有多种策略，其中最简单的一种是栈式分配
trait FrameAllocator {
    fn alloc(&mut self) -> Option<PhysPageNum>;
    fn alloc_continuous(&mut self, len: usize) -> Option<Vec<PhysPageNum>>;
    fn dealloc(&mut self, ppn: PhysPageNum);
}

/// 栈式物理页帧分配器
///
/// `current`为栈顶的物理页地址
/// (虽然分配是返回物理页号，但是拼上12个0就是地址了)，
/// 页号区间 [current, end) 的物理内存**从未**被分配
#[derive(Default)]
pub struct StackFrameAllocator {
    current: usize,
    end: usize,
    /// 被回收的物理页号之栈，栈顶位于尾部
    recycled: VecDeque<usize>,
}

/// 实际上是一个独占指针
pub struct Frame {
    pub ppn: PhysPageNum,
}

impl Frame {
    pub fn new(ppn: PhysPageNum) -> Self {
        // 清零整个页面
        ppn.page_bytes_mut().fill(0);
        Self { ppn }
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        FRAME_ALLOCATOR.exclusive_access().dealloc(self.ppn);
    }
}

impl FrameAllocator for StackFrameAllocator {
    /// 分配新页面
    fn alloc(&mut self) -> Option<PhysPageNum> {
        match self.recycled.pop_back() {
            // 尝试分配以前的回收的物理页号
            Some(ppn) => Some(PhysPageNum::from_raw(ppn)),
            None => (self.current < self.end).then(|| {
                // 若内存尚未用尽，则分配其左端点`current`，并缩短页号区间
                let current = self.current;
                self.current += 1;
                PhysPageNum::from_raw(current)
            }),
        }
    }

    /// 分配一段连续的页面
    fn alloc_continuous(&mut self, len: usize) -> Option<Vec<PhysPageNum>> {
        let new_current = self.current + len;
        (new_current < self.end).then(|| {
            self.current = new_current;
            (1..=len)
                .map(|i| PhysPageNum::from(new_current - i))
                .collect()
        })
    }

    /// 回收页面
    ///
    /// 合法的被回收页面
    /// - 之前一定被分配出去过，因此其物理页号小于`current`
    /// - 它不是回收状态，即`recycled`中不包含此物理页号
    fn dealloc(&mut self, ppn: PhysPageNum) {
        let ppn: usize = ppn.into();
        if ppn >= self.current || self.recycled.iter().any(|&v| v == ppn) {
            panic!("Frame ppn={:#x} has not been allocated!", ppn);
        }
        self.recycled.push_back(ppn);
    }
}

impl StackFrameAllocator {
    const fn new() -> Self {
        Self {
            current: 0,
            end: 0,
            recycled: VecDeque::new(),
        }
    }

    fn init(&mut self, left: PhysPageNum, right: PhysPageNum) {
        self.current = left.into();
        self.end = right.into();
    }
}
