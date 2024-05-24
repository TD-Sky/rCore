use alloc::vec::Vec;
use core::mem;

use enumflags2::BitFlags;

use crate::volume::data::{
    dirents2name, name2dirents, sector_dirents, AttrFlag, DirEntry, DirEntryStatus, LongDirEntry,
    ShortDirEntry,
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
    pub const ROOT: Self = Self {
        start_id: ClusterId::MIN,
        dirent_pos: DirEntryPos::ROOT,
        kind: InodeKind::Directory,
    };

    /// 目录
    ///
    /// # 参数
    ///
    /// `relat_path`: 相对于[`Inode`]的相对路径。
    pub fn find(&self, relat_path: &str, sb: &FatFileSystem) -> Option<Self> {
        let mut cmps = relat_path.split('/');
        let mut inode = self.clone();
        let basename = cmps.next_back()?;
        for cmp in cmps {
            let cmp_inode = inode.find_cwd(cmp, sb)?;
            if cmp_inode.kind != InodeKind::Directory {
                return None;
            }
            inode = cmp_inode;
        }
        inode.find_cwd(basename, sb)
    }

    /// 文件
    pub fn read_at(&self, offset: usize, buf: &mut [u8], sb: &FatFileSystem) -> usize {
        let file_size = self.dirent_pos.access(ShortDirEntry::file_size);
        let sector_size = bpb().sector_bytes();

        let start = offset;
        let end = (start + buf.len()).min(file_size); // exclusive

        if start >= end {
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

    /// 目录
    ///
    /// 在当前目录下创建文件。
    pub fn touch(&self, name: &str, sb: &mut FatFileSystem) -> Option<Self> {
        let (start_id, dirent_pos) = self.create(name, sb, |_, _, _| ClusterId::EOF)?;
        sector::sync_all();
        Some(Self {
            start_id,
            dirent_pos,
            kind: InodeKind::File,
        })
    }

    /// 文件
    ///
    /// 随机写入，对于空文件会分配有效的起始簇编号再写入。
    pub fn write_at(&mut self, offset: usize, buf: &[u8], sb: &mut FatFileSystem) -> usize {
        let file_size = self.dirent_pos.access(ShortDirEntry::file_size);
        let sector_size = bpb().sector_bytes();

        let start = offset;
        let end = start + buf.len(); // exclusive

        // Expand
        if end > file_size {
            let added_sectors = (end - file_size).div_ceil(sector_size);
            debug_assert!(added_sectors > 0);

            let mut added_clusters = added_sectors.div_ceil(bpb().cluster_sectors());

            let mut current = if self.start_id == ClusterId::EOF {
                added_clusters -= 1;
                self.start_id = sb.alloc_cluster().0;
                self.dirent_pos
                    .access_mut(|dirent| dirent.set_cluster_id(self.start_id));
                self.start_id
            } else {
                sb.fat().last(self.start_id).unwrap()
            };

            for _ in 0..added_clusters {
                let next = sb.alloc_cluster().0;
                unsafe {
                    sb.fat_mut().couple(current, next);
                }
                current = next;
            }
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

    /// 目录
    ///
    /// 在当前目录下创建目录。
    pub fn mkdir(&self, name: &str, sb: &mut FatFileSystem) -> Option<Self> {
        let (start_id, dirent_pos) = self.create(name, sb, Self::alloc_dir)?;
        sector::sync_all();
        Some(Self {
            start_id,
            dirent_pos,
            kind: InodeKind::Directory,
        })
    }
}

impl Inode {
    /// 目录
    ///
    /// 搜索当前目录下指定名称的项。
    fn find_cwd(&self, name: &str, sb: &FatFileSystem) -> Option<Self> {
        if name == "." {
            return Some(self.clone());
        }

        let checksum = ShortDirEntry::checksum_from(name.as_bytes());
        let mut sectors = sb.data_sectors(self.start_id);

        if name == ".." {
            // NOTE: 把相对目录项的位置交出去不太好，但也没别的办法
            let dirent_pos = DirEntryPos::new(sectors.next().unwrap(), 1);
            let start_id = dirent_pos.access(ShortDirEntry::cluster_id);
            return Some(Self {
                start_id,
                dirent_pos,
                kind: InodeKind::Directory,
            });
        }

        let mut prev_sector = None;
        for sid in sectors {
            let dirents = sector::get(sid);
            let dirents = dirents.lock();
            let dirents: &[DirEntry] = dirents.as_slice();

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

                    let dname = dirents2name(&longs);
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

    /// 目录
    ///
    /// 在当前目录下创建目录项。
    fn create(
        &self,
        name: &str,
        sb: &mut FatFileSystem,
        gen_cid: fn(&Self, &mut ShortDirEntry, &mut FatFileSystem) -> ClusterId<u32>,
    ) -> Option<(ClusterId<u32>, DirEntryPos)> {
        if self.find_cwd(name, sb).is_some() {
            // TODO: 返回“文件已存在”的错误
            return None;
        }

        let sector_dirents = sector_dirents();

        let (mut short, longs) = name2dirents(name);
        let cluster_id = gen_cid(self, &mut short, sb);

        let n_long = longs.len();

        let mut sectors = sb.data_sectors(self.start_id);
        let mut prev_sector = None;

        /* 尝试收集足够的连续中间槽 */
        let mut slot_count = 0;
        let mut discrete = false;
        let pos = 'out: loop {
            if let Some(sid) = sectors.next() {
                let dirents = sector::get(sid);
                let dirents = dirents.lock();
                let dirents: &[DirEntry] = dirents.as_slice();

                for (i, dirent) in dirents.iter().enumerate() {
                    match unsafe { dirent.short.status() } {
                        DirEntryStatus::Free => {
                            slot_count += 1;

                            if slot_count == n_long + 1 {
                                break 'out Some(DirEntryPos::new(sid, i));
                            }
                        }
                        DirEntryStatus::FreeHead => {
                            slot_count = 0;
                            break 'out Some(DirEntryPos::new(sid, i));
                        }
                        DirEntryStatus::Occupied => {
                            slot_count = 0;
                        }
                    }
                }

                // NOTE: 有计数，但没中止，说明要去下一个扇区
                //       继续收集中间槽
                discrete = slot_count > 0;

                prev_sector = Some(sid);
            } else {
                break 'out None;
            }
        };
        if slot_count > 0 {
            // NOTE: 上面的循环绝不会以`slot_count == 0`或
            //       `slot_count == need_slots`之外的状态退出，
            //       而且满足后者时，必定存在可用的中间槽位。
            let start = pos.expect("A middle slot of directory entry");

            if discrete {
                let end_sector = prev_sector.unwrap();
                // NOTE: 离散情况下，`start.nth`等于当前扇区的长目录项个数
                let longs_in_prev = n_long - start.nth;

                let (prev_longs, next_longs) = longs.split_at(longs_in_prev);

                sector::get(end_sector)
                    .lock()
                    .map_mut_slice(|dirents: &mut [LongDirEntry]| {
                        dirents[sector_dirents - longs_in_prev..].copy_from_slice(prev_longs)
                    });

                sector::get(start.sector)
                    .lock()
                    .map_mut_slice(|dirents: &mut [LongDirEntry]| {
                        dirents[..start.nth].copy_from_slice(next_longs)
                    });
            } else {
                sector::get(start.sector)
                    .lock()
                    .map_mut_slice(|dirents: &mut [LongDirEntry]| {
                        dirents[start.nth - n_long..start.nth].copy_from_slice(&longs)
                    });
            }

            start.access_mut(|dirent| *dirent = short);

            return Some((cluster_id, start));
        }

        /* 尝试利用尾空槽 */
        if let Some(end) = pos {
            let need_next_sectors = (n_long + 1).saturating_sub(sector_dirents - end.nth);
            let start_nth = (end.nth + n_long) % sector_dirents; // inclusive

            let start = if need_next_sectors == 0 {
                // 终点和起点都在同一扇区
                sector::get(end.sector)
                    .lock()
                    .map_mut_slice(|dirents: &mut [LongDirEntry]| {
                        dirents[end.nth..start_nth].copy_from_slice(&longs)
                    });
                DirEntryPos::new(end.sector, start_nth)
            } else {
                // 终点和起点在相异的扇区
                let longs_in_prev = sector_dirents - end.nth;

                let (prev_longs, next_longs) = longs.split_at(longs_in_prev);

                let start_sector = if let Some(sc) = sectors.next() {
                    sc
                } else {
                    drop(sectors);
                    let last_cid = sb.fat().last(self.start_id).unwrap();
                    let (ncid, new_sectors) = sb.alloc_cluster();
                    unsafe {
                        sb.fat_mut().couple(last_cid, ncid);
                    }
                    new_sectors.start
                };

                sector::get(end.sector)
                    .lock()
                    .map_mut_slice(|dirents: &mut [LongDirEntry]| {
                        dirents[sector_dirents - longs_in_prev..].copy_from_slice(prev_longs)
                    });

                sector::get(start_sector)
                    .lock()
                    .map_mut_slice(|dirents: &mut [LongDirEntry]| {
                        dirents[..start_nth].copy_from_slice(next_longs)
                    });

                DirEntryPos::new(start_sector, start_nth)
            };

            start.access_mut(|dirent| *dirent = short);

            return Some((cluster_id, start));
        }

        /* 尝试分配新块 */
        drop(sectors);
        let last = sb.fat().last(self.start_id).unwrap();
        let (ncid, sectors) = sb.alloc_cluster();
        unsafe {
            sb.fat_mut().couple(last, ncid);
        }
        sector::get(sectors.start)
            .lock()
            .map_mut_slice(|dirents: &mut [DirEntry]| {
                for (dirent, long) in dirents.iter_mut().zip(longs) {
                    dirent.long = long;
                }
                dirents[n_long].short = short;
            });

        Some((cluster_id, DirEntryPos::new(sectors.start, n_long)))
    }

    fn alloc_dir(&self, dir: &mut ShortDirEntry, sb: &mut FatFileSystem) -> ClusterId<u32> {
        let (ncid, sectors) = sb.alloc_cluster();
        dir.set_cluster_id(ncid);
        dir.attr |= AttrFlag::Directory;
        sector::get(sectors.start)
            .lock()
            .map_mut_slice(|dirents: &mut [ShortDirEntry]| {
                let mut cwd = *dir;
                cwd.name.fill(0);
                cwd.name[0] = b'.';

                let mut parent = ShortDirEntry::default();
                parent.set_cluster_id(self.start_id);
                parent.attr |= AttrFlag::Directory;
                parent.name[..2].copy_from_slice(b"..");

                dirents[0] = cwd;
                dirents[1] = parent;
            });
        ncid
    }
}

#[derive(Debug, Clone, Copy)]
struct DirEntryPos {
    sector: SectorId,
    nth: usize,
}

impl DirEntryPos {
    /// 根目录性质特殊，目录项位置无关紧要，占个位就行。
    pub const ROOT: Self = DirEntryPos::new(SectorId::new(0), 0);

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
            kind: dirent.attr.into(),
        }
    }
}
