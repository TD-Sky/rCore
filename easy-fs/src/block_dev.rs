//! # 块设备接口层
//!
//! 块设备是以**块**为单位存储数据的设备，例如磁盘、光盘、U盘等；
//! [`BlockDevice`] 就是对读写块设备的抽象，
//! 实现了此特质的类型称为**块设备驱动**。
//!
//! `easy-fs` 可以通过块设备驱动读写块设备。

use core::any::Any;

/// 块设备驱动特质
pub trait BlockDevice: Send + Sync + Any {
    fn read_block(&self, block_id: usize, buf: &mut [u8]);
    fn write_block(&self, block_id: usize, buf: &[u8]);
}
