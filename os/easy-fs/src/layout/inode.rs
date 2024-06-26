//! 间接索引块
//! - 一级：整个块连续存储**块编号**，每个编号都指向一个**数据块**
//! - 二级：整个块连续存储**块编号**，每个编号都指向一个一级索引块
//! - 三级：整个块连续存储**块编号**，每个编号都指向一个二级索引块
//!
//! 目录的空间用于存放子项的元信息；
//! 文件的空间用于存放它的数据。
//!
//! ## 块索引编码
//!
//! - x+1 级块索引模 x 级块的**可编号数量**，可得**最后**一块 x 的内部索引
//! - x+1 级块索引除以 x 级块的**可编号数量**，可得 x 级块的位置

use alloc::sync::Arc;
use alloc::vec::Vec;
use block_dev::BlockDevice;

use crate::block_cache;
use crate::DataBlock;
use crate::BLOCK_SIZE;

/// 间接索引块的编号容量
const INDIRECT_COUNT: usize = BLOCK_SIZE / 4;
/// 间接索引块
type IndirectBlock = [u32; INDIRECT_COUNT];

/// 直接索引块可编号数量
const DIRECT_COUNT: usize = 26;
/// 一级索引块可编号数量
const INDIRECT1_COUNT: usize = INDIRECT_COUNT;
/// 二级索引块可编号数量
const INDIRECT2_COUNT: usize = INDIRECT_COUNT.pow(2);
#[allow(dead_code)]
/// 三级索引块可编号数量
const INDIRECT3_COUNT: usize = INDIRECT_COUNT.pow(3);
/// 直接索引时的编号容量
const DIRECT_CAP: usize = DIRECT_COUNT;
/// 用上一级索引时的编号容量
const INDIRECT1_CAP: usize = DIRECT_CAP + INDIRECT1_COUNT;
/// 用上二级索引时的编号容量
const INDIRECT2_CAP: usize = INDIRECT1_CAP + INDIRECT2_COUNT;
#[allow(dead_code)]
/// 用上三级索引时的编号容量
const INDIRECT3_CAP: usize = INDIRECT2_CAP + INDIRECT3_COUNT;

#[derive(Default)]
#[repr(C)]
pub struct DiskInode {
    /// ID
    pub id: u32,
    // 不用usize是为了严控布局
    pub size: u32,
    /// 硬链接个数
    pub links: u32,
    /// 类型
    pub kind: DiskInodeKind,
    /// 直接索引块，包含 DIRECT_COUNT 个块编号，
    /// 存储容量：DIRECT_CAP * BLOCK_SIZE 字节
    direct: [u32; DIRECT_COUNT],
    /// 指向一个一级索引块
    indirect1: u32,
    /// 指向一个二级索引块
    indirect2: u32,
    /// 指向一个三级索引块
    indirect3: u32,
}

#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub enum DiskInodeKind {
    #[default]
    File,
    Directory,
}

impl DiskInode {
    #[inline]
    pub fn init(&mut self, id: u32, kind: DiskInodeKind) {
        *self = Self {
            id,
            links: 1,
            kind,
            ..Default::default()
        }
    }

    #[inline]
    pub fn is_dir(&self) -> bool {
        self.kind == DiskInodeKind::Directory
    }

    /// 逻辑上 inode 指向一系列数据块，此处传入的是这些数据块的索引（逻辑索引），
    /// 然后返回给**块缓存层**使用的ID
    pub fn block_id(&self, block_index: u32, block_device: &Arc<dyn BlockDevice>) -> u32 {
        let block_index = block_index as usize;

        if block_index < DIRECT_CAP {
            self.direct[block_index]
        } else if block_index < INDIRECT1_CAP {
            block_cache::get(self.indirect1 as usize, block_device.clone())
                .lock()
                .map(0, |indirect_block: &IndirectBlock| {
                    // 剔去直接索引的部分
                    indirect_block[block_index - DIRECT_CAP]
                })
        } else if block_index < INDIRECT2_CAP {
            // 剔去使用了一级索引的部分
            let index = block_index - INDIRECT1_CAP;

            // 数量上二级索引有128个INDIRECT1_COUNT
            let indirect1 = block_cache::get(self.indirect2 as usize, block_device.clone())
                .lock()
                .map(0, |indirect2: &IndirectBlock| {
                    indirect2[index / INDIRECT1_COUNT]
                });
            block_cache::get(indirect1 as usize, block_device.clone())
                .lock()
                .map(0, |indirect1: &IndirectBlock| {
                    indirect1[index % INDIRECT1_COUNT]
                })
        } else {
            // 剔去使用了二级索引的部分
            let index = block_index - INDIRECT2_CAP;

            // 数量上三级索引有128个INDIRECT2_COUNT
            let indirect2 = block_cache::get(self.indirect3 as usize, block_device.clone())
                .lock()
                .map(0, |indirect3: &IndirectBlock| {
                    indirect3[index / INDIRECT2_COUNT]
                });
            let indirect1 = block_cache::get(indirect2 as usize, block_device.clone())
                .lock()
                .map(0, |indirect2: &IndirectBlock| {
                    indirect2[index % INDIRECT2_COUNT / INDIRECT1_COUNT]
                });
            block_cache::get(indirect1 as usize, block_device.clone())
                .lock()
                .map(0, |indirect1: &IndirectBlock| {
                    // 视三级索引块的单元为一级索引块，
                    // 取模INDIRECT1_COUNT即可得到index
                    // 所指向一级索引块内的位置
                    indirect1[index % INDIRECT1_COUNT]
                })
        }
    }

    pub fn expand_to(
        &mut self,
        larger_size: u32,
        new_blocks: Vec<u32>,
        block_device: &Arc<dyn BlockDevice>,
    ) {
        let mut block_index = Self::count_data_block(self.size);
        self.size = larger_size;
        let mut new_total_blocks = Self::count_data_block(self.size);
        let mut new_blocks = new_blocks.into_iter();

        /******************** 直接索引 ********************/
        // 填充直接索引
        while block_index < new_total_blocks.min(DIRECT_COUNT) {
            self.direct[block_index] = new_blocks.next().unwrap();
            block_index += 1;
        }
        /******************** END ********************/

        if new_total_blocks <= DIRECT_COUNT {
            return;
        }

        /******************** 一级索引 ********************/
        // 这次size的增加经过了DIRECT_CAP，创建一级索引
        if block_index == DIRECT_COUNT {
            self.indirect1 = new_blocks.next().unwrap();
        }

        block_index -= DIRECT_COUNT;
        new_total_blocks -= DIRECT_COUNT;

        // 填充一级索引
        block_cache::get(self.indirect1 as usize, block_device.clone())
            .lock()
            .map_mut(0, |indirect1: &mut IndirectBlock| {
                while block_index < new_total_blocks.min(INDIRECT1_COUNT) {
                    indirect1[block_index] = new_blocks.next().unwrap();
                    block_index += 1;
                }
            });
        /******************** END ********************/

        if new_total_blocks <= INDIRECT1_COUNT {
            return;
        }

        /******************** 二级索引 ********************/
        // 这次size的增加经过了INDIRECT1_CAP，创建二级索引
        if block_index == INDIRECT1_COUNT {
            self.indirect2 = new_blocks.next().unwrap();
        }

        block_index -= INDIRECT1_COUNT;
        new_total_blocks -= INDIRECT1_COUNT;

        // 填充二级索引
        let mut index2 = block_index / INDIRECT1_COUNT;
        let mut index1 = block_index % INDIRECT1_COUNT;
        let new_end2 = new_total_blocks / INDIRECT1_COUNT;
        let new_end1 = new_total_blocks % INDIRECT1_COUNT;
        block_cache::get(self.indirect2 as usize, block_device.clone())
            .lock()
            .map_mut(0, |indirect2: &mut IndirectBlock| {
                // 索引一旦呈树状，就无法用 `new_total_blocks.min(COUNT)` 做限制了；
                // new_total_blocks 的限制通过 new_end 达成
                while block_index < INDIRECT2_COUNT
                    && ((index2 < new_end2) || (index2 == new_end2 && index1 < new_end1))
                {
                    // 子块索引为0表示进入新块
                    if index1 == 0 {
                        indirect2[index2] = new_blocks.next().unwrap();
                        block_index += 1;
                    }

                    block_cache::get(indirect2[index2] as usize, block_device.clone())
                        .lock()
                        .map_mut(0, |indirect1: &mut IndirectBlock| {
                            indirect1[index1] = new_blocks.next().unwrap();
                            block_index += 1;
                        });

                    index1 += 1;
                    if index1 == INDIRECT1_COUNT {
                        index1 = 0;
                        index2 += 1;
                    }
                }
            });
        /******************** END ********************/

        if new_total_blocks <= INDIRECT2_COUNT {
            return;
        }

        /******************** 三级索引 ********************/
        // 这次size的增加经过了INDIRECT2_CAP，创建三级索引
        if block_index == INDIRECT2_COUNT {
            self.indirect3 = new_blocks.next().unwrap();
        }

        block_index -= INDIRECT2_COUNT;
        new_total_blocks -= INDIRECT2_COUNT;

        // 填充三级索引
        let mut index3 = block_index / INDIRECT2_COUNT;
        let mut index2 = block_index % INDIRECT2_COUNT / INDIRECT1_COUNT;
        let mut index1 = block_index % INDIRECT1_COUNT;
        let new_end3 = new_total_blocks / INDIRECT2_COUNT;
        let new_end2 = new_total_blocks % INDIRECT2_COUNT / INDIRECT1_COUNT;
        let new_end1 = new_total_blocks % INDIRECT1_COUNT;
        block_cache::get(self.indirect3 as usize, block_device.clone())
            .lock()
            .map_mut(0, |indirect3: &mut IndirectBlock| {
                while (index3 < new_end3)
                    || (index3 == new_end3 && index2 < new_end2)
                    || (index3 == new_end3 && index2 == new_end2 && index1 < new_end1)
                {
                    if index2 == 0 {
                        // 未初始化块该怎么用由我安排
                        indirect3[index3] = new_blocks.next().unwrap();
                        block_index += 1;
                    }

                    block_cache::get(indirect3[index3] as usize, block_device.clone())
                        .lock()
                        .map_mut(0, |indirect2: &mut IndirectBlock| {
                            if index1 == 0 {
                                indirect2[index2] = new_blocks.next().unwrap();
                                block_index += 1;
                            }

                            block_cache::get(indirect2[index2] as usize, block_device.clone())
                                .lock()
                                .map_mut(0, |indirect1: &mut IndirectBlock| {
                                    indirect1[index1] = new_blocks.next().unwrap();
                                    block_index += 1;
                                });
                        });

                    index1 += 1;
                    if index1 == INDIRECT1_COUNT {
                        index1 = 0;
                        index2 += 1;
                        if index2 == INDIRECT2_COUNT {
                            index2 = 0;
                            index3 += 1;
                        }
                    }
                }
            });
        /******************** END ********************/
    }

    pub fn clear(&mut self, block_device: &Arc<dyn BlockDevice>) -> Vec<u32> {
        let mut drop_data_blocks: Vec<u32> = Vec::with_capacity(Self::count_total_block(self.size));
        let mut data_blocks = Self::count_data_block(self.size);
        self.size = 0;

        /******************** 直接索引 ********************/
        drop_data_blocks.extend_from_slice(&self.direct[..data_blocks.min(DIRECT_CAP)]);
        self.direct.fill(0);
        /******************** END ********************/

        if data_blocks <= DIRECT_COUNT {
            return drop_data_blocks;
        }

        /******************** 一级索引 ********************/
        drop_data_blocks.push(self.indirect1);
        data_blocks -= DIRECT_COUNT;

        block_cache::get(self.indirect1 as usize, block_device.clone())
            .lock()
            .map_mut(0, |indirect1: &mut IndirectBlock| {
                drop_data_blocks.extend_from_slice(&indirect1[..data_blocks.min(INDIRECT1_COUNT)]);
            });
        self.indirect1 = 0;
        /******************** END ********************/

        if data_blocks <= INDIRECT1_COUNT {
            return drop_data_blocks;
        }

        /******************** 二级索引 ********************/
        drop_data_blocks.push(self.indirect2);
        data_blocks -= INDIRECT1_COUNT;

        let index2 = if data_blocks <= INDIRECT2_COUNT {
            data_blocks / INDIRECT1_COUNT
        } else {
            // 拥有超出二级容量的块数，直接清空整个二级索引
            INDIRECT_COUNT
        };
        block_cache::get(self.indirect2 as usize, block_device.clone())
            .lock()
            .map(0, |indirect2: &IndirectBlock| {
                // 遍历 index2 之前的所有ID
                for &block in indirect2.iter().take(index2) {
                    drop_data_blocks.push(block);
                    block_cache::get(block as usize, block_device.clone())
                        .lock()
                        .map(0, |indirect1: &IndirectBlock| {
                            drop_data_blocks.extend_from_slice(indirect1);
                        });
                }

                // 若索引只有二级，则取 index2 所指引的最后一块
                // 一级索引在 index1 之前的全部ID
                let index1 = data_blocks % INDIRECT1_COUNT;
                if index1 > 0 && index2 != INDIRECT_COUNT {
                    drop_data_blocks.push(indirect2[index2]);
                    block_cache::get(indirect2[index2] as usize, block_device.clone())
                        .lock()
                        .map(0, |indirect1: &IndirectBlock| {
                            drop_data_blocks.extend_from_slice(&indirect1[..index1]);
                        });
                }
            });
        self.indirect2 = 0;
        /******************** END ********************/

        if data_blocks <= INDIRECT2_COUNT {
            return drop_data_blocks;
        }

        /******************** 三级索引 ********************/
        // NOTE: 索引最深为三级时才需要写
        assert!(data_blocks <= INDIRECT3_COUNT);
        drop_data_blocks.push(self.indirect3);
        data_blocks -= INDIRECT2_COUNT;

        let index3 = data_blocks / INDIRECT2_COUNT;

        block_cache::get(self.indirect3 as usize, block_device.clone())
            .lock()
            .map(0, |indirect3: &IndirectBlock| {
                for &block in indirect3.iter().take(index3) {
                    drop_data_blocks.push(block);
                    block_cache::get(block as usize, block_device.clone())
                        .lock()
                        .map(0, |indirect2: &IndirectBlock| {
                            for &block in indirect2 {
                                drop_data_blocks.push(block);
                                block_cache::get(block as usize, block_device.clone())
                                    .lock()
                                    .map(0, |indirect1: &IndirectBlock| {
                                        drop_data_blocks.extend_from_slice(indirect1);
                                    });
                            }
                        });
                }

                let index2 = data_blocks % INDIRECT2_COUNT / INDIRECT1_COUNT;
                if index2 > 0 {
                    drop_data_blocks.push(indirect3[index3]);
                    block_cache::get(indirect3[index3] as usize, block_device.clone())
                        .lock()
                        .map(0, |indirect2: &IndirectBlock| {
                            for &block in indirect2.iter().take(index2) {
                                drop_data_blocks.push(block);
                                block_cache::get(block as usize, block_device.clone())
                                    .lock()
                                    .map(0, |indirect1: &IndirectBlock| {
                                        drop_data_blocks.extend_from_slice(indirect1);
                                    });
                            }

                            let index1 = data_blocks % INDIRECT1_COUNT;
                            if index1 > 0 {
                                drop_data_blocks.push(indirect2[index2]);
                                block_cache::get(indirect2[index2] as usize, block_device.clone())
                                    .lock()
                                    .map(0, |indirect1: &IndirectBlock| {
                                        drop_data_blocks.extend_from_slice(&indirect1[..index1]);
                                    });
                            }
                        });
                }
            });

        self.indirect3 = 0;
        /******************** END ********************/

        drop_data_blocks
    }

    /// 从指定位置(字节偏移)读出数据填充`buf`
    pub fn read_at(
        &self,
        offset: usize,
        buf: &mut [u8],
        block_device: &Arc<dyn BlockDevice>,
    ) -> usize {
        let mut start = offset;
        let end = (start + buf.len()).min(self.size as usize);

        if start > end {
            return 0;
        }

        // 已读取多少字节
        let mut read_size = 0;
        loop {
            // 当前块的逻辑索引，见 `Inode::block_id`
            let block_index = start / BLOCK_SIZE;
            // 当前块的末地址(字节)
            let current_block_end = ((block_index + 1) * BLOCK_SIZE).min(end);
            let block_read_size = current_block_end - start;
            let dest = &mut buf[read_size..read_size + block_read_size];

            block_cache::get(
                self.block_id(block_index as u32, block_device) as usize,
                block_device.clone(),
            )
            .lock()
            .map(0, |data_block: &DataBlock| {
                // 绝对地址 % 块大小 = 块内偏移
                let src = &data_block[start % BLOCK_SIZE..start % BLOCK_SIZE + block_read_size];
                dest.copy_from_slice(src);
            });

            read_size += block_read_size;

            if current_block_end == end {
                break;
            }

            start = current_block_end;
        }

        read_size
    }

    pub fn write_at(
        &mut self,
        offset: usize,
        buf: &[u8],
        block_device: &Arc<dyn BlockDevice>,
    ) -> usize {
        let mut start = offset;
        let end = (start + buf.len()).min(self.size as usize);

        assert!(start <= end);

        let mut written_size = 0;
        loop {
            let block_index = start / BLOCK_SIZE;
            let current_block_end = ((block_index + 1) * BLOCK_SIZE).min(end);
            let block_write_size = current_block_end - start;

            block_cache::get(
                self.block_id(block_index as u32, block_device) as usize,
                block_device.clone(),
            )
            .lock()
            .map_mut(0, |data_block: &mut DataBlock| {
                let src = &buf[written_size..written_size + block_write_size];
                let dest =
                    &mut data_block[start % BLOCK_SIZE..start % BLOCK_SIZE + block_write_size];
                dest.copy_from_slice(src);
            });

            written_size += block_write_size;

            if current_block_end == end {
                break;
            }

            start = current_block_end;
        }

        written_size
    }

    /// 计算容纳指定数据量需要多少个**数据块**
    #[inline]
    pub fn count_data_block(size: u32) -> usize {
        (size as usize).div_ceil(BLOCK_SIZE)
    }

    /// 计算容纳指定数据量需要多少个 **数据块** 和 **索引块**(`IndirectBlock`)
    pub fn count_total_block(size: u32) -> usize {
        let data_blocks = Self::count_data_block(size);
        let mut total = data_blocks;

        // 超出直接索引，使用一级索引块，
        if data_blocks > DIRECT_CAP {
            total += 1;
        }

        // 超出一级索引，使用二级索引块
        if data_blocks > INDIRECT1_CAP {
            total += 1 + (data_blocks - INDIRECT1_CAP).div_ceil(INDIRECT_COUNT);
        }

        // 超出二级索引，使用三级索引块
        if data_blocks > INDIRECT2_CAP {
            total += 1 + (data_blocks - INDIRECT2_CAP).div_ceil(INDIRECT_COUNT);
        }

        total
    }
}
