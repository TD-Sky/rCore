use alloc::sync::Arc;
use core::iter::Iterator;
use core::mem;
use core::ops::Range;

use block_dev::BlockDevice;

use crate::util;
use crate::volume::{
    data::DataArea,
    fat::FatArea,
    reserved::{init_bpb, Bpb},
};
use crate::{sector, ClusterId, SectorId};

#[derive(Debug)]
pub struct FatFileSystem {
    /// FAT区的起始扇区
    fat_area: FatArea,
    /// 数据区的起始扇区
    data_area: DataArea,
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
            data_area: DataArea::new(&bpb),
        };
        init_bpb(bpb);
        fs
    }

    pub const fn fat(&self) -> &FatArea {
        &self.fat_area
    }

    pub const fn data(&self) -> &DataArea {
        &self.data_area
    }

    pub fn alloc_cluster(&mut self) -> (ClusterId<u32>, Range<SectorId>) {
        let id = self.fat_area.alloc().unwrap();
        let sectors = self.data_area.cluster(id).unwrap();
        util::zeroize_sectors(sectors.clone());
        (id, sectors)
    }

    pub fn data_sectors(
        &self,
        start_cluster: ClusterId<u32>,
    ) -> impl Iterator<Item = SectorId> + '_ {
        DataSectors {
            id: Some(start_cluster),
            control: self,
        }
        .flatten()
    }
}

#[derive(Debug)]
struct DataSectors<'a> {
    id: Option<ClusterId<u32>>,
    control: &'a FatFileSystem,
}

impl Iterator for DataSectors<'_> {
    type Item = Range<SectorId>;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.id.take()?;
        let sectors = self.control.data_area.cluster(id).unwrap();
        self.id = self.control.fat_area.next(id).unwrap();
        Some(sectors)
    }
}
