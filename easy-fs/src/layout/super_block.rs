use crate::MAGIC;

/// 超级块：
/// - 提供文件系统合法性校验；
/// - 定位其它连续区域
#[derive(Debug)]
#[repr(C)]
pub struct SuperBlock {
    /// 魔数：用于校验文件系统合法性
    magic: u32,
    /// 文件系统占据块数
    pub total_blocks: u32,
    pub inode_bitmap_blocks: u32,
    pub inode_area_blocks: u32,
    pub data_bitmap_blocks: u32,
    pub data_area_blocks: u32,
}

impl SuperBlock {
    #[inline]
    pub fn init(
        &mut self,
        total_blocks: u32,
        inode_bitmap_blocks: u32,
        inode_area_blocks: u32,
        data_bitmap_blocks: u32,
        data_area_blocks: u32,
    ) {
        *self = Self {
            magic: MAGIC,
            total_blocks,
            inode_bitmap_blocks,
            inode_area_blocks,
            data_bitmap_blocks,
            data_area_blocks,
        };
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        self.magic == MAGIC
    }
}
