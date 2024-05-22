use core::mem;

use alloc::vec::Vec;
use enumflags2::BitFlags;

use crate::volume::data::{
    dir_entry_name, AttrFlag, DirEntry, DirEntryStatus, LongDirEntry, ShortDirEntry,
};
use crate::volume::reserved::bpb;
use crate::{sector, ClusterId, FatFileSystem, SectorId};

/// 目录项会指向一个簇链表，这就是FAT文件系统中的inode。
///
/// 理论上每个[`Inode`]是唯一的、目录项无关的，但为了实用，
/// 我们不得不将其与目录项的位置与属性关联起来。
#[derive(Debug, Clone)]
pub struct Inode {
    start_id: ClusterId<u32>,
    dirent_pos: DirEntryPos,
    kind: InodeKind,
}

impl Inode {
    /// 目录
    pub fn find(&self, relat_path: &str, sb: &FatFileSystem) -> Option<Self> {
        let mut cmps = relat_path.split('/');
        let mut inode = self.clone();
        let basename = cmps.next_back()?;
        for cmp in cmps {
            let cmp_inode = inode.find_pwd(cmp, sb)?;
            if cmp_inode.kind != InodeKind::Directory {
                return None;
            }
            inode = cmp_inode;
        }
        inode.find_pwd(basename, sb)
    }

    /// 文件
    pub fn read_at(&self, offset: usize, buf: &mut [u8], sb: &FatFileSystem) -> usize {
        let file_size = self.dirent_pos.access(ShortDirEntry::file_size);
        let sector_size = bpb().sector_bytes();

        let start = offset;
        let end = (start + buf.len()).min(file_size); // exclusive

        if start > end {
            return 0;
        }

        let mut read_size = 0;

        let n_skip = start / sector_size;
        let n_take = end.div_ceil(sector_size);
        for sid in sb.data_sectors(self.start_id).take(n_take).skip(n_skip) {
            let block_read_size = (end - read_size).min(sector_size);
            sector::get(sid).lock().map_slice(|data: &[u8]| {
                buf[read_size..read_size + block_read_size]
                    .copy_from_slice(&data[..block_read_size])
            });
            read_size += block_read_size;
        }

        read_size
    }

    /// 文件
    pub fn write_at(&self, offset: usize, buf: &[u8], sb: &mut FatFileSystem) -> usize {
        let file_size = self.dirent_pos.access(ShortDirEntry::file_size);
        let sector_size = bpb().sector_bytes();

        let start = offset;
        let end = start + buf.len(); // exclusive

        if end > file_size {
            self.expand_to(file_size, end, sb);
        }

        let mut wrote_size = 0;

        let n_skip = start / sector_size;
        let n_take = end.div_ceil(sector_size);
        for sid in sb.data_sectors(self.start_id).take(n_take).skip(n_skip) {
            let block_write_size = (end - wrote_size).min(sector_size);
            sector::get(sid).lock().map_mut_slice(|data: &mut [u8]| {
                data[..block_write_size]
                    .copy_from_slice(&buf[wrote_size..wrote_size + block_write_size])
            });
            wrote_size += block_write_size;
        }

        if end > file_size {
            self.dirent_pos
                .access_mut(|dirent| dirent.set_file_size(end));
        }

        wrote_size
    }
}

impl Inode {
    fn find_pwd(&self, name: &str, sb: &FatFileSystem) -> Option<Self> {
        let checksum = ShortDirEntry::checksum_from(name.as_bytes());
        let sectors = sb.data_sectors(self.start_id);

        let mut prev_sector = None;
        for sid in sectors {
            let dirent = sector::get(sid);
            let dirent = dirent.lock();
            let dirents: &[DirEntry] = dirent.as_slice();

            for (i, dirent) in dirents
                .iter()
                .take_while(|dirent| unsafe { dirent.short.status() != DirEntryStatus::FreeHead })
                .enumerate()
            {
                if unsafe {
                    dirent.short.status() == DirEntryStatus::Occupied
                        && dirent.attr() != LongDirEntry::attr()
                        && dirent.short.checksum() == checksum
                } {
                    let mut longs = Vec::with_capacity(10);

                    let mut discrete = true;

                    for dirent in dirents[..i].iter().rev().take_while(|dirent| unsafe {
                        dirent.attr() == LongDirEntry::attr() && dirent.long.chksum == checksum
                    }) {
                        longs.push(unsafe { LongDirEntry::clone(&dirent.long) });
                        if unsafe {
                            dirent.long.ord & LongDirEntry::LAST_MASK == LongDirEntry::LAST_MASK
                        } {
                            discrete = false;
                            break;
                        }
                    }

                    if discrete {
                        let prev = prev_sector.unwrap();
                        sector::get(prev).lock().map_slice(|dirents: &[DirEntry]| {
                            let end = dirents
                                .iter()
                                .rposition(|dirent| unsafe {
                                    dirent.attr() == LongDirEntry::attr()
                                        && dirent.long.chksum == checksum
                                        && (dirent.long.ord & LongDirEntry::LAST_MASK
                                            == LongDirEntry::LAST_MASK)
                                })
                                .expect("The last long entry was lost");
                            longs.extend(
                                dirents[end..]
                                    .iter()
                                    .rev()
                                    .map(|dirent| unsafe { LongDirEntry::clone(&dirent.long) }),
                            );
                        });
                    }

                    let dname = dir_entry_name(&longs);
                    if name == dname {
                        let pos = DirEntryPos::new(sid, i);
                        let dirent: &ShortDirEntry = unsafe { &dirent.short };
                        return Some((pos, dirent).into());
                    }
                }
            }

            prev_sector = Some(sid);
        }

        None
    }

    /// 块
    fn expand_to(&self, old_size: usize, larger_size: usize, sb: &mut FatFileSystem) {
        let sector_bytes = bpb().sector_bytes();

        let old_sectors = old_size.div_ceil(sector_bytes);
        let new_sectors = larger_size.div_ceil(sector_bytes);
        let added_sectors = new_sectors - old_sectors;

        let mut current = sb.fat().last(self.start_id).unwrap();
        for _ in 0..added_sectors {
            let next = sb.alloc_cluster().0;
            unsafe {
                sb.fat_mut().couple(current, next);
            }
            current = next;
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct DirEntryPos {
    sector: SectorId,
    nth: usize,
}

impl DirEntryPos {
    pub const fn new(sector: SectorId, nth: usize) -> Self {
        Self { sector, nth }
    }

    pub fn access<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&ShortDirEntry) -> R,
    {
        sector::get(self.sector)
            .lock()
            .map(self.nth * mem::size_of::<ShortDirEntry>(), f)
    }

    pub fn access_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut ShortDirEntry) -> R,
    {
        sector::get(self.sector)
            .lock()
            .map_mut(self.nth * mem::size_of::<ShortDirEntry>(), f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InodeKind {
    File,
    Directory,
}

impl From<BitFlags<AttrFlag>> for InodeKind {
    fn from(attr: BitFlags<AttrFlag>) -> Self {
        if attr.contains(AttrFlag::Directory) {
            Self::Directory
        } else {
            Self::File
        }
    }
}

impl From<(DirEntryPos, &ShortDirEntry)> for Inode {
    fn from((pos, dirent): (DirEntryPos, &ShortDirEntry)) -> Self {
        Self {
            start_id: dirent.cluster_id(),
            dirent_pos: pos,
            kind: dirent.attr().into(),
        }
    }
}
