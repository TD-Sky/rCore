use alloc::sync::Arc;
use block_dev::BlockDevice;

use crate::BLOCK_BITS;
use crate::block_cache;

/// 位图区域内块的结构
type BitmapBlock = [u64; BLOCK_BITS / 64];

/// 位图区域，记录其指示区域的块分配情况
#[derive(Debug)]
pub struct Bitmap {
    /// 位图的起始块
    start_block_id: usize,
    /// 位图占用块数
    blocks: usize,
}

/// 块编号
struct BlockID(u32);

impl Bitmap {
    #[inline]
    pub fn new(start_block_id: usize, blocks: usize) -> Self {
        Self {
            start_block_id,
            blocks,
        }
    }

    /// 位图所指示区域的总块数
    #[inline]
    pub fn capacity(&self) -> usize {
        self.blocks * BLOCK_BITS
    }

    /// 在指示区域内分配新的块，返回其编号。
    /// 若位图的空间用尽，则返回空。
    pub fn alloc(&self, block_device: &Arc<dyn BlockDevice>) -> Option<u32> {
        // 遍历位图区域内所有的块，寻找块内还有剩余空间的bit组(即还有0)
        // 起始块ID + 块索引 = 索引指向块的实际ID
        for block_index in 0..self.blocks {
            let cache = block_cache::get(self.start_block_id + block_index, block_device.clone());
            let mut cache = cache.lock();
            let bitmap_block: &mut BitmapBlock = cache.get_mut(0);

            let Some((group_index, ingroup_index)) =
                bitmap_block
                    .iter()
                    .enumerate()
                    .find_map(|(group_index, &bits)| {
                        (bits != u64::MAX).then_some((group_index, bits.trailing_ones()))
                    })
            else {
                continue;
            };

            // 追加新位
            bitmap_block[group_index] |= 1 << ingroup_index;
            // 计算位图所指示区域内块的编号
            return Some(BlockID::encode(
                block_index,
                group_index,
                ingroup_index as usize,
            ));
        }

        None
    }

    pub fn dealloc(&self, block_device: &Arc<dyn BlockDevice>, block_id: u32) {
        let (block_index, group_index, ingroup_index) = BlockID(block_id).decode();
        let cache = block_cache::get(self.start_block_id + block_index, block_device.clone());
        let mut cache = cache.lock();
        let bitmap_block: &mut BitmapBlock = cache.get_mut(0);

        // 编号一定得有对应的位
        assert_ne!(bitmap_block[group_index] & (1 << ingroup_index), 0);

        bitmap_block[group_index] -= 1 << ingroup_index;
    }
}

impl BlockID {
    /// 线性映射编码得到块ID
    #[inline]
    fn encode(block_index: usize, group_index: usize, ingroup_index: usize) -> u32 {
        (block_index * BLOCK_BITS + group_index * 64 + ingroup_index) as u32
    }

    fn decode(self) -> (usize, usize, usize) {
        let mut block_id = self.0 as usize;

        let block_index = block_id / BLOCK_BITS;
        block_id %= BLOCK_BITS;
        (block_index, block_id / 64, block_id % 64)
    }
}
