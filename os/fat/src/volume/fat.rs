use core::mem;
use core::ops::Range;

use crate::volume::reserved::{self, bpb, Bpb};
use crate::{sector, SectorId};
use crate::{ClusterError, ClusterId};

/// File Allocation Table
#[derive(Debug)]
pub struct Fat {
    range: Range<SectorId>,
}

impl Fat {
    pub fn new(bpb: &Bpb) -> Self {
        let start = bpb.fat_area();
        let end = start + bpb.fat_sectors();

        Self {
            range: Range { start, end },
        }
    }

    pub fn range(&self) -> Range<SectorId> {
        self.range.clone()
    }

    /// 获取下一个簇编号。
    /// 若`id`指向未分配簇，则报错。
    /// `Ok(None)`表示`id`为链表上最后一个簇。
    pub fn next(&self, id: ClusterId<u32>) -> Result<Option<ClusterId<u32>>, ClusterError> {
        let id = self.validate_id(id)?;

        match self.id2pos(id).access(|next_id| next_id.validate()) {
            Ok(cid) => Ok(Some(cid)),
            Err(ClusterError::Eof) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// 寻找簇链表的最后一个簇
    pub fn last(&self, id: ClusterId<u32>) -> Result<ClusterId<u32>, ClusterError> {
        let mut id = self.validate_id(id)?;

        while let Some(next_id) = self.next(id)? {
            id = next_id;
        }

        Ok(id)
    }

    /// 分配根目录
    pub fn alloc_root(&mut self) {
        sector::get(self.range.start)
            .lock()
            .map_mut_slice(|cids: &mut [ClusterId<u32>]| {
                cids[0] = ClusterId::new(0xFF_FF_FF_00 + bpb().media as u32);
                // WARN: 标准中要求FAT[1]除了标志位，其它均设为1，
                //       而`ClusterId::new`内部会进行一次掩码，应该没关系？
                cids[1] = ClusterId::new((Self::SET_CLN_SHUT + Self::SET_HRD_ERR) | u32::MAX);
                cids[2] = ClusterId::EOF;
            });

        reserved::record_alloc();
    }

    /// 寻找未分配的簇，并将其设为`EOF`。
    ///
    /// 此方法仅在FAT表做注册，不会初始化簇。
    /// 若需要初始化簇，请调用[`FatFileSystem::alloc_cluster`]。
    ///
    /// [`FatFileSystem::alloc_cluster`]: crate::FatFileSystem::alloc_cluster
    pub fn alloc(&mut self) -> Option<ClusterId<u32>> {
        let sector_clusters = Self::sector_clusters();

        for (i, sid) in self.range.clone().enumerate() {
            if let Some(cidx) =
                sector::get(sid)
                    .lock()
                    .map_mut_slice(|clusters: &mut [ClusterId<u32>]| {
                        clusters
                            .iter_mut()
                            .enumerate()
                            .find(|(_, cid)| **cid == ClusterId::FREE)
                            .map(|(cidx, cid)| {
                                *cid = ClusterId::EOF;
                                cidx
                            })
                    })
            {
                reserved::record_alloc();
                return Some(ClusterId::from(i * sector_clusters + cidx));
            }
        }

        None
    }

    /// 以前后顺序链接两个簇，为扩展分配准备的。
    ///
    /// # Safety
    ///
    /// 若`prev`不是尾簇，赋予其`next`的链接会导致链表的剩余部分丢失！
    pub unsafe fn couple(&mut self, prev: ClusterId<u32>, next: ClusterId<u32>) {
        self.id2pos(prev).access_mut(|next_id| *next_id = next);
    }

    /// 移除整个簇链表。
    pub fn dealloc(&mut self, id: ClusterId<u32>) -> Result<(), ClusterError> {
        let mut id = self.validate_id(id)?;

        loop {
            let is_eof = self.id2pos(id).access_mut(|next_id| {
                id = *next_id;
                *next_id = ClusterId::FREE;
                id == ClusterId::EOF
            });
            reserved::record_free();
            if is_eof {
                break;
            }
        }

        Ok(())
    }
}

impl Fat {
    const SET_CLN_SHUT: u32 = 0x08000000;
    const SET_HRD_ERR: u32 = 0x04000000;

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

    fn id2pos(&self, id: ClusterId<u32>) -> ClusterIdPos {
        ClusterIdPos {
            sector: self.get_sector(id),
            nth: u32::from(id) as usize % Self::sector_clusters(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClusterIdPos {
    sector: SectorId,
    nth: usize,
}

impl ClusterIdPos {
    pub fn access<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&ClusterId) -> R,
    {
        sector::get(self.sector)
            .lock()
            .map(self.nth * mem::size_of::<ClusterId>(), f)
    }

    pub fn access_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut ClusterId) -> R,
    {
        sector::get(self.sector)
            .lock()
            .map_mut(self.nth * mem::size_of::<ClusterId>(), f)
    }
}
