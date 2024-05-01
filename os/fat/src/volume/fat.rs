use alloc::boxed::Box;
use core::mem;
use core::ops::Range;

use super::reserved::{bpb, Bpb};
use crate::{sector, SectorId};
use crate::{ClusterError, ClusterId};

pub type Fat32 = Box<[ClusterId<u32>]>;

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

    // /// 获取指定簇的链表
    // pub fn get(
    //     &self,
    //     mut id: ClusterId<u32>,
    // ) -> Result<impl Iterator<Item = ClusterId<u32>>, ClusterError> {
    //     let mut clusters = vec![];
    //     let sector_id = self.get_sector(id);
    //
    //     if !self.range.contains(&sector_id) {
    //         return Err(ClusterError::Reserved);
    //     }
    //
    //     loop {
    //         let cluster_index = u32::from(id) as usize % Self::sector_clusters();
    //         let next_cluster_id = sector::get(sector_id).lock().map(
    //             cluster_index * mem::size_of::<u32>(),
    //             |&id: &ClusterId<u32>| id.validate(),
    //         );
    //
    //         match next_cluster_id {
    //             Err(e) if e == ClusterError::Eof => break,
    //             e @ Err(_) => return e,
    //             Ok(next_cluster_id) => {
    //                 clusters.push(next_cluster_id);
    //                 id = next_cluster_id;
    //             }
    //         }
    //     }
    //
    //     Ok(clusters.into_iter())
    // }

    /// 移除整个簇链
    pub fn remove(&self, id: ClusterId<u32>) -> Result<(), ClusterError> {
        let mut id = self.validate_id(id)?;

        loop {
            let (sid, idx) = self.cluster_id_pos(id);
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
    fn get_sector(&self, id: ClusterId<u32>) -> SectorId {
        let sector_index = u32::from(id) as usize / Self::sector_clusters();
        self.range.start + sector_index
    }

    /// 一个扇区能容纳多少条簇编号
    fn sector_clusters() -> usize {
        bpb().sector_bytes() / mem::size_of::<u32>()
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
    fn cluster_id_pos(&self, id: ClusterId<u32>) -> (SectorId, usize) {
        (
            self.get_sector(id),
            u32::from(id) as usize % Self::sector_clusters(),
        )
    }
}
