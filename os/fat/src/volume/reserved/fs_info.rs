use crate::{sector, SectorId};

/// # 文件系统信息
///
/// 仅FAT32格式在用，
/// 位于#1扇区，备份于#7扇区，
/// 保存着空闲簇的信息，需要持续维护。
#[derive(Debug, Clone)]
#[repr(packed)]
pub struct FsInfo {
    /// 头签名 0x41615252
    lead_sig: u32,

    _reserved1: [u8; 480],

    /// 额外签名 0x61417272
    struc_sig: u32,

    /// 剩余空闲簇数量
    /// - 0xFFFFFFFF 表示不知道
    free_count: u32,

    /// 下一个空闲簇
    /// - 0xFFFFFFFF 表示不知道
    _nxt_free: u32,

    _reserved2: [u8; 12],

    /// 尾签名 0xAA550000
    trail_sig: u32,
}

pub fn free_count() {
    sector::get(SectorId::new(1))
        .lock()
        .map(0, |fs_info: &FsInfo| fs_info.free_count);
}

pub fn record_alloc() {
    sector::get(SectorId::new(1))
        .lock()
        .map_mut(0, |fs_info: &mut FsInfo| {
            fs_info.free_count = fs_info.free_count.saturating_sub(1);
        });
}

pub fn record_free() {
    sector::get(SectorId::new(1))
        .lock()
        .map_mut(0, |fs_info: &mut FsInfo| {
            fs_info.free_count += 1;
        });
}
