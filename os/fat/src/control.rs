use alloc::sync::Arc;
use core::iter::Iterator;
use core::mem;
use core::ops::Range;

use block_dev::BlockDevice;
use vfs::StatFs;

use crate::volume::{
    data::DataArea,
    fat::FatArea,
    reserved::{bpb, init_bpb, Bpb, FsInfo},
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
    pub fn load(dev: &Arc<dyn BlockDevice>) -> Self {
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

    pub fn new(disk_size: usize) -> Self {
        let bpb = Bpb::new(disk_size);
        let fs = FatFileSystem {
            fat_area: FatArea::new(&bpb),
            data_area: DataArea::new(&bpb),
        };
        init_bpb(bpb);
        fs
    }

    pub fn foramt(&mut self, dev: &Arc<dyn BlockDevice>) {
        sector::init_cache(dev);

        let bpb = bpb();
        sector::get(SectorId::new(0))
            .lock()
            .map_mut(0, |disk_bpb: &mut Bpb| disk_bpb.clone_from(bpb));
        sector::get(SectorId::new(6))
            .lock()
            .map_mut(0, |disk_bpb: &mut Bpb| disk_bpb.clone_from(bpb));

        let fs_info = FsInfo::new(bpb);
        sector::get(SectorId::new(1))
            .lock()
            .map_mut(0, |disk_fs_info: &mut FsInfo| {
                disk_fs_info.clone_from(&fs_info)
            });
        sector::get(SectorId::new(7))
            .lock()
            .map_mut(0, |disk_fs_info: &mut FsInfo| *disk_fs_info = fs_info);

        for sid in self.fat_area.range() {
            sector::get(sid)
                .lock()
                .map_mut_slice(|cids: &mut [ClusterId<u32>]| cids.fill(ClusterId::FREE));
        }

        self.fat_area.alloc_root();

        sector::sync_all();
    }

    pub const fn fat(&self) -> &FatArea {
        &self.fat_area
    }

    pub fn fat_mut(&mut self) -> &mut FatArea {
        &mut self.fat_area
    }

    pub const fn data(&self) -> &DataArea {
        &self.data_area
    }

    pub fn alloc_cluster(&mut self) -> (ClusterId<u32>, Range<SectorId>) {
        let id = self.fat_area.alloc().unwrap();
        let sectors = self
            .data_area
            .cluster(id)
            .inspect_err(|_| log::error!("Exception from {id:?}"))
            .unwrap();

        for sid in sectors.clone() {
            sector::get(sid).lock().zeroize();
        }
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

    pub fn statfs(&self) -> StatFs {
        let bpb = bpb();
        let cluster_sectors = bpb.cluster_sectors() as u64;
        let files = bpb.total_clusters() as u64;

        sector::get(SectorId::new(1))
            .lock()
            .map(0, |fs_info: &FsInfo| {
                let blocks_free = fs_info.free_count() as u64 * cluster_sectors;

                StatFs {
                    block_size: bpb.sector_bytes() as u64,
                    blocks: files * cluster_sectors as u64,
                    blocks_free,
                    blocks_available: blocks_free,
                    files,
                    files_free: fs_info.free_count() as u64,
                    name_cap: 255,
                    ..Default::default()
                }
            })
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
