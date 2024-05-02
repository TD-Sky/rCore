use core::mem;
use core::ops::Range;

use crate::volume::reserved::{bpb, Bpb};
use crate::{sector, SectorId};
use crate::{ClusterError, ClusterId};

#[derive(Debug)]
pub struct FatArea {
    range: Range<SectorId>,
}

impl FatArea {
    pub fn new(bpb: &Bpb) -> Self {
        let start = bpb.fat_area_sector();
        let end = start + bpb.fat_count() * bpb.fat_sectors();
        Self {
            range: Range { start, end },
        }
    }

    /// 获取下一个簇编号。
    /// 若`id`指向未分配簇，则报错。
    /// `Ok(None)`表示`id`为链表上最后一个簇。
    pub fn next(&self, id: ClusterId<u32>) -> Result<Option<ClusterId<u32>>, ClusterError> {
        let id = self.validate_id(id)?;

        let (sid, idx) = self.cluster_id2pos(id);
        match sector::get(sid).lock().map(
            idx * mem::size_of::<ClusterId<u32>>(),
            |cid: &ClusterId<u32>| cid.validate(),
        ) {
            Ok(cid) => Ok(Some(cid)),
            Err(ClusterError::Eof) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// 寻找未分配的簇。
    pub fn alloc(&self) -> Option<ClusterId<u32>> {
        let mut range = self.range.clone();

        if let Some(cidx) =
            sector::get(range.next()?)
                .lock()
                .map_slice(|clusters: &[ClusterId<u32>]| {
                    clusters
                        .iter()
                        .skip(ClusterId::MIN.into())
                        .position(|&cid| cid == ClusterId::FREE)
                        .map(|cidx| cidx + 2)
                })
        {
            return Some((cidx as u32).into());
        }

        for (i, sid) in range.enumerate() {
            if let Some(cidx) =
                sector::get(sid)
                    .lock()
                    .map_slice(|clusters: &[ClusterId<u32>]| {
                        clusters.iter().position(|&cid| cid == ClusterId::FREE)
                    })
            {
                return Some(Self::pos2cluster_id(i + 1, cidx));
            }
        }

        None
    }

    /// 移除整个簇链表。
    pub fn remove(&self, id: ClusterId<u32>) -> Result<(), ClusterError> {
        let mut id = self.validate_id(id)?;

        loop {
            let (sid, idx) = self.cluster_id2pos(id);
            let is_eof = sector::get(sid).lock().map_mut(
                idx * mem::size_of::<ClusterId<u32>>(),
                |next_id: &mut ClusterId| {
                    id = *next_id;
                    *next_id = ClusterId::FREE;
                    id == ClusterId::EOF
                },
            );
            if is_eof {
                break;
            }
        }

        Ok(())
    }
}

impl FatArea {
    /// 一个扇区能容纳多少条簇编号
    fn sector_clusters() -> usize {
        bpb().sector_bytes() / mem::size_of::<u32>()
    }

    /// 获取`id`所在扇区
    fn get_sector(&self, id: ClusterId<u32>) -> SectorId {
        let sector_index = usize::from(id) / Self::sector_clusters();
        self.range.start + sector_index
    }

    fn validate_id(&self, id: ClusterId<u32>) -> Result<ClusterId<u32>, ClusterError> {
        id.validate().and_then(|id| {
            if self.range.contains(&self.get_sector(id)) {
                Ok(id)
            } else {
                Err(ClusterError::Reserved)
            }
        })
    }

    /// 返回簇编号实际所处的磁盘位置（扇区号 + 扇区内索引）
    fn cluster_id2pos(&self, id: ClusterId<u32>) -> (SectorId, usize) {
        (
            self.get_sector(id),
            u32::from(id) as usize % Self::sector_clusters(),
        )
    }

    const fn pos2cluster_id(sector_index: usize, cluster_index: usize) -> ClusterId<u32> {
        ClusterId::new((sector_index * cluster_index) as u32)
    }
}
