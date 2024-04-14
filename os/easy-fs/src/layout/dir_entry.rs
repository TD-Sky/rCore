use core::{ptr, slice};

const NAME_MAX_LEN: usize = 27;

/// 文件系统项的元信息
#[derive(Debug, Default, Clone)]
#[repr(C)]
pub struct DirEntry {
    // 最后一字节留给 \0
    name: [u8; NAME_MAX_LEN + 1],
    inode_id: u32,
}

impl DirEntry {
    /// 元信息大小恒为32字节
    pub const SIZE: usize = 32;

    #[inline]
    pub fn new(name: &str, inode_id: u32) -> Self {
        let bytes = name.as_bytes();
        let mut name = [0; NAME_MAX_LEN + 1];
        name[..bytes.len()].copy_from_slice(bytes);

        Self { name, inode_id }
    }

    pub fn name(&self) -> &str {
        let len = self.name.iter().position(|&c| c == 0).unwrap();
        core::str::from_utf8(&self.name[..len]).unwrap()
    }

    #[inline]
    pub fn inode_id(&self) -> u32 {
        self.inode_id
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(ptr::from_ref(self).cast(), Self::SIZE) }
    }

    #[inline]
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(ptr::from_mut(self).cast(), Self::SIZE) }
    }
}
