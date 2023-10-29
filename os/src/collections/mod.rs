use core::ops::Deref;

use alloc::vec::Vec;

#[derive(Debug, Clone)]
pub struct SlotVec<T>(Vec<Option<T>>);

impl<T> Default for SlotVec<T> {
    fn default() -> Self {
        Self(Vec::default())
    }
}

impl<T> SlotVec<T> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn push(&mut self, element: T) {
        self.0.push(Some(element));
    }

    /// 插入新元素至空槽位，并返回槽位的索引
    pub fn insert(&mut self, element: T) -> usize {
        let index = self.0.iter().position(Option::is_none).unwrap_or_else(|| {
            self.0.push(None);
            self.0.len() - 1
        });
        self.0[index] = Some(element);
        index
    }

    /// 插入元素至指定槽位，若槽位数量不足，则扩容再插入
    pub fn insert_kv(&mut self, index: usize, element: T) {
        if self.0.len() < index + 1 {
            self.0.resize_with(index + 1, || Option::None);
        }

        self.0[index] = Some(element);
    }

    pub fn remove(&mut self, index: usize) -> Option<T> {
        self.0[index].take()
    }

    pub fn clear(&mut self) {
        self.0.clear()
    }
}

impl<T> SlotVec<T>
where
    T: Clone,
{
    /// 直接复制指定槽位的值
    ///
    /// # Panics
    ///
    /// 当指定槽位为空时panic。
    pub fn get(&self, index: usize) -> T {
        self.0[index].clone().unwrap()
    }
}

impl<T> Deref for SlotVec<T> {
    type Target = [Option<T>];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> FromIterator<T> for SlotVec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self(iter.into_iter().map(Option::Some).collect())
    }
}
