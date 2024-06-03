use alloc::vec::Vec;
use alloc::{sync::Arc, vec};
use core::iter::Iterator;
use core::mem;
use core::ops::Range;

use block_dev::BlockDevice;

use crate::volume::{
    data::DataArea,
    fat::Fat,
    reserved::{bpb, init_bpb, Bpb, FsInfo},
};
use crate::{sector, ClusterId, SectorId};

#[derive(Debug)]
pub struct FatFileSystem {
    /// FAT
    fat: Fat,
    /// 数据区
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
            fat: Fat::new(&bpb),
            data_area: DataArea::new(&bpb),
        };
        init_bpb(bpb);
        fs
    }

    pub fn new(disk_size: usize) -> Self {
        let bpb = Bpb::new(disk_size);
        let fs = FatFileSystem {
            fat: Fat::new(&bpb),
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
        sector::get(bpb.backup_boot())
            .lock()
            .map_mut(0, |disk_bpb: &mut Bpb| disk_bpb.clone_from(bpb));

        let fs_info = FsInfo::new(bpb);
        sector::get(bpb.fs_info())
            .lock()
            .map_mut(0, |disk_fs_info: &mut FsInfo| {
                disk_fs_info.clone_from(&fs_info)
            });
        sector::get(SectorId::new(7))
            .lock()
            .map_mut(0, |disk_fs_info: &mut FsInfo| *disk_fs_info = fs_info);

        for sid in self.fat.range() {
            sector::get(sid)
                .lock()
                .map_mut_slice(|cids: &mut [ClusterId<u32>]| cids.fill(ClusterId::FREE));
        }

        self.fat.alloc_root();

        sector::sync_all();
    }

    pub const fn fat(&self) -> &Fat {
        &self.fat
    }

    pub fn fat_mut(&mut self) -> &mut Fat {
        &mut self.fat
    }

    pub const fn data(&self) -> &DataArea {
        &self.data_area
    }

    pub fn alloc_cluster(&mut self) -> (ClusterId<u32>, Range<SectorId>) {
        let id = self.fat.alloc().unwrap();
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

    pub fn data_sector_cursor(&self, start_cluster: ClusterId<u32>) -> SectorCursor {
        SectorCursor::new(start_cluster, self)
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
        self.id = self.control.fat.next(id).unwrap();
        Some(sectors)
    }
}

#[derive(Debug)]
pub struct SectorCursor<'a> {
    current: (usize, Range<SectorId>, usize, SectorId),
    clusters: Vec<ClusterId>,
    control: &'a FatFileSystem,
}

impl<'a> SectorCursor<'a> {
    pub fn new(start_cluster: ClusterId<u32>, control: &'a FatFileSystem) -> Self {
        let sids = control.data_area.cluster(start_cluster).unwrap();
        let start_sector = sids.start;

        Self {
            current: (0, sids, 0, start_sector),
            clusters: vec![start_cluster],
            control,
        }
    }

    pub fn sector(&self) -> SectorId {
        self.current.3
    }

    /// 向后搜寻目标扇区，移动。若搜寻无果则返回`None`，游标停留在最后一个扇区。
    pub fn find(&mut self, sector: SectorId) -> Option<&mut Self> {
        let mut cur = self;
        loop {
            if cur.sector() == sector {
                return Some(cur);
            }
            cur = cur.next()?;
        }
    }

    /// 前向搜寻目标扇区，移动。若搜寻无果则返回`None`，游标停留在第一个扇区。
    pub fn rfind(&mut self, sector: SectorId) -> Option<&mut Self> {
        let mut cur = self;
        loop {
            if cur.sector() == sector {
                return Some(cur);
            }
            cur = cur.prev()?;
        }
    }

    /// 移动到上一个扇区，没有时返回`None`且状态不变。
    pub fn prev(&mut self) -> Option<&mut Self> {
        let (cindex, sids, sindex, sid) = &mut self.current;

        if let Some(prev_si) = sindex.checked_sub(1) {
            // 簇不变，扇区前移

            *sindex = prev_si;
            *sid = sids.clone().nth(prev_si).expect("nth has been checked");

            Some(self)
        } else if let Some(prev_ci) = cindex.checked_sub(1) {
            // 簇前移，扇区变为上一个簇的末扇区

            *cindex = prev_ci;
            *sids = self
                .control
                .data_area
                .cluster(self.clusters[prev_ci])
                .unwrap();
            *sindex = bpb().cluster_sectors() - 1;
            *sid = sids.clone().nth(*sindex).expect("nth has been checked");

            Some(self)
        } else {
            None
        }
    }

    /// 移动到下一个扇区，没有时返回`None`且状态不变。。
    pub fn next(&mut self) -> Option<&mut Self> {
        let (cindex, sids, sindex, sid) = &mut self.current;

        if let Some(next_sid) = sids.clone().nth(*sindex + 1) {
            // 簇不变，扇区后移
            *sindex += 1;
            *sid = next_sid;

            Some(self)
        } else {
            // 簇后移，扇区变为下一个簇的头扇区
            let next_ci = *cindex + 1;

            let next_cid = match self.clusters.get(next_ci) {
                Some(&next_cid) => next_cid,
                None => {
                    let next_cid = self.control.fat.next(self.clusters[*cindex]).unwrap()?;
                    self.clusters.push(next_cid);
                    next_cid
                }
            };

            *cindex = next_ci;
            *sids = self.control.data_area.cluster(next_cid).unwrap();
            *sindex = 0;
            *sid = sids.start;

            Some(self)
        }
    }
}
