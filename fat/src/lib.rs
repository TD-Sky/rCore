#![no_std]

use core::num::{NonZeroU16, NonZeroU32};

use binrw::binrw;
use binrw::{BinRead, BinWrite};

/// BIOS Parameter Block BIOS参数块
/// 位于保留区的第一扇区，该扇区又名启动扇区。
#[derive(Debug)]
#[binrw]
pub struct Bpb {
    /// 跳转至启动代码的指令
    _bs_jmp_boot: [u8; 3],

    /// 一般用于记录什么系统格式化此卷
    bs_oem_name: [u8; 8],

    /// 一个扇区的字节量
    byte_per_sec: SectorBytes,

    /// 一个簇的扇区数
    sector_per_clus: ClusterSectors,

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

#[derive(Debug, PartialEq, Eq, BinRead, BinWrite)]
#[br(repr = u16)]
#[bw(repr = u16)]
#[repr(u16)]
pub enum SectorBytes {
    B512 = 512,
    B1024 = 1024,
    B2048 = 2048,
    B4096 = 4096,
}

#[derive(Debug, PartialEq, Eq, BinRead, BinWrite)]
#[br(repr = u8)]
#[bw(repr = u8)]
#[repr(u8)]
pub enum ClusterSectors {
    S1 = 1,
    S2 = 2,
    S4 = 4,
    S8 = 8,
    S16 = 16,
    S32 = 32,
    S64 = 64,
    S128 = 128,
}

#[derive(Debug, PartialEq, Eq, BinRead, BinWrite)]
#[br(repr = u8)]
#[bw(repr = u8)]
#[repr(u8)]
pub enum Media {
    Fixed = 0xF8,
    Removable = 0xF0,
}

#[derive(Debug, PartialEq, Eq, BinRead, BinWrite)]
#[br(repr = u8)]
#[bw(repr = u8)]
#[repr(u8)]
pub enum BootSignature {
    Set = 0x29,
    Unset = 0x00,
}