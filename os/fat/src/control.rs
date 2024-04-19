use core::mem;

use alloc::sync::Arc;
use block_dev::BlockDevice;

use crate::volume::reserved::Bpb;

#[derive(Debug)]
pub struct FatFileSystem {
    /// FAT区的起始扇区
    fat_area: usize,
    /// 数据区的起始扇区
    data_area: usize,
}

impl FatFileSystem {
    pub fn new(dev: &Arc<dyn BlockDevice>) -> Self {
        let bpb: Bpb = {
            let mut buf = [0u8; mem::size_of::<Bpb>()];
            dev.read_block(0, &mut buf);
            unsafe { mem::transmute(buf) }
        };

        FatFileSystem {
            fat_area: bpb.fat_area_sector(),
            data_area: bpb.data_area_sector(),
        }
    }
}
