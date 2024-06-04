//! # 内核文件系统
//!
//! ## 分层（自上而下）
//!
//! 1. 系统调用层
//! 2. 文件描述符层
//! 3. 内核索引节点层
//! 4. 文件系统层
//! 5. 块设备驱动层
//!
//! ## 文件描述符层
//!
//! 一个进程可以访问多个文件，并通过**文件描述符表**管理。
//! 表中的描述符表示带有特定读写属性的I/O资源(文件/目录/socket等)。

pub mod eventfd;
mod inode;
mod pipe;
pub mod stdio;

use core::fmt::Debug;

use vfs::{DirEntryType, Stat};

pub use self::{inode::*, pipe::*};
use crate::memory::UserBuffer;

/// 内存与存储设备之间的数据交换通道
pub trait File: Debug + Send + Sync {
    fn readable(&self) -> bool {
        false
    }

    fn writable(&self) -> bool {
        false
    }

    #[allow(unused_variables)]
    fn read(&self, buf: UserBuffer) -> usize {
        0
    }

    #[allow(unused_variables)]
    fn write(&self, buf: UserBuffer) -> usize {
        0
    }

    fn stat(&self) -> Stat {
        Stat {
            mode: DirEntryType::Regular,
            block_size: 0,
            blocks: 0,
            size: 0,
        }
    }

    #[allow(unused_variables)]
    fn getdents(&self, buf: UserBuffer, len: usize) -> usize {
        0
    }

    #[allow(unused_variables)]
    fn mkdir(&self, name: &str) -> Result<(), vfs::Error> {
        Err(vfs::Error::Unsupported)
    }

    #[allow(unused_variables)]
    fn unlink(&self, name: &str) -> Result<(), vfs::Error> {
        Err(vfs::Error::Unsupported)
    }

    #[allow(unused_variables)]
    fn rmdir(&self, name: &str) -> Result<(), vfs::Error> {
        Err(vfs::Error::Unsupported)
    }
}
