//! # 磁盘块管理器层
//!
//! 构建出磁盘的布局并使用。

use core::mem;

use alloc::sync::Arc;
use block_dev::BlockDevice;
use spin::Mutex;

use crate::DataBlock;
use crate::Inode;
use crate::block_cache;
use crate::layout::*;
use crate::{BLOCK_BITS, BLOCK_SIZE};

const INODE_SIZE: usize = mem::size_of::<DiskInode>();
const INODES_PER_BLOCK: usize = BLOCK_SIZE / INODE_SIZE;

#[derive(Debug)]
pub struct EasyFileSystem {
    block_device: Arc<dyn BlockDevice>,
    inode_bitmap: Bitmap,
    data_bitmap: Bitmap,
    inode_area_start_block: u32,
    data_area_start_block: u32,
}

impl EasyFileSystem {
    pub fn new(
        block_device: Arc<dyn BlockDevice>,
        total_blocks: u32,
        inode_bitmap_blocks: u32,
    ) -> Arc<Mutex<Self>> {
        let inode_bitmap = Bitmap::new(1, inode_bitmap_blocks as usize);
        let inode_area_cap = inode_bitmap.capacity();
        let inode_area_blocks =
            ((inode_area_cap * mem::size_of::<DiskInode>() + BLOCK_SIZE - 1) / BLOCK_SIZE) as u32;
        let inode_total_blocks = inode_bitmap_blocks + inode_area_blocks;

        let data_total_blocks = total_blocks - 1 - inode_total_blocks;
        let data_bitmap_blocks = (data_total_blocks + BLOCK_BITS as u32) / (BLOCK_BITS as u32 + 1);
        let data_area_blocks = data_total_blocks - data_bitmap_blocks;
        let data_bitmap = Bitmap::new(
            (1 + inode_bitmap_blocks + inode_area_blocks) as usize,
            data_bitmap_blocks as usize,
        );

        let mut efs = Self {
            block_device: block_device.clone(),
            inode_bitmap,
            data_bitmap,
            inode_area_start_block: 1 + inode_bitmap_blocks,
            data_area_start_block: 1 + inode_total_blocks + data_bitmap_blocks,
        };

        for i in 0..total_blocks {
            block_cache::get(i as usize, block_device.clone())
                .lock()
                .map_mut(0, |data_block: &mut DataBlock| data_block.fill(0));
        }

        block_cache::get(0, block_device.clone()).lock().map_mut(
            0,
            |super_block: &mut SuperBlock| {
                super_block.init(
                    total_blocks,
                    inode_bitmap_blocks,
                    inode_area_blocks,
                    data_bitmap_blocks,
                    data_area_blocks,
                )
            },
        );

        assert_eq!(efs.alloc_inode(), 0);
        let (root_inode_block_id, root_inode_offset) = efs.disk_inode_pos(0);
        block_cache::get(root_inode_block_id as usize, block_device)
            .lock()
            .map_mut(root_inode_offset, |disk_inode: &mut DiskInode| {
                disk_inode.init(0, DiskInodeKind::Directory)
            });
        block_cache::sync_all();

        Arc::new(Mutex::new(efs))
    }

    pub fn open(block_device: Arc<dyn BlockDevice>) -> Arc<Mutex<Self>> {
        block_cache::get(0, block_device.clone())
            .lock()
            .map(0, |super_block: &SuperBlock| {
                assert!(super_block.is_valid(), "error when loading EFS");

                let inode_total_blocks =
                    super_block.inode_bitmap_blocks + super_block.inode_area_blocks;
                let efs = Self {
                    block_device,
                    inode_bitmap: Bitmap::new(1, super_block.inode_bitmap_blocks as usize),
                    data_bitmap: Bitmap::new(
                        1 + inode_total_blocks as usize,
                        super_block.data_bitmap_blocks as usize,
                    ),
                    inode_area_start_block: 1 + super_block.inode_bitmap_blocks,
                    data_area_start_block: 1 + inode_total_blocks + super_block.data_bitmap_blocks,
                };

                Arc::new(Mutex::new(efs))
            })
    }

    /// 在磁盘上分配新的 inode 并返回其ID
    #[inline]
    pub fn alloc_inode(&mut self) -> u32 {
        self.inode_bitmap.alloc(&self.block_device).unwrap()
    }

    /// 在磁盘上分配新的数据块并返回其ID
    #[inline]
    pub fn alloc_data(&mut self) -> u32 {
        self.data_area_start_block + self.data_bitmap.alloc(&self.block_device).unwrap()
    }

    pub fn dealloc_data(&mut self, block_id: u32) {
        block_cache::get(block_id as usize, self.block_device.clone())
            .lock()
            .map_mut(0, |data_block: &mut DataBlock| data_block.fill(0));
        self.data_bitmap
            .dealloc(&self.block_device, block_id - self.data_area_start_block)
    }

    /// 通过ID获取 inode 在磁盘上的位置：**块ID**以及**块内偏移**
    pub fn disk_inode_pos(&self, inode_id: u32) -> (u32, usize) {
        let block_id = self.inode_area_start_block + inode_id / INODES_PER_BLOCK as u32;
        let block_inoffset = inode_id as usize % INODES_PER_BLOCK * INODE_SIZE;

        (block_id, block_inoffset)
    }

    pub fn root_inode(efs: &Arc<Mutex<Self>>) -> Inode {
        let block_device = efs.lock().block_device.clone();
        let (block_id, block_offset) = efs.lock().disk_inode_pos(0);
        Inode::new(block_id, block_offset, efs.clone(), block_device)
    }
}
