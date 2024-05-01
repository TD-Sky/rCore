use alloc::sync::Arc;
use core::mem;

use block_dev::BlockDevice;

use crate::sector::{self, SectorId};
use crate::volume::fat::FatArea;
use crate::volume::reserved::{init_bpb, Bpb};

#[derive(Debug)]
pub struct FatFileSystem {
    /// FAT区的起始扇区
    fat_area: FatArea,
    /// 数据区的起始扇区
    data_area: SectorId,
}

impl FatFileSystem {
    pub fn new(dev: &Arc<dyn BlockDevice>) -> Self {
        let bpb: Bpb = {
            let mut buf = [0u8; mem::size_of::<Bpb>()];
            dev.read_block(0, &mut buf);
            unsafe { mem::transmute(buf) }
        };

        sector::init_cache(dev);

        let fs = FatFileSystem {
            fat_area: FatArea::new(&bpb),
            data_area: bpb.data_area_sector(),
        };
        init_bpb(bpb);
        fs
    }
}
