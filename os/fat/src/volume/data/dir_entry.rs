//! 数据区，存放目录项的区域，使用**簇编号**索引。
//!
//! 因为FAT条目存放着下一个簇的编号，
//! 其中`0`表示簇未分配，`1`保留，
//! 所以数据区第一个可用的簇编号（Bpb.root_clus）一般为2

use enumflags2::{bitflags, BitFlags};

use crate::ClusterId;

#[derive(Debug, Default)]
#[repr(packed)]
pub struct DirEntry {
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

impl DirEntry {
    pub fn cluster_id(&self) -> ClusterId<u32> {
        (self.fst_clus_lo, self.fst_clus_hi).into()
    }

    pub fn checksum(&self) -> u8 {
        self.name.iter().rev().fold(0, |sum, b| {
            (if sum & 1 != 0 { 0x80 } else { 0 }) + (sum >> 1) + *b
        })
    }

    pub fn new_directory(name: &str, id: ClusterId<u32>) -> Self {
        let mut dir_entry = DirEntry::default();

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
}

#[derive(Debug)]
#[repr(packed)]
pub struct LongDirEntry {
    ord: u8,
    name1: [u8; 10],
    /// [`attr_long_name`]
    attr: BitFlags<AttrFlag>,
    /// 0
    _type: u8,
    /// 此项跟随的短名称目录项的校验和。
    /// 若不一致则说明发生了错误
    chksum: u8,
    name2: [u8; 12],
    /// 0
    _fst_clus_lo: u16,
    name3: [u8; 4],
}

impl LongDirEntry {
    const LAST_LONG_ENTRY: u8 = 0b0100_0000;
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

pub fn attr_long_name() -> BitFlags<AttrFlag> {
    AttrFlag::ReadOnly | AttrFlag::Hidden | AttrFlag::System | AttrFlag::VolumeID
}
