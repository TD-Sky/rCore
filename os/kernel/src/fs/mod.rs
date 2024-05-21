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

mod directory;
pub mod eventfd;
mod inode;
mod pipe;
pub mod stdio;

use core::fmt::Debug;

use easy_fs::Stat;

pub use self::{directory::Directory, inode::*, pipe::*};
use crate::memory::UserBuffer;

/// 内存与存储设备之间的数据交换通道
pub trait File: Debug + Send + Sync {
    fn readable(&self) -> bool;
    fn writable(&self) -> bool;
    fn read(&self, buf: UserBuffer) -> usize;
    fn write(&self, buf: UserBuffer) -> usize;
    fn stat(&self) -> Stat;
}
