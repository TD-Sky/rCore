//! 卷的布局
//!
//! 保留区 | FAT区 | 根目录(FAT12/16) | 数据区

pub mod data;
pub mod fat;
pub mod reserved;

#[cfg(test)]
mod tests {
    use core::mem;

    use super::{
        data::{LongDirEntry, ShortDirEntry},
        reserved::{Bpb, FsInfo},
    };

    #[test]
    fn volume() {
        assert_eq!(512, mem::size_of::<Bpb>());
        assert_eq!(512, mem::size_of::<FsInfo>());
        assert_eq!(32, mem::size_of::<ShortDirEntry>());
        assert_eq!(32, mem::size_of::<LongDirEntry>())
    }
}
