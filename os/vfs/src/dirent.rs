use alloc::string::String;
use core::ffi::c_char;

#[derive(Debug)]
pub struct DirEntry {
    /// Inode number
    pub inode: u64,
    pub ty: DirEntryType,
    pub name: String,
}

/// 系统调用所交换的目录项
#[derive(Debug)]
#[repr(C)]
pub struct CDirEntry {
    /// Inode number
    pub inode: u64,
    pub ty: DirEntryType,
    pub name: *mut c_char,
}

impl CDirEntry {
    pub const NAME_CAP: usize = 255;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum DirEntryType {
    Block,
    Char,
    Directory,
    Fifo,
    SymLink,
    #[default]
    Regular,
}
