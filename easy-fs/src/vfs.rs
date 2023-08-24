//! # 索引节点层
//!
//! 位于内存的虚拟文件系统，确立了文件系统的操作逻辑：
//! 通过多个 [`Inode`] 形成文件树。

use alloc::sync::Arc;
use alloc::vec::Vec;

use enumflags2::bitflags;
use spin::Mutex;

use crate::block_cache;
use crate::layout::DirEntry;
use crate::layout::{DiskInode, DiskInodeKind};
use crate::BlockDevice;
use crate::EasyFileSystem;

pub struct Inode {
    /// inode所在块
    block_id: usize,
    /// inode的块内偏移
    block_offset: usize,
    fs: Arc<Mutex<EasyFileSystem>>,
    block_device: Arc<dyn BlockDevice>,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct Stat {
    pub dev: u64,
    pub inode: u64,
    pub kind: StatKind,
    pub links: u32,
    pad: [u64; 7],
}

#[allow(clippy::upper_case_acronyms)]
#[bitflags]
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StatKind {
    DIR = 0o040000,
    #[default]
    FILE = 0o100000,
}

impl Inode {
    #[inline]
    pub fn new(
        block_id: u32,
        block_offset: usize,
        fs: Arc<Mutex<EasyFileSystem>>,
        block_device: Arc<dyn BlockDevice>,
    ) -> Self {
        Self {
            block_id: block_id as usize,
            block_offset,
            fs,
            block_device,
        }
    }

    /// 在当前 inode 下创建子 inode
    pub fn create(&self, name: &str) -> Option<Arc<Self>> {
        let mut fs = self.fs.lock();

        let inode_id = self.on_disk(|root_inode: &DiskInode| self.get(root_inode, name));
        // 确认没有已创建的同名项
        if inode_id.is_some() {
            return None;
        }

        // 创建新文件
        let new_inode_id = fs.alloc_inode();
        let (new_inode_block_id, new_inode_block_offset) = fs.disk_inode_pos(new_inode_id);
        block_cache::get(new_inode_block_id as usize, self.block_device.clone())
            .lock()
            .map_mut(new_inode_block_offset, |new_inode: &mut DiskInode| {
                new_inode.init(new_inode_id, DiskInodeKind::File)
            });

        self.on_disk_mut(|root_inode| {
            let slot = self.find_or_new_slot(root_inode, &mut fs);
            let dir_entry = DirEntry::new(name, new_inode_id);
            root_inode.write_at(slot, dir_entry.as_bytes(), &self.block_device);
        });

        block_cache::sync_all();

        Some(Arc::new(Self::new(
            new_inode_block_id,
            new_inode_block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        )))
    }

    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let _fs = self.fs.lock();
        self.on_disk(|disk_inode| disk_inode.read_at(offset, buf, &self.block_device))
    }

    pub fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        let mut fs = self.fs.lock();
        let size = self.on_disk_mut(|disk_inode| {
            self.expand_to((offset + buf.len()) as u32, disk_inode, &mut fs);
            disk_inode.write_at(offset, buf, &self.block_device)
        });
        block_cache::sync_all();
        size
    }

    pub fn clear(&self) {
        let mut fs = self.fs.lock();
        self.internal_clear(&mut fs);
        block_cache::sync_all();
    }

    /// 根据文件名获取 inode
    pub fn find(&self, name: &str) -> Option<Arc<Inode>> {
        let fs = self.fs.lock();
        self.on_disk(|disk_inode| {
            self.get(disk_inode, name)
                .map(|inode_id| Arc::new(self.inode(&fs, inode_id)))
        })
    }

    pub fn link_at(&self, name: &str, new_path: &str) -> Option<()> {
        let mut fs = self.fs.lock();

        let inode_id = self.on_disk(|root_inode: &DiskInode| {
            assert!(root_inode.is_dir());
            self.get(root_inode, name)
        })?;
        self.inode(&fs, inode_id).on_disk_mut(|disk_inode| {
            disk_inode.links += 1;
        });

        self.on_disk_mut(|root_inode| {
            let slot = self.find_or_new_slot(root_inode, &mut fs);
            let dir_entry = DirEntry::new(new_path, inode_id);
            root_inode.write_at(slot, dir_entry.as_bytes(), &self.block_device);
        });

        block_cache::sync_all();
        Some(())
    }

    pub fn unlink_at(&self, name: &str) -> Option<()> {
        let mut fs = self.fs.lock();

        let inode_id = self.on_disk_mut(|root_inode| {
            assert!(root_inode.is_dir());
            self.remove(root_inode, name)
        })?;
        let inode = self.inode(&fs, inode_id);

        let links = inode.on_disk_mut(|disk_inode| {
            disk_inode.links -= 1;
            disk_inode.links
        });
        if links == 0 {
            inode.internal_clear(&mut fs);
        }

        block_cache::sync_all();
        Some(())
    }

    pub fn stat(&self) -> Stat {
        let _fs = self.fs.lock();
        self.on_disk(|disk_inode| {
            Stat::new(
                disk_inode.id as u64,
                disk_inode.kind.into(),
                disk_inode.links,
            )
        })
    }
}

impl Inode {
    /// 读取对磁盘的映射并处理
    fn on_disk<V>(&self, f: impl FnOnce(&DiskInode) -> V) -> V {
        block_cache::get(self.block_id, self.block_device.clone())
            .lock()
            .map(self.block_offset, f)
    }

    /// 以某种方式修改对磁盘的映射
    fn on_disk_mut<V>(&self, f: impl FnOnce(&mut DiskInode) -> V) -> V {
        block_cache::get(self.block_id, self.block_device.clone())
            .lock()
            .map_mut(self.block_offset, f)
    }

    /// 在 DiskInode 下通过名字获取目录项的inode ID
    fn get(&self, disk_inode: &DiskInode, name: &str) -> Option<u32> {
        assert!(disk_inode.is_dir());
        // 目录下存放的是文件系统项元信息
        let size = disk_inode.size as usize;
        let mut dir_entry = DirEntry::default();

        for offset in (0..size).step_by(DirEntry::SIZE) {
            assert_eq!(
                disk_inode.read_at(offset, dir_entry.as_bytes_mut(), &self.block_device),
                DirEntry::SIZE
            );
            if dir_entry.name() == name {
                return Some(dir_entry.inode_id());
            }
        }

        None
    }

    /// 在 DiskInode 下通过名字删除目录项并返回其inode ID
    fn remove(&self, disk_inode: &mut DiskInode, name: &str) -> Option<u32> {
        assert!(disk_inode.is_dir());
        let size = disk_inode.size as usize;
        let mut dir_entry = DirEntry::default();

        for offset in (0..size).step_by(DirEntry::SIZE) {
            assert_eq!(
                disk_inode.read_at(offset, dir_entry.as_bytes_mut(), &self.block_device),
                DirEntry::SIZE
            );
            if dir_entry.name() == name {
                disk_inode.write_at(offset, &[0; DirEntry::SIZE], &self.block_device);
                return Some(dir_entry.inode_id());
            }
        }

        None
    }

    /// 在当前目录的数据当中，寻找空槽位；找不到就分配新槽位
    fn find_or_new_slot(&self, disk_inode: &mut DiskInode, fs: &mut EasyFileSystem) -> usize {
        assert!(disk_inode.is_dir());
        let size = disk_inode.size as usize;
        let mut dir_entry = DirEntry::default();

        for offset in (0..size).step_by(DirEntry::SIZE) {
            assert_eq!(
                disk_inode.read_at(offset, dir_entry.as_bytes_mut(), &self.block_device),
                DirEntry::SIZE
            );
            if dir_entry.name().is_empty() {
                return offset;
            }
        }

        self.expand_to((size + DirEntry::SIZE) as u32, disk_inode, fs);
        size
    }

    /// 凭借ID获取Inode
    #[inline]
    fn inode(&self, fs: &EasyFileSystem, id: u32) -> Inode {
        let (block_id, block_offset) = fs.disk_inode_pos(id);
        Self::new(
            block_id,
            block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        )
    }

    fn expand_to(&self, larger_size: u32, disk_inode: &mut DiskInode, fs: &mut EasyFileSystem) {
        assert!(larger_size > disk_inode.size);

        let new_blocks = DiskInode::count_total_block(larger_size)
            - DiskInode::count_total_block(disk_inode.size);
        let new_blocks: Vec<u32> = (0..new_blocks).map(|_| fs.alloc_data()).collect();

        // 传进去的是一批未初始化块的ID
        disk_inode.expand_to(larger_size, new_blocks, &self.block_device);
    }

    fn internal_clear(&self, fs: &mut EasyFileSystem) {
        self.on_disk_mut(|disk_inode| {
            let data_blocks = disk_inode.clear(&self.block_device);
            assert_eq!(
                data_blocks.len(),
                DiskInode::count_total_block(disk_inode.size)
            );
            for data_block in data_blocks {
                fs.dealloc_data(data_block);
            }
        });
    }
}

impl Stat {
    #[inline]
    pub fn new(inode: u64, kind: StatKind, links: u32) -> Self {
        Self {
            dev: 0,
            inode,
            kind,
            links,
            pad: Default::default(),
        }
    }
}

impl From<DiskInodeKind> for StatKind {
    #[inline]
    fn from(kind: DiskInodeKind) -> Self {
        match kind {
            DiskInodeKind::Directory => Self::DIR,
            DiskInodeKind::File => Self::FILE,
        }
    }
}
