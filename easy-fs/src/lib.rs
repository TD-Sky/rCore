#![no_std]
#![feature(int_roundings)]

extern crate alloc;

/* easyfs 的整体架构，自上而下 */

// 索引节点层：实现文件创建、打开、读写等操作
mod vfs;
pub use vfs::Inode;
pub use vfs::{Stat, StatKind};

// 磁盘块管理器层
mod efs;
pub use efs::EasyFileSystem;

// 磁盘数据结构层：表示磁盘文件系统的数据结构
mod layout;

// 块缓存层：内存上的磁盘块数据缓存
mod block_cache;

// 磁盘块设备接口层：读写磁盘块设备的接口
mod block_dev;
pub use block_dev::BlockDevice;

pub const MAGIC: u32 = 0x3b800001;
pub const BLOCK_SIZE: usize = 512;
pub const BLOCK_BITS: usize = BLOCK_SIZE * 8;

type DataBlock = [u8; BLOCK_SIZE];
