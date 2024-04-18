use core::num::{NonZeroU16, NonZeroU32};

/// BIOS Parameter Block BIOS参数块
/// 位于保留区的第一扇区，该扇区又名启动扇区。
#[derive(Debug)]
#[repr(packed)]
pub struct Bpb {
    /// 跳转至启动代码的指令
    _bs_jmp_boot: [u8; 3],

    /// 一般用于记录什么系统格式化此卷
    bs_oem_name: [u8; 8],

    /// 一个扇区的字节量
    byts_per_sec: SectorBytes,

    /// 一个簇的扇区数
    sec_per_clus: ClusterSectors,

    /// TODO: 对齐用
    rsvd_sec_cnt: NonZeroU16,

    /// 此卷的文件分配表(FAT)数量，建议为2
    num_fats: u8,

    /// - FAT32: 0
    _root_ent_cnt: u16,

    /// - FAT32: 0
    _tot_sec16: u16,

    /// 物理媒介的类型
    media: Media,

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

    /* Extended BPB fields for FAT32 volume */
    /// FAT占用扇区数
    fat_sz32: NonZeroU32,

    /// TODO: 标志位，将其类型化
    ext_flags: u16,

    /// 卷版本号，为0x0
    fs_ver: u16,

    /// 根目录首个簇的编号，
    /// 应该为2，或首个可用的簇编号
    root_clus: u32,

    /// FSINFO所在扇区号（此扇区位于保留区），通常为1
    fs_info: u16,

    /// 非0时，表示boot备份所在扇区号（此扇区位于保留区，恒为6号）
    bk_boot_sec: u16,

    _reserved: [u8; 12],

    /// 中断0x13驱动号，为0x80或0x00
    _drv_num: u8,

    _reserved1: [u8; 1],

    /// 启用时，
    boot_sig: BootSignature,

    _voll_d: u32,

    /// 卷标签，与根目录记录的卷标签一致
    /// NOTE: 若不设卷标签，则值为"NO NAME    "
    voll_lab: [u8; 11],

    /// "FAT32   "
    fil_sys_type: [u8; 8],

    _reserved2: [u8; 420],

    /// [0x55, 0xAA]
    signature_word: [u8; 2],
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

#[derive(Debug)]
pub struct DiskSz2SecPerClus<const N: usize> {
    base: [(usize, ClusterSectors); N],
}

impl<const N: usize> DiskSz2SecPerClus<N> {
    pub fn get(&self, disk_size: usize) -> ClusterSectors {
        self.base
            .iter()
            .find(|(dsz, _)| disk_size <= *dsz)
            .unwrap_or(self.base.last().unwrap())
            .1
    }
}

#[rustfmt::skip]
static DiskSz2SecPerClusFat32: DiskSz2SecPerClus<6> = DiskSz2SecPerClus {
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
    const fn root_dir_sectors(&self) -> u16 {
        (self._root_ent_cnt * 32 + (self.byts_per_sec as u16 - 1)) / self.byts_per_sec as u16
    }

    /// 计算FAT占用扇区数并设置
    fn set_fat_size(&mut self, disk_size: usize) {
        let tmp1 = disk_size - (self.rsvd_sec_cnt.get() + self.root_dir_sectors()) as usize;
        let mut tmp2 = 256 * self.sec_per_clus as usize + self.num_fats as usize;

        if self.fil_sys_type.starts_with(b"FAT32") {
            tmp2 /= 2;
        }
        let fat_size = (tmp1 + tmp2 - 1) / tmp2;

        if self.fil_sys_type.starts_with(b"FAT32") {
            self._fat_sz16 = 0;
            self.fat_sz32 = (fat_size as u32).try_into().unwrap();
        } else {
            self._fat_sz16 = fat_size as u16;
        }
    }

    const fn fat_size(&self) -> usize {
        if self._fat_sz16 > 0 {
            self._fat_sz16 as usize
        } else {
            self.fat_sz32.get() as usize
        }
    }

    const fn total_sectors(&self) -> usize {
        if self._tot_sec16 > 0 {
            self._tot_sec16 as usize
        } else {
            self.tot_sec32.get() as usize
        }
    }

    /// Required: the FAT size is known
    const fn fat_type(&self) -> FatType {
        let data_sec = self.total_sectors()
            - (self.rsvd_sec_cnt.get() as usize
                + self.num_fats as usize * self.fat_size()
                + self.root_dir_sectors() as usize);
        let clusters = data_sec / self.sec_per_clus as u8 as usize;

        if clusters <= 4084 {
            FatType::T12
        } else if clusters <= 65524 {
            FatType::T16
        } else {
            FatType::T32
        }
    }
}
