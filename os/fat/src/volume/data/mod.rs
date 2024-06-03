mod dir_entry;

use core::ops::Range;

pub use self::dir_entry::*;
use crate::volume::reserved::{bpb, Bpb};
use crate::{ClusterError, ClusterId, SectorId};

#[derive(Debug)]
pub struct DataArea {
    range: Range<SectorId>,
}

impl DataArea {
    pub fn new(bpb: &Bpb) -> Self {
        let start = bpb.data_area();
        let end = SectorId::new(bpb.total_sectors());
        Self { range: start..end }
    }

    /// 返回簇编号指向的一系列扇区
    ///
    /// 数据区不占有`ClusterId::MIN`前面的簇，所以需要转换计算得到索引指向的扇区。
    pub fn cluster(&self, id: ClusterId<u32>) -> Result<Range<SectorId>, ClusterError> {
        let id = id.validate()?;
        let start = self.range.start + usize::from(id - ClusterId::MIN) * bpb().cluster_sectors();
        let end = (start + bpb().cluster_sectors()).min(self.range.end);
        Ok(start..end)
    }
}
