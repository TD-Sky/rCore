use core::num::{NonZero, NonZeroU16, NonZeroU32, NonZeroU8};

use crate::SectorId;

/// BIOS Parameter Block BIOS参数块
/// 位于保留区的第一扇区，该扇区又名启动扇区。
#[derive(Debug, Clone)]
#[repr(packed)]
pub struct Bpb {
    /// 跳转至启动代码的指令
    _bs_jmp_boot: [u8; 3],

    /// 一般用于记录什么系统格式化此卷
    _bs_oem_name: [u8; 8],

    /// 一个扇区的字节量
    byts_per_sec: SectorBytes,

    /// 一个簇的扇区数
    sec_per_clus: ClusterSectors,

    /// 保留区的扇区数
    rsvd_sec_cnt: NonZeroU16,

    /// 此卷的文件分配表(FAT)数量，建议为2
    num_fats: NonZeroU8,

    /// - FAT32: 0
    _root_ent_cnt: u16,

    /// - FAT32: 0
    _tot_sec16: u16,

    /// 物理媒介的类型
    pub media: Media,

    /// - FAT32: 0
    _fat_sz16: u16,

    /// 中断0x13模式下，轨道的扇区数
    _sec_per_trk: u16,

    /// 中断0x13模式下，头数量
    _num_heads: u16,

    /// 中断0x13模式下使用
    _hidd_sec: u32,

    /// - FAT32: 此卷的扇区总数
    tot_sec32: NonZeroU32,

    /*
     * Extended BPB fields for FAT32 volume
     */
    /// FAT占用扇区数
    fat_sz32: NonZeroU32,

    _ext_flags: ExtFlags,

    /// 卷版本号，为0x0
    _fs_ver: u16,

    /// 根目录首个簇的编号，
    /// 应该为2，或首个可用的簇编号
    _root_clus: u32,

    /// FSINFO所在扇区号（此扇区位于保留区），通常为1
    fs_info: u16,

    /// 非0时，表示boot备份所在扇区号（此扇区位于保留区，恒为6号）
    bk_boot_sec: u16,

    _reserved: [u8; 12],

    /// 中断0x13驱动号，为0x80或0x00
    _drv_num: u8,

    _reserved1: [u8; 1],

    /// 启用时，表示接下来的三个字段存在
    _boot_sig: BootSignature,

    /// 供移动介质使用
    _voll_d: u32,

    /// 卷标签，与根目录记录的卷标签一致
    /// NOTE: 若不设卷标签，则值为"NO NAME    "
    _voll_lab: [u8; 11],

    /// 文件系统类型：FAT12/FAT16/FAT32
    ///
    /// 只用来做告示，不应信赖此字段。
    _fil_sys_type: [u8; 8],

    _reserved2: [u8; 420],

    /// [0x55, 0xAA]
    _signature_word: [u8; 2],
}

/* 扇区剩余部分皆填0x00 */

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum SectorBytes {
    B512 = 512,
    B1024 = 1024,
    B2048 = 2048,
    B4096 = 4096,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ClusterSectors {
    S0 = 0,
    S1 = 1,
    S2 = 2,
    S4 = 4,
    S8 = 8,
    S16 = 16,
    S32 = 32,
    S64 = 64,
    S128 = 128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Media {
    Fixed = 0xF8,
    Removable = 0xF0,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BootSignature {
    Set = 0x29,
    Unset = 0x00,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FatType {
    T12,
    T16,
    T32,
}

/// Bits 0-3  -- Zero-based number of active FAT. Only valid if mirroring is disabled.
/// Bits 4-6  -- Reserved.
/// Bit  7    -- 0 means the FAT is mirrored at runtime into all FATs;
///              1 means only one FAT is active; it is the one referenced in bits 0-3.
/// Bits 8-15 -- Reserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ExtFlags(u16);

impl Default for ExtFlags {
    fn default() -> Self {
        Self(0x80)
    }
}

#[derive(Debug)]
pub struct DiskSz2SecPerClus {
    base: [(usize, ClusterSectors); 6],
}

impl DiskSz2SecPerClus {
    pub fn get(&self, disk_size: usize) -> ClusterSectors {
        self.base
            .iter()
            .find(|(dsz, _)| disk_size <= *dsz)
            .unwrap_or(&self.base[5])
            .1
    }
}

#[rustfmt::skip]
static DS2SPC: DiskSz2SecPerClus = DiskSz2SecPerClus {
    base: [
        (66600,      ClusterSectors::S0),   // <= 32.5 MiB => 0 value for SecPerClusVal trips an error
        (532480,     ClusterSectors::S1),   // <= 260  MiB => 0.5k cluster
        (16777216,   ClusterSectors::S8),   // <= 8    GiB => 4k   cluster
        (33554432,   ClusterSectors::S16),  // <= 16   GB  => 8k   cluster
        (67108864,   ClusterSectors::S32),  // <= 32   GB  => 16k  cluster
        (usize::MAX, ClusterSectors::S64),  // >  32   GB  => 32k  cluster
    ],
};

impl Bpb {
    pub fn new(disk_size: usize) -> Self {
        let sec_per_clus = DS2SPC.get(disk_size);
        let num_fats = unsafe { NonZero::new_unchecked(2) };

        let byts_per_sec = SectorBytes::B512;
        let tot_sec32 = disk_size / byts_per_sec as usize;

        let mut bpb = Self {
            _bs_jmp_boot: Default::default(),
            _bs_oem_name: *b"rCore   ",
            byts_per_sec,
            sec_per_clus,
            rsvd_sec_cnt: unsafe { NonZero::new_unchecked(8) },
            num_fats,
            _root_ent_cnt: Default::default(),
            _tot_sec16: Default::default(),
            media: Media::Fixed,
            _fat_sz16: Default::default(),
            _sec_per_trk: Default::default(),
            _num_heads: Default::default(),
            _hidd_sec: Default::default(),
            tot_sec32: NonZero::new(tot_sec32 as u32).expect("Disk size should be enough"),
            fat_sz32: unsafe { NonZero::new_unchecked(1) }, // 仅仅是用来占位
            _ext_flags: Default::default(),
            _fs_ver: 0x0,
            _root_clus: 2,
            fs_info: 1,
            bk_boot_sec: 6,
            _reserved: Default::default(),
            _drv_num: Default::default(),
            _reserved1: Default::default(),
            _boot_sig: BootSignature::Unset,
            _voll_d: Default::default(),
            _voll_lab: *b"NO NAME    ",
            _fil_sys_type: *b"FAT32   ",
            _reserved2: [0; 420],
            _signature_word: [0x55, 0xAA],
        };

        bpb.set_fat_size(FatType::T32, disk_size);

        bpb
    }

    pub const fn fs_info(&self) -> SectorId {
        SectorId::new(self.fs_info as usize)
    }

    pub const fn backup_boot(&self) -> SectorId {
        SectorId::new(self.bk_boot_sec as usize)
    }

    pub const fn fat_area(&self) -> SectorId {
        SectorId::new(self.rsvd_sec_cnt.get() as usize)
    }

    pub const fn fat_count(&self) -> usize {
        self.num_fats.get() as usize
    }

    pub fn data_area(&self) -> SectorId {
        self.fat_area()
            + self.num_fats.get() as usize * self.fat_sectors()
            + self.root_dir_sectors()
    }

    pub const fn sector_bytes(&self) -> usize {
        self.byts_per_sec as usize
    }

    pub const fn cluster_sectors(&self) -> usize {
        self.sec_per_clus as usize
    }

    /// FAT占用的扇区数
    pub const fn fat_sectors(&self) -> usize {
        if self._fat_sz16 > 0 {
            self._fat_sz16 as usize
        } else {
            self.fat_sz32.get() as usize
        }
    }

    pub const fn total_sectors(&self) -> usize {
        if self._tot_sec16 > 0 {
            self._tot_sec16 as usize
        } else {
            self.tot_sec32.get() as usize
        }
    }

    pub fn total_clusters(&self) -> usize {
        (self.total_sectors() - usize::from(self.data_area())) / self.sec_per_clus as usize
    }
}

impl Bpb {
    /// 计算根目录占用的扇区数
    ///
    /// - FAT32: 0
    const fn root_dir_sectors(&self) -> usize {
        ((self._root_ent_cnt * 32 + (self.byts_per_sec as u16 - 1)) / self.byts_per_sec as u16)
            as usize
    }

    #[allow(dead_code)]
    fn fat_type(&self) -> FatType {
        let clusters = self.total_clusters();

        if clusters <= 4084 {
            FatType::T12
        } else if clusters <= 65524 {
            FatType::T16
        } else {
            FatType::T32
        }
    }

    /// 计算FAT占用扇区数并设置
    fn set_fat_size(&mut self, ty: FatType, disk_size: usize) {
        let tmp1 = disk_size - (self.rsvd_sec_cnt.get() as usize + self.root_dir_sectors());
        let mut tmp2 = 256 * self.sec_per_clus as usize + self.num_fats.get() as usize;

        if ty == FatType::T32 {
            tmp2 /= 2;
        }
        let fat_size = (tmp1 + tmp2 - 1) / tmp2;

        if ty == FatType::T32 {
            self._fat_sz16 = 0;
            self.fat_sz32 = (fat_size as u32).try_into().unwrap();
        } else {
            self._fat_sz16 = fat_size as u16;
        }
    }
}
