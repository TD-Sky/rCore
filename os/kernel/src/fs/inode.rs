use alloc::slice;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::mem;
use core::ptr;

use enumflags2::BitFlags;
use enumflags2::bitflags;
use fat::FatFileSystem;
use fat::Inode;
use fat::ROOT;
use spin::Lazy;
use vfs::CDirEntry;
use vfs::DirEntryType;
use vfs::Stat;

use super::File;
use crate::drivers::BLOCK_DEVICE;
use crate::memory::UserBuffer;
use crate::path::Path;
use crate::sync::UpCell;

static FS: Lazy<UpCell<FatFileSystem>> =
    Lazy::new(|| UpCell::new(FatFileSystem::load(&BLOCK_DEVICE)));

/// 表示进程打开的文件或目录
#[derive(Debug)]
pub struct OSInode {
    readable: bool,
    writable: bool,
    inner: UpCell<OSInodeInner>,
}

#[derive(Debug)]
struct OSInodeInner {
    /// **文件**内的偏移量
    offset: usize,
    inode: Inode,
}

impl OSInode {
    #[inline]
    pub fn new(readable: bool, writable: bool, inode: Inode) -> Self {
        Self {
            readable,
            writable,
            inner: UpCell::new(OSInodeInner { offset: 0, inode }),
        }
    }

    pub fn read_all(&self) -> Vec<u8> {
        let mut inner = self.inner.exclusive_access();
        let mut buffer = [0u8; 512];

        let mut bytes = Vec::new();
        loop {
            let len = inner
                .inode
                .read_at(inner.offset, &mut buffer, &FS.exclusive_access());
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

    fn read(&self, mut buf: UserBuffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_read_size = 0;

        for sub_buf in buf.as_mut() {
            let read_size = inner
                .inode
                .read_at(inner.offset, sub_buf, &FS.exclusive_access());
            if read_size == 0 {
                break;
            }
            inner.offset += read_size;
            total_read_size += read_size;
        }

        total_read_size
    }

    fn write(&self, buf: UserBuffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_write_size = 0;
        let offset = inner.offset;

        for sub_buf in buf.as_ref() {
            let write_size = inner
                .inode
                .write_at(offset, sub_buf, &mut FS.exclusive_access());
            assert_eq!(write_size, sub_buf.len());
            inner.offset += write_size;
            total_write_size += write_size;
        }

        total_write_size
    }

    fn stat(&self) -> Stat {
        self.inner
            .exclusive_access()
            .inode
            .stat(&FS.exclusive_access())
    }

    fn getdents(&self, mut buf: UserBuffer, len: usize) -> usize {
        let mut inner = self.inner.exclusive_access();
        let dirents = inner.inode.ls_at(inner.offset, len, &FS.exclusive_access());
        let read = dirents.len();
        log::debug!("Read DirEntries: {read}");

        let name_ptrs: Vec<_> = buf
            .transmute_slice::<CDirEntry>()
            .into_iter()
            .take(read)
            .map(|c_dirent| c_dirent.name)
            .collect();

        for (&name_ptr, dirent) in name_ptrs.iter().zip(&dirents) {
            let mut name_buf = UserBuffer::new(buf.token(), name_ptr, CDirEntry::NAME_CAP);
            for (cnb, &dnb) in name_buf.iter_mut().zip(dirent.name.as_bytes()) {
                *cnb = dnb;
            }
        }

        let dirents: Vec<_> = dirents
            .iter()
            .zip(name_ptrs)
            .map(|(dirent, name)| CDirEntry {
                inode: dirent.inode,
                ty: dirent.ty,
                name,
            })
            .collect();

        for (b, &db) in buf.iter_mut().zip(dirents.iter().flat_map(|dirent| unsafe {
            slice::from_raw_parts(
                ptr::from_ref(&dirent).cast::<u8>(),
                mem::size_of::<CDirEntry>(),
            )
        })) {
            *b = db;
        }

        inner.offset += read;
        read
    }

    fn mkdir(&self, name: &str) -> Result<(), vfs::Error> {
        let inner = self.inner.exclusive_access();
        inner.inode.mkdir(name, &mut FS.exclusive_access())?;
        Ok(())
    }

    fn unlink(&self, name: &str) -> Result<(), vfs::Error> {
        let mut inner = self.inner.exclusive_access();
        inner.inode.unlink(name, &mut FS.exclusive_access())
    }

    fn rmdir(&self, name: &str) -> Result<(), vfs::Error> {
        let mut inner = self.inner.exclusive_access();
        inner.inode.rmdir(name, &mut FS.exclusive_access())
    }

    fn rename(&self, old_name: &str, newpath: &str) -> Result<(), vfs::Error> {
        let mut inner = self.inner.exclusive_access();

        let (mut new_parent, new_name) = match open_dir_inode(newpath) {
            Ok(p) => {
                log::info!("{old_name} -> {newpath}/");
                (p, old_name)
            }
            Err(vfs::Error::NotADirectory | vfs::Error::NotFound) => {
                let (parent, file) = newpath.parent_file().expect("path was verified as not `/`");
                log::info!(
                    "{old_name} -> {}/{file}",
                    parent.root_relative().unwrap_or_default()
                );
                (open_dir_inode(parent)?, file)
            }
            Err(e) => return Err(e),
        };

        if inner.inode.id() == new_parent.id() {
            // 当前目录
            log::info!("rename currently");
            if old_name == new_name {
                return Err(vfs::Error::AlreadyExists);
            } else {
                inner
                    .inode
                    .rename(old_name, None, new_name, &mut FS.exclusive_access())?;
            }
        } else {
            // 跨目录
            log::info!("rename cross directories");
            inner.inode.rename(
                old_name,
                Some(&mut new_parent),
                new_name,
                &mut FS.exclusive_access(),
            )?;
        }

        Ok(())
    }
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

    #[inline]
    pub fn read_only() -> BitFlags<OpenFlag> {
        BitFlags::from_bits_truncate(Self::RDONLY)
    }
}

/// `path`为标准路径
pub fn open_dir(path: &str) -> Result<Arc<OSInode>, vfs::Error> {
    open_dir_inode(path).map(|inode| Arc::new(OSInode::new(true, true, inode)))
}

fn open_dir_inode(path: &str) -> Result<Inode, vfs::Error> {
    if path == "/" {
        Ok(ROOT.clone())
    } else {
        let inode = ROOT
            .find(path.root_relative().unwrap(), &FS.exclusive_access())
            .ok_or(vfs::Error::NotFound)?;
        if inode.kind() != DirEntryType::Directory {
            return Err(vfs::Error::NotADirectory);
        }
        Ok(inode)
    }
}

pub fn open(path: &str, flags: BitFlags<OpenFlag>) -> Option<Arc<OSInode>> {
    let [readable, writable] = if flags.is_empty() {
        [true, false]
    } else if flags.contains(OpenFlag::WRONLY) {
        [false, true]
    } else {
        [true, true]
    };
    let create = flags.contains(OpenFlag::CREATE);

    let mut fs = FS.exclusive_access();
    let Some(relat_path) = path.root_relative() else {
        return Some(Arc::new(OSInode::new(readable, writable, ROOT.clone())));
    };

    ROOT.find(relat_path, &fs)
        .map(|mut inode| {
            if create || flags.contains(OpenFlag::TRUNC) {
                inode.clear(&mut fs);
            }
            Arc::new(OSInode::new(readable, writable, inode))
        })
        .or_else(|| {
            create
                .then(|| {
                    if let Some((parent, fname)) = relat_path.rsplit_once('/') {
                        let parent = ROOT.find(parent, &fs)?;
                        parent.create_file(fname, &mut fs)
                    } else {
                        ROOT.create_file(relat_path, &mut fs)
                    }
                    .ok()
                    .map(|inode| Arc::new(OSInode::new(readable, writable, inode)))
                })
                .flatten()
        })
}

#[allow(unused_variables)]
#[inline]
pub fn link(old_path: &str, new_path: &str) -> Option<()> {
    None
}

// /// # 参数
// ///
// /// `old_path`和`new_path`都是标准路径。
// pub fn rename(old_path: &str, new_path: &str) -> Option<()> {
//     // let fs = FS.exclusive_access();
//     //
//     // if new_path.starts_with(old_path) {
//     //     // 倒反天罡: /foo/bar -> /foo/bar/zoo
//     //     return None;
//     // }
//     //
//     // let old_parent = old_path.parent()?;
//     // let new_parent = new_path.parent();
//     //
//     // if old_parent == new_path {
//     //     // 原地命名，跳过
//     //     return None;
//     // }
//     //
//     // if Some(old_parent) == new_parent {
//     //     // 同一目录下的重命名
//     //     let old_name = old_path
//     //         .file_name()
//     //         .expect("it has parent, should has file name");
//     //     let new_name = new_path.file_name();
//     //     let old_parent = ROOT.find(old_parent.root_relative()?, &fs)?;
//     // }
//     //
//     // let old_parent = ROOT.find(old_parent.root_relative()?, &fs)?;
//     // let new_parent = match new_path.parent().and_then(|p| p.root_relative()) {
//     //     Some(parent) => &ROOT.find(parent, &fs)?,
//     //     None => &ROOT,
//     // };
//
//     todo!()
// }
