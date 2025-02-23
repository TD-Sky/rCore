//! 数据区，存放目录项的区域，使用**簇编号**索引。
//!
//! 因为FAT条目存放着下一个簇的编号，
//! 其中`0`表示簇未分配，`1`保留，
//! 所以数据区第一个可用的簇编号（Bpb.root_clus）一般为2。

use alloc::string::String;
use alloc::vec::Vec;
use core::mem;

use enumflags2::{BitFlags, bitflags};

use crate::{ClusterId, sector};

static CWD_NAME: [u8; 11] = {
    let mut arr = [0; 11];
    arr[0] = b'.';
    arr
};

static PARENT_NAME: [u8; 11] = {
    let mut arr = [0; 11];
    arr[0] = b'.';
    arr[1] = b'.';
    arr
};

pub type FreeDirEntry = [u8; 32];

pub static FREE: FreeDirEntry = {
    let mut arr = [0; 32];
    arr[0] = 0xE5;
    arr
};

pub static TAIL_FREE: FreeDirEntry = [0; 32];

/// 这是一个极度危险的类型，只应该在搜索目录项时使用。
///
/// 出于方便考虑，两个目录项都实现`Copy`，当C语言写吧。
pub union DirEntry {
    pub short: ShortDirEntry,
    pub long: LongDirEntry,
}

impl DirEntry {
    /// # Safety
    ///
    /// 通过属性才能知晓目录项属于短还是长。
    pub unsafe fn attr(&self) -> BitFlags<AttrFlag> {
        unsafe { self.short.attr }
    }
}

#[derive(Debug, Default, Clone, Copy)]
#[repr(packed)]
pub struct ShortDirEntry {
    name: [u8; 11],

    pub attr: BitFlags<AttrFlag>,

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
    pub fn as_cwd(&self) -> Self {
        let mut cwd = *self;
        cwd.name = CWD_NAME;
        cwd
    }

    /// 创建一个簇编号为`pid`的父目录项(..)
    pub fn new_parent(mut pid: ClusterId<u32>) -> Self {
        let mut dirent = Self::default();

        // 若此父目录为根，则使用[`ClusterId::FREE`]
        if pid == ClusterId::MIN {
            pid = ClusterId::FREE;
        }
        dirent.set_cluster_id(pid);

        dirent.attr |= AttrFlag::Directory;
        dirent.name = PARENT_NAME;
        dirent
    }

    pub fn cluster_id(&self) -> ClusterId<u32> {
        let id: ClusterId<u32> = (self.fst_clus_lo, self.fst_clus_hi).into();

        if self.attr.contains(AttrFlag::Directory) {
            // NOTE: 相对目录项指向根目录时，其簇编号为0。
            //       但我们需要有效的索引，所以直接提到[`ClusterId::MIN`]，
            //       即真正的根目录的地址（绝大部分情况）。
            id.max(ClusterId::MIN)
        } else {
            /* 新创的空文件ID为[`ClusterId::FREE`]，直接返回无妨 */
            id
        }
    }

    pub fn set_cluster_id(&mut self, id: ClusterId<u32>) {
        (self.fst_clus_lo, self.fst_clus_hi) = id.split();
    }

    pub fn checksum(&self) -> u8 {
        Self::checksum_from(&self.name)
    }

    pub fn checksum_from<'a>(bytes: impl IntoIterator<Item = &'a u8>) -> u8 {
        let mut arr = [0; 11];
        for (a, b) in arr.iter_mut().zip(bytes) {
            *a = b.to_ascii_uppercase();
        }
        log::trace!("Input bytes: {arr:?}");
        let checksum = arr.iter().fold(0, |sum, &b| {
            // NOTE: The operation is an unsigned char rotate right
            (if sum & 1 != 0 { 0x80 } else { 0u8 })
                .wrapping_add(sum >> 1)
                .wrapping_add(b)
        });
        log::trace!("checksum={checksum:?}");
        checksum
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
            0x00 => DirEntryStatus::TailFree,
            _ => DirEntryStatus::Occupied,
        }
    }

    pub const fn size(&self) -> usize {
        self.file_size as usize
    }

    pub fn resize(&mut self, size: usize) {
        self.file_size = size as u32;
    }

    pub fn is_relative(&self) -> bool {
        self.name == CWD_NAME || self.name == PARENT_NAME
    }
}

impl ShortDirEntry {
    fn rename(&mut self, name: &str) {
        let mut arr = [0; 11];
        for (b, nb) in arr.iter_mut().zip(name.as_bytes()) {
            *b = nb.to_ascii_uppercase();
        }
        self.name = arr;
    }
}

/// 可容纳名字的26个字节。
///
/// 目录项名称最长为255字节，所以最多用到10个长目录项。
#[derive(Debug, Clone, Copy)]
#[repr(packed)]
pub struct LongDirEntry {
    /// 序号（1起）
    pub ord: u8,
    name1: [u8; 10],
    /// [`attr_long_name`]
    _attr: BitFlags<AttrFlag>,
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

impl Default for LongDirEntry {
    fn default() -> Self {
        Self {
            ord: 0,
            name1: Default::default(),
            _attr: Self::attr(),
            _type: 0,
            chksum: 0,
            name2: Default::default(),
            _fst_clus_lo: 0,
            name3: Default::default(),
        }
    }
}

impl LongDirEntry {
    pub const LAST_MASK: u8 = 0b0100_0000;

    /// 可为名称容纳的字节数
    pub const CAP: usize = 26;

    #[inline]
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
    /// name[0] == 0，此条目后的条目皆为[`DirEntryStatus::TailFree`]
    TailFree,
    /// 已被使用
    Occupied,
}

/// Converts [`LongDirEntry`] to directory entry name.
///
/// # 参数
///
/// - `dirents`: **正序排列**的长目录项。
pub fn dirents2name(dirents: &[LongDirEntry]) -> String {
    let bytes: Vec<u8> = dirents
        .iter()
        .flat_map(|dirent| [dirent.name1.as_slice(), &dirent.name2, &dirent.name3].into_iter())
        .flatten()
        .take_while(|b| **b != b'\0')
        .cloned()
        .collect();

    String::from_utf8(bytes).expect("Valid UTF-8 dir_entry name")
}

/// Converts directory entry name to [`ShortDirEntry`] + [`Vec<LongDirEntry>`].
///
/// # 返回
///
/// - `ShortDirEntry`: 除了`name`，其它均为默认值。
/// - `Vec<LongDirEntry>`: **反序排列**的长目录项，已全数赋值。
pub fn name2dirents(name: &str) -> (ShortDirEntry, Vec<LongDirEntry>) {
    let mut short = ShortDirEntry::default();
    short.rename(name);

    let chksum = short.checksum();

    let mut longs: Vec<_> = name
        .as_bytes()
        .chunks(LongDirEntry::CAP)
        .enumerate()
        .map(|(i, bytes)| {
            let mut long = LongDirEntry {
                ord: (i + 1) as u8,
                chksum,
                ..Default::default()
            };
            for (b, &nb) in [long.name1.as_mut_slice(), &mut long.name2, &mut long.name3]
                .into_iter()
                .flatten()
                .zip(bytes)
            {
                *b = nb;
            }
            long
        })
        .rev()
        .collect();

    longs[0].ord |= LongDirEntry::LAST_MASK;

    (short, longs)
}

/// 修改短目录的名称，并构造新的长目录项。
///
/// # 返回
///
/// - `ShortDirEntry`: 重命名过的短目录项。
/// - `Vec<LongDirEntry>`: **反序排列**的长目录项，已全数赋值。
pub fn rename_dirents(short: &ShortDirEntry, new_name: &str) -> (ShortDirEntry, Vec<LongDirEntry>) {
    let mut short = *short;
    short.rename(new_name);
    let chksum = short.checksum();

    let mut longs: Vec<_> = new_name
        .as_bytes()
        .chunks(LongDirEntry::CAP)
        .enumerate()
        .map(|(i, bytes)| {
            let mut long = LongDirEntry {
                ord: (i + 1) as u8,
                chksum,
                ..Default::default()
            };
            for (b, &nb) in [long.name1.as_mut_slice(), &mut long.name2, &mut long.name3]
                .into_iter()
                .flatten()
                .zip(bytes)
            {
                *b = nb;
            }
            long
        })
        .rev()
        .collect();

    longs[0].ord |= LongDirEntry::LAST_MASK;

    (short, longs)
}

pub fn sector_dirents() -> usize {
    sector::size() / mem::size_of::<ShortDirEntry>()
}
