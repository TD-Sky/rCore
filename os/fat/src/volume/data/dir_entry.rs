//! 数据区，存放目录项的区域，使用**簇编号**索引。
//!
//! 因为FAT条目存放着下一个簇的编号，
//! 其中`0`表示簇未分配，`1`保留，
//! 所以数据区第一个可用的簇编号（Bpb.root_clus）一般为2。

use core::mem::ManuallyDrop;

use alloc::{string::String, vec::Vec};
use enumflags2::{bitflags, BitFlags};

use crate::ClusterId;

/// 这是一个极度危险的类型，只应该在搜索目录项时使用
pub union DirEntry {
    pub short: ManuallyDrop<ShortDirEntry>,
    pub long: ManuallyDrop<LongDirEntry>,
}

impl DirEntry {
    /// # Safety
    ///
    /// 通过属性才能知晓目录项属于短还是长。
    pub unsafe fn attr(&self) -> BitFlags<AttrFlag> {
        self.short.attr()
    }
}

#[derive(Debug, Default, Clone)]
#[repr(packed)]
pub struct ShortDirEntry {
    name: [u8; 11],

    attr: BitFlags<AttrFlag>,

    /// Reserved, must be 0
    _ntres: u8,

    /// Count of tenths of a second.
    /// Range: [0, 199]
    _crt_time_tenth: u8,

    /// Creation time, granularity is 2 seconds
    _crt_time: u16,

    /// Creation date
    _crt_date: u16,

    /// Last access date
    _lst_acc_date: u16,

    /// High word of first data cluster number
    /// for file/directory described by this entry
    fst_clus_hi: u16,

    /// Last modification time
    _wrt_time: u16,

    /// Last modification date
    _wrt_date: u16,

    /// Low word of first data cluster number
    /// for file/directory described by this entry
    fst_clus_lo: u16,

    /// Quantity containing size in bytes
    /// of file/directory described by this entry
    file_size: u32,
}

impl ShortDirEntry {
    pub fn cluster_id(&self) -> ClusterId<u32> {
        (self.fst_clus_lo, self.fst_clus_hi).into()
    }

    pub fn checksum(&self) -> u8 {
        Self::checksum_from(self.name.as_slice())
    }

    pub fn checksum_from<'a>(bytes: impl IntoIterator<Item = &'a u8>) -> u8 {
        let mut arr = [0; 8];
        for (a, &b) in arr.iter_mut().zip(bytes) {
            *a = b;
        }
        arr.iter().rev().fold(0, |sum, b| {
            (if sum & 1 != 0 { 0x80 } else { 0 }) + (sum >> 1) + *b
        })
    }

    pub fn new_directory(name: &str, id: ClusterId<u32>) -> Self {
        let mut dir_entry = ShortDirEntry::default();

        dir_entry
            .name
            .iter_mut()
            .zip(name.as_bytes())
            .for_each(|(b1, b2)| *b1 = *b2);

        let (low, high) = id.split();
        dir_entry.fst_clus_lo = low;
        dir_entry.fst_clus_hi = high;

        dir_entry.attr |= AttrFlag::Archive;

        dir_entry
    }

    pub fn status(&self) -> DirEntryStatus {
        match self.name[0] {
            0xE5 => DirEntryStatus::Free,
            0x00 => DirEntryStatus::FreeHead,
            _ => DirEntryStatus::Occupied,
        }
    }

    pub const fn attr(&self) -> BitFlags<AttrFlag> {
        self.attr
    }

    pub const fn file_size(&self) -> usize {
        self.file_size as usize
    }

    pub fn set_file_size(&mut self, size: usize) {
        self.file_size = size as u32;
    }
}

/// 可容纳名字的26个字节。
///
/// 目录项名称最长为255字节，所以最多用到10个长目录项。
#[derive(Debug, Default, Clone)]
#[repr(packed)]
pub struct LongDirEntry {
    /// 序号（1起）
    pub ord: u8,
    name1: [u8; 10],
    /// [`attr_long_name`]
    attr: BitFlags<AttrFlag>,
    /// 0
    _type: u8,
    /// 此项跟随的短名称目录项的校验和。
    /// 若不一致则说明发生了错误
    pub chksum: u8,
    name2: [u8; 12],
    /// 0
    _fst_clus_lo: u16,
    name3: [u8; 4],
}

impl LongDirEntry {
    pub const LAST_MASK: u8 = 0b0100_0000;
}

impl LongDirEntry {
    pub fn attr() -> BitFlags<AttrFlag> {
        AttrFlag::ReadOnly | AttrFlag::Hidden | AttrFlag::System | AttrFlag::VolumeID
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[bitflags]
#[repr(u8)]
pub enum AttrFlag {
    ReadOnly = 0b0000_0001,
    Hidden = 0b0000_0010,
    /// The corresponding file is tagged as a component of the operating system
    System = 0b0000_0100,
    /// The corresponding entry contains the volume label
    VolumeID = 0b0000_1000,
    Directory = 0b0001_0000,
    /// Indicates that properties of the associated file have been modified
    Archive = 0b0010_0000,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DirEntryStatus {
    /// name[0] == 0xE5
    Free,
    /// name[0] == 0，是接下来连续空条目之首
    FreeHead,
    /// 已被使用
    Occupied,
}

pub fn dir_entry_name(dirents: &[LongDirEntry]) -> String {
    let bytes: Vec<u8> = dirents
        .iter()
        .flat_map(|dirent| {
            [
                dirent.name1.as_slice(),
                dirent.name2.as_slice(),
                dirent.name3.as_slice(),
            ]
            .into_iter()
        })
        .flatten()
        .take_while(|b| **b != b'\0')
        .cloned()
        .collect();

    String::from_utf8(bytes).expect("Valid UTF-8 dir_entry name")
}
