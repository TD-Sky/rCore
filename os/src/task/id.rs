use alloc::vec::Vec;

/// 通用资源分配器
#[derive(Default)]
pub struct RecycleAllocator {
    current: usize,
    recycled: Vec<usize>,
}

impl RecycleAllocator {
    pub fn alloc(&mut self) -> usize {
        if let Some(id) = self.recycled.pop() {
            id
        } else {
            let current = self.current;
            self.current += 1;
            current
        }
    }

    pub fn dealloc(&mut self, id: usize) {
        assert!(id < self.current);
        assert!(
            !self.recycled.iter().any(|&i| i == id),
            "id={id} has been deallocated!",
        );
        self.recycled.push(id);
    }
}
