use super::address::{PhysAddr, PhysPageNum};
use crate::config::MEMORY_END;
use crate::sync::UPSafeCell;
use alloc::collections::VecDeque;
use lazy_static::lazy_static;

extern "C" {
    fn ekernel();
}

lazy_static! {
    static ref FRAME_ALLOCATOR: UPSafeCell<StackFrameAllocator> =
        unsafe { UPSafeCell::new(StackFrameAllocator::default()) };
}

pub fn init() {
    FRAME_ALLOCATOR.exclusive_access().init(
        PhysAddr::from(ekernel as usize).ceil(),
        PhysAddr::from(MEMORY_END).floor(),
    );
}

pub fn alloc() -> Option<Frame> {
    FRAME_ALLOCATOR.exclusive_access().alloc().map(|ppn| {
        // 清零整个页面
        ppn.page_bytes_mut().fill(0);
        Frame { ppn }
    })
}

#[inline]
pub fn dealloc(ppn: PhysPageNum) {
    FRAME_ALLOCATOR.exclusive_access().dealloc(ppn);
}

/// 物理页帧分配器
///
/// 物理页帧的管理有多种策略，其中最简单的一种是栈式分配
trait FrameAllocator {
    fn alloc(&mut self) -> Option<PhysPageNum>;
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
    fn init(&mut self, left: PhysPageNum, right: PhysPageNum) {
        self.current = left.into();
        self.end = right.into();
    }
}
