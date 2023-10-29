use alloc::vec;
use alloc::vec::Vec;

use super::address::VirtAddr;
use super::PageTable;

/// 来自用户空间的缓冲区
#[derive(Default)]
pub struct UserBuffer(Vec<&'static mut [u8]>);

impl UserBuffer {
    /// 翻译虚拟内存的指针，集合来自不同物理页的字节流以组成连续的字节流(mut)
    pub fn new(token: usize, ptr: *mut u8, len: usize) -> Self {
        let page_table = PageTable::from_token(token);
        let mut start = ptr as usize;
        let end = start + len;
        let mut bytes = vec![];

        while start < end {
            let start_va = VirtAddr::from(start);
            let vpn = start_va.page_number();
            let ppn = page_table.translate(vpn).unwrap().ppn();
            let end_va = VirtAddr::from(end).min(VirtAddr::from(vpn + 1));

            if end_va.page_offset() == 0 {
                // 跨页了，先读完当前页所有
                bytes.push(&mut ppn.page_bytes_mut()[start_va.page_offset()..]);
            } else {
                // 同一页内
                bytes.push(&mut ppn.page_bytes_mut()[start_va.page_offset()..end_va.page_offset()]);
            }

            start = end_va.into();
        }

        Self(bytes)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.iter().map(|bs| bs.len()).sum()
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &u8> {
        self.0.iter().flat_map(|sb| sb.iter())
    }

    #[inline]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut u8> {
        self.0.iter_mut().flat_map(|sb| sb.iter_mut())
    }
}

impl AsRef<[&'static mut [u8]]> for UserBuffer {
    fn as_ref(&self) -> &[&'static mut [u8]] {
        self.0.as_slice()
    }
}

impl AsMut<[&'static mut [u8]]> for UserBuffer {
    fn as_mut(&mut self) -> &mut [&'static mut [u8]] {
        self.0.as_mut_slice()
    }
}
