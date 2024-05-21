use alloc::vec::Vec;
use enumflags2::BitFlags;

use crate::volume::data::{
    dir_entry_name, AttrFlag, DirEntry, DirEntryStatus, LongDirEntry, ShortDirEntry,
};
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
}

#[derive(Debug, Clone, Copy)]
pub struct DirEntryPos {
    sector: SectorId,
    nth: usize,
}

impl DirEntryPos {
    pub const fn new(sector: SectorId, nth: usize) -> Self {
        Self { sector, nth }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InodeKind {
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
