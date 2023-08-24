use alloc::sync::Arc;

use alloc::vec::Vec;
use easy_fs::EasyFileSystem;
use easy_fs::Inode;
use easy_fs::Stat;
use enumflags2::bitflags;
use enumflags2::BitFlags;
use lazy_static::lazy_static;

use super::File;
use crate::drivers::BLOCK_DEVICE;
use crate::memory::Buffer;
use crate::sync::UPSafeCell;

lazy_static! {
    static ref ROOT_INODE: Arc<Inode> = {
        let efs = EasyFileSystem::open(BLOCK_DEVICE.clone());
        Arc::new(EasyFileSystem::root_inode(&efs))
    };
}

/// 表示进程打开的文件或目录
pub struct OSInode {
    readable: bool,
    writable: bool,
    inner: UPSafeCell<OSInodeInner>,
}

pub fn list_apps() {
    println!("/**** APPS ****/");
    for app in ROOT_INODE.ls() {
        println!("{}", app);
    }
    println!("/**************/");
}

pub fn open_file(name: &str, flags: BitFlags<OpenFlag>) -> Option<Arc<OSInode>> {
    let [readable, writable] = if flags.is_empty() {
        [true, false]
    } else if flags.contains(OpenFlag::WRONLY) {
        [false, true]
    } else {
        [true, true]
    };
    let create = flags.contains(OpenFlag::CREATE);

    ROOT_INODE
        .find(name)
        .map(|inode| {
            if create || flags.contains(OpenFlag::TRUNC) {
                inode.clear();
            }
            Arc::new(OSInode::new(readable, writable, inode))
        })
        .or_else(|| {
            create
                .then(|| {
                    ROOT_INODE
                        .create(name)
                        .map(|inode| Arc::new(OSInode::new(readable, writable, inode)))
                })
                .flatten()
        })
}

#[inline]
pub fn link_at(old_path: &str, new_path: &str) -> Option<()> {
    ROOT_INODE.link_at(old_path, new_path)
}

#[inline]
pub fn unlink_at(path: &str) -> Option<()> {
    ROOT_INODE.unlink_at(path)
}


struct OSInodeInner {
    /// **文件**内的偏移量
    offset: usize,
    inode: Arc<Inode>,
}

#[rustfmt::skip]
#[allow(clippy::upper_case_acronyms)]
#[bitflags]
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenFlag {
    /// 只写
    WRONLY = 0b0000_0000_0001,
    /// 读写兼备
    RDWR   = 0b0000_0000_0010,
    /// 创建文件，若文件存在则清空
    CREATE = 0b0010_0000_0000,
    /// 先清空文件，再交给用户
    TRUNC  = 0b0100_0000_0000,
}

impl OpenFlag {
    // enumflags2拒绝值为0的标志
    /// 只读
    pub const RDONLY: u32 = 0b0000_0000_0000;
}

impl OSInode {
    #[inline]
    pub fn new(readable: bool, writable: bool, inode: Arc<Inode>) -> Self {
        Self {
            readable,
            writable,
            inner: unsafe { UPSafeCell::new(OSInodeInner { offset: 0, inode }) },
        }
    }

    pub fn read_all(&self) -> Vec<u8> {
        let mut inner = self.inner.exclusive_access();
        let mut buffer = [0u8; 512];

        let mut bytes = Vec::new();
        loop {
            let len = inner.inode.read_at(inner.offset, &mut buffer);
            if len == 0 {
                break;
            }
            inner.offset += len;
            bytes.extend_from_slice(&buffer[..len]);
        }
        bytes
    }
}

impl File for OSInode {
    #[inline]
    fn readable(&self) -> bool {
        self.readable
    }

    #[inline]
    fn writable(&self) -> bool {
        self.writable
    }

    fn read(&self, mut buf: Buffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_read_size = 0;

        for sub_buf in buf.as_mut() {
            let read_size = inner.inode.read_at(inner.offset, sub_buf);
            if read_size == 0 {
                break;
            }
            inner.offset += read_size;
            total_read_size += read_size;
        }

        total_read_size
    }

    fn write(&self, buf: Buffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_write_size = 0;

        for sub_buf in buf.as_ref() {
            let write_size = inner.inode.write_at(inner.offset, sub_buf);
            assert_eq!(write_size, sub_buf.len());
            inner.offset += write_size;
            total_write_size += write_size;
        }

        total_write_size
    }

    fn stat(&self) -> Stat {
        let inner = self.inner.exclusive_access();
        inner.inode.stat()
    }
}
