use alloc::vec::Vec;
use core::mem;

use vfs::{DirEntryType, Stat};

use crate::volume::data::*;
use crate::volume::reserved::bpb;
use crate::{sector, ClusterId, FatFileSystem, SectorId};

pub static ROOT: Inode = Inode {
    start_id: ClusterId::MIN,
    range: DirEntryRange::ROOT,
    ty: DirEntryType::Directory,
};

/// 目录项会指向一个簇链表，这就是FAT文件系统中的inode。
///
/// 理论上每个[`Inode`]是唯一的、目录项无关的，但为了实用，
/// 我们不得不将其与目录项的位置与属性关联起来。
#[derive(Debug, Clone)]
pub struct Inode {
    start_id: ClusterId<u32>,
    range: DirEntryRange,
    ty: DirEntryType,
}

impl Inode {
    pub fn id(&self) -> u64 {
        self.start_id.into()
    }

    pub fn kind(&self) -> DirEntryType {
        self.ty
    }

    /// 目录
    ///
    /// # 参数
    ///
    /// `relat_path`: 相对于[`Inode`]的相对路径，不能出现`.`或`..`。
    pub fn find(&self, relat_path: &str, sb: &FatFileSystem) -> Option<Self> {
        debug_assert_eq!(self.ty, DirEntryType::Directory);

        let mut cmps = relat_path.split('/');
        let mut inode = self.clone();
        let basename = cmps.next_back()?;
        for cmp in cmps {
            let cmp_inode = inode.find_cwd(cmp, sb)?;
            if cmp_inode.ty != DirEntryType::Directory {
                log::error!("Middle segment isn't directory");
                return None;
            }
            inode = cmp_inode;
        }
        inode.find_cwd(basename, sb)
    }

    /// 文件
    pub fn read_at(&self, offset: usize, buf: &mut [u8], sb: &FatFileSystem) -> usize {
        debug_assert_eq!(self.ty, DirEntryType::Regular);

        let file_size = self.range.short.access(ShortDirEntry::size);
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
    pub fn create_file(&self, name: &str, sb: &mut FatFileSystem) -> Result<Self, vfs::Error> {
        debug_assert_eq!(self.ty, DirEntryType::Directory);

        // NOTE: 出来的是默认值，不需要赋予[`ClusterId::FREE`]了
        let (short, longs) = name2dirents(name);
        let range = self.create(name, short, longs, sb)?;
        sector::sync_all();

        Ok(Self {
            start_id: ClusterId::FREE,
            range,
            ty: DirEntryType::Regular,
        })
    }

    /// 文件
    ///
    /// 随机写入，对于空文件会分配有效的起始簇编号再写入。
    pub fn write_at(&mut self, offset: usize, buf: &[u8], sb: &mut FatFileSystem) -> usize {
        debug_assert_eq!(self.ty, DirEntryType::Regular);

        let file_size = self.range.short.access(ShortDirEntry::size);
        let sector_size = bpb().sector_bytes();

        let start = offset;
        let end = start + buf.len(); // exclusive

        // Expand
        if end > file_size {
            let added_sectors = (end - file_size).div_ceil(sector_size);
            debug_assert!(added_sectors > 0);

            let mut added_clusters = added_sectors.div_ceil(bpb().cluster_sectors());

            let mut current = if self.start_id == ClusterId::FREE {
                /* 空文件 */
                added_clusters -= 1;
                self.start_id = sb.alloc_cluster().0;
                self.range
                    .short
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
            self.range.short.access_mut(|dirent| dirent.resize(end));
        }
        sector::sync_all();

        wrote_size
    }

    /// 文件
    pub fn clear(&mut self, sb: &mut FatFileSystem) {
        debug_assert_eq!(self.ty, DirEntryType::Regular);

        // 跳过空文件
        if self.start_id != ClusterId::FREE {
            sb.fat_mut().dealloc(self.start_id).unwrap();
            self.start_id = ClusterId::FREE;
            self.range.short.access_mut(|dirent| dirent.resize(0));
        }
    }

    /// 目录
    ///
    /// 在当前目录下创建目录。
    pub fn mkdir(&self, name: &str, sb: &mut FatFileSystem) -> Result<Self, vfs::Error> {
        debug_assert_eq!(self.ty, DirEntryType::Directory);

        let (mut short, longs) = name2dirents(name);
        let start_id = self.alloc_dir(&mut short, sb);
        let range = self.create(name, short, longs, sb)?;
        sector::sync_all();

        Ok(Self {
            start_id,
            range,
            ty: DirEntryType::Directory,
        })
    }

    /// 目录
    ///
    /// 读取at之后的目录项，最多为count个。
    pub fn ls_at(&self, at: usize, count: usize, sb: &FatFileSystem) -> Vec<vfs::DirEntry> {
        debug_assert_eq!(self.ty, DirEntryType::Directory);

        let mut buf = Vec::with_capacity(count);
        let mut skipped = 0;
        let sectors = sb.data_sectors(self.start_id);
        let mut read = 0;

        let mut prev_sector = None;
        for sid in sectors {
            let dirents = sector::get(sid);
            let dirents = dirents.lock();
            let dirents: &[DirEntry] = dirents.as_slice();

            for (i, dirent) in dirents
                .iter()
                .take_while(|dirent| unsafe { dirent.short.status() != DirEntryStatus::TailFree })
                .enumerate()
            {
                if read == count {
                    return buf;
                }

                if unsafe {
                    dirent.short.status() == DirEntryStatus::Occupied
                        && dirent.attr() != LongDirEntry::attr()
                        && !dirent.short.is_relative()
                } {
                    if skipped < at {
                        skipped += 1;
                        continue;
                    }

                    let checksum = unsafe { dirent.short.checksum() };
                    log::debug!(
                        "parent={} pos=({sid}, {i}) checksum={checksum:#x}",
                        self.start_id
                    );
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
                    buf.push(unsafe {
                        vfs::DirEntry {
                            inode: dirent.short.cluster_id().into(),
                            ty: if dirent.attr().contains(AttrFlag::Directory) {
                                DirEntryType::Directory
                            } else {
                                DirEntryType::Regular
                            },
                            name: dname,
                        }
                    });
                    read += 1;
                }
            }

            prev_sector = Some(sid);
        }

        buf
    }

    pub fn stat(&self, sb: &FatFileSystem) -> Stat {
        Stat {
            mode: self.ty,
            block_size: bpb().sector_bytes() as u64,
            blocks: sb.data_sectors(self.start_id).count() as u64,
            size: self.range.short.access(ShortDirEntry::size) as u64,
        }
    }

    /// 目录
    pub fn unlink(&mut self, name: &str, sb: &mut FatFileSystem) -> Result<(), vfs::Error> {
        debug_assert_eq!(self.ty, DirEntryType::Directory);

        let inode = self.find_cwd(name, sb).ok_or(vfs::Error::NotFound)?;
        if inode.ty == DirEntryType::Directory {
            return Err(vfs::Error::IsADirectory);
        }
        if inode.start_id != ClusterId::FREE {
            sb.fat_mut().dealloc(inode.start_id).unwrap();
        }
        self.remove(inode.range, sb);

        Ok(())
    }

    /// 目录
    ///
    /// 删除空目录。
    pub fn rmdir(&mut self, name: &str, sb: &mut FatFileSystem) -> Result<(), vfs::Error> {
        debug_assert_eq!(self.ty, DirEntryType::Directory);

        let inode = self.find_cwd(name, sb).ok_or(vfs::Error::NotFound)?;
        if inode.ty != DirEntryType::Directory {
            return Err(vfs::Error::NotADirectory);
        } else if !inode.is_empty_dir(sb) {
            return Err(vfs::Error::DirectoryNotEmpty);
        }

        sb.fat_mut().dealloc(inode.start_id).unwrap();
        self.remove(inode.range, sb);

        Ok(())
    }

    /// 目录
    ///
    /// 当`new_parent`为`None`时，`old_name`和`new_name`必须不同。
    pub fn rename(
        &self,
        old_name: &str,
        new_parent: Option<&mut Self>,
        new_name: &str,
        sb: &mut FatFileSystem,
    ) -> Result<(), vfs::Error> {
        debug_assert_eq!(self.ty, DirEntryType::Directory);

        let src = self.find_cwd(old_name, sb).ok_or(vfs::Error::NotFound)?;
        let (short, new_longs) = src
            .range
            .short
            .access(|short| rename_dirents(short, new_name));

        if let Some(new_parent) = new_parent {
            // Rename to another directory
            debug_assert_eq!(new_parent.ty, DirEntryType::Directory);

            if let Some(dest) = new_parent.find_cwd(new_name, sb) {
                match (src.ty, dest.ty) {
                    (DirEntryType::Directory, DirEntryType::Directory) => {
                        // 对于非空的目录，删除失败就退出
                        new_parent.rmdir(new_name, sb)?;
                        self.remove(src.range, sb);
                        new_parent.create(new_name, short, new_longs, sb)?;
                    }
                    (_, DirEntryType::Directory) => return Err(vfs::Error::IsADirectory),
                    (DirEntryType::Directory, _) => return Err(vfs::Error::NotADirectory),
                    _ => todo!(),
                }
            }
        } else {
            // Rename currently
            debug_assert_ne!(old_name, new_name);
            self.remove(src.range, sb);
            self.create(new_name, short, new_longs, sb)?;
        }

        Ok(())
    }
}

impl Inode {
    /// 目录
    ///
    /// 搜索当前目录下指定名称的项。
    fn find_cwd(&self, name: &str, sb: &FatFileSystem) -> Option<Self> {
        let checksum = ShortDirEntry::checksum_from(name.as_bytes());
        log::debug!("Checksum of {name}: {checksum:#x}");

        let mut prev_sector = None;
        for sid in sb.data_sectors(self.start_id) {
            let dirents = sector::get(sid);
            let dirents = dirents.lock();
            let dirents: &[DirEntry] = dirents.as_slice();

            for (i, dirent) in dirents
                .iter()
                .take_while(|dirent| unsafe { dirent.short.status() != DirEntryStatus::TailFree })
                .enumerate()
            {
                if unsafe {
                    dirent.short.status() == DirEntryStatus::Occupied
                        && dirent.attr() != LongDirEntry::attr()
                        && dirent.short.checksum() == checksum
                } {
                    let mut longs = Vec::with_capacity(10);

                    let mut end = None;
                    let mut discrete = true;

                    for (nth, dirent) in
                        dirents
                            .iter()
                            .enumerate()
                            .take(i)
                            .rev()
                            .take_while(|(_, dirent)| unsafe {
                                dirent.attr() == LongDirEntry::attr()
                                    && dirent.long.chksum == checksum
                            })
                    {
                        longs.push(unsafe { LongDirEntry::clone(&dirent.long) });
                        if unsafe {
                            dirent.long.ord & LongDirEntry::LAST_MASK == LongDirEntry::LAST_MASK
                        } {
                            end = Some(DirEntryPos::new(sid, nth));
                            discrete = false;
                            break;
                        }
                    }

                    if discrete {
                        let prev = prev_sector.unwrap();
                        sector::get(prev).lock().map_slice(|dirents: &[DirEntry]| {
                            let nth = dirents
                                .iter()
                                .rposition(|dirent| unsafe {
                                    dirent.attr() == LongDirEntry::attr()
                                        && dirent.long.chksum == checksum
                                        && (dirent.long.ord & LongDirEntry::LAST_MASK
                                            == LongDirEntry::LAST_MASK)
                                })
                                .expect("The last long entry was lost");

                            end = Some(DirEntryPos::new(prev, nth));

                            longs.extend(
                                dirents[nth..]
                                    .iter()
                                    .rev()
                                    .map(|dirent| unsafe { LongDirEntry::clone(&dirent.long) }),
                            );
                        });
                    }

                    let dname = dirents2name(&longs);
                    if name == dname {
                        let start = DirEntryPos::new(sid, i);
                        let range = DirEntryRange::new(end.unwrap(), start);
                        let dirent: &ShortDirEntry = unsafe { &dirent.short };
                        return Some((range, dirent).into());
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
        short: ShortDirEntry,
        longs: Vec<LongDirEntry>,
        sb: &mut FatFileSystem,
    ) -> Result<DirEntryRange, vfs::Error> {
        if self.find_cwd(name, sb).is_some() {
            return Err(vfs::Error::AlreadyExists);
        }

        let sector_dirents = sector_dirents();

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
                        DirEntryStatus::TailFree => {
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
            let short_pos = pos.expect("A middle slot of directory entry");
            let last_long_pos = if discrete {
                let sector = prev_sector.unwrap();
                // NOTE: 离散情况下，`start.nth`等于当前扇区的长目录项个数
                let longs_in_prev = n_long - short_pos.nth;
                let nth = sector_dirents - longs_in_prev;

                DirEntryPos::new(sector, nth)
            } else {
                let nth = short_pos.nth - n_long;

                DirEntryPos::new(short_pos.sector, nth)
            };

            let range = DirEntryRange::new(last_long_pos, short_pos);
            range.write_longs(&longs);
            short_pos.access_mut(|dirent| *dirent = short);

            return Ok(range);
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

            return Ok(DirEntryRange::new(end, start));
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
        let end = DirEntryPos::new(sectors.start, 0);
        let start = DirEntryPos::new(sectors.start, n_long);

        Ok(DirEntryRange::new(end, start))
    }

    fn alloc_dir(&self, dir: &mut ShortDirEntry, sb: &mut FatFileSystem) -> ClusterId<u32> {
        let (ncid, sectors) = sb.alloc_cluster();
        dir.set_cluster_id(ncid);
        dir.attr |= AttrFlag::Directory;
        sector::get(sectors.start)
            .lock()
            .map_mut_slice(|dirents: &mut [ShortDirEntry]| {
                dirents[0] = dir.as_cwd();
                dirents[1] = ShortDirEntry::new_parent(self.start_id);
            });
        ncid
    }

    fn remove(&self, range: DirEntryRange, sb: &mut FatFileSystem) {
        let sector_dirents = sector_dirents();

        let mut cursor = sb.data_sector_cursor(self.start_id);

        cursor
            .find(range.short.sector)
            .expect("range has been checked");
        let tail_status = if range.short.nth + 1 == sector_dirents {
            // 判断依据在下一个扇区
            cursor
                .next()
                .map(|cursor| {
                    sector::get(cursor.sector())
                        .lock()
                        .map(0, |dirent: &ShortDirEntry| dirent.status())
                })
                .unwrap_or(DirEntryStatus::TailFree)
        } else {
            // 判断依据在当前扇区
            sector::get(range.short.sector).lock().map(
                (range.short.nth + 1) * mem::size_of::<ShortDirEntry>(),
                |dirent: &ShortDirEntry| dirent.status(),
            )
        };

        let mut head_pos = None;
        cursor
            .rfind(range.last_long.sector)
            .expect("range has been checked");
        let head_status = if range.last_long.nth == 0 {
            // 判断依据在上一个扇区
            cursor
                .prev()
                .map(|cursor| {
                    let pos = DirEntryPos::new(cursor.sector(), sector_dirents - 1);
                    head_pos = Some(pos);
                    pos.access(|dirent| dirent.status())
                })
                .unwrap_or_else(|| {
                    if tail_status == DirEntryStatus::TailFree {
                        DirEntryStatus::TailFree
                    } else {
                        DirEntryStatus::Free
                    }
                })
        } else {
            // 判断依据在当前扇区
            let mut pos = range.last_long;
            pos.nth -= 1;
            head_pos = Some(pos);
            pos.access(|dirent: &ShortDirEntry| {
                if dirent.is_relative() {
                    DirEntryStatus::TailFree
                } else {
                    dirent.status()
                }
            })
        };

        match (head_status, tail_status) {
            /* Occupied + TF */
            (DirEntryStatus::Occupied, DirEntryStatus::TailFree) => range.clear(&TAIL_FREE),
            /* Free|Occupied + Occupied|Free */
            (
                DirEntryStatus::Free | DirEntryStatus::Occupied,
                DirEntryStatus::Free | DirEntryStatus::Occupied,
            ) => range.clear(&FREE),
            /* TF + Any */
            (DirEntryStatus::TailFree, _) => unreachable!(),
            /* Free + TF */
            (DirEntryStatus::Free, DirEntryStatus::TailFree) => {
                let head_pos = head_pos.unwrap();
                let mut end = head_pos.nth + 1; // exclusive

                let free_as;
                let mut start = loop {
                    let nth = sector::get(cursor.sector()).lock().map_slice(
                        |dirents: &[ShortDirEntry]| {
                            dirents[..end]
                                .iter()
                                .rposition(|dirent| {
                                    dirent.status() == DirEntryStatus::Occupied
                                        && !dirent.is_relative()
                                })
                                .map(|n| n + 1)
                        },
                    );

                    if let Some(mut nth) = nth {
                        free_as = &FREE;
                        if nth == sector_dirents {
                            cursor
                                .next()
                                .expect("it won't be the last dirent of directory");
                            nth = 0
                        }
                        break nth;
                    }

                    end = sector_dirents;
                    if cursor.prev().is_none() {
                        free_as = &TAIL_FREE;
                        break if self.start_id == ClusterId::MIN {
                            0
                        } else {
                            2
                        };
                    }
                };

                loop {
                    if cursor.sector() == range.short.sector {
                        sector::get(cursor.sector()).lock().map_mut_slice(
                            |dirents: &mut [FreeDirEntry]| {
                                dirents[start..=range.short.nth].fill(*free_as)
                            },
                        );
                        break;
                    }

                    sector::get(cursor.sector()).lock().map_mut_slice(
                        |dirents: &mut [FreeDirEntry]| dirents[start..].fill(*free_as),
                    );
                    start = 0;
                    cursor.next().expect("cursor won't beyond end");
                }
            }
        }
    }

    fn is_empty_dir(&self, sb: &FatFileSystem) -> bool {
        let mut sectors = sb.data_sectors(self.start_id);
        let i = if self.start_id == ClusterId::MIN {
            0
        } else {
            2
        };
        sector::get(sectors.next().unwrap()).lock().map(
            i * mem::size_of::<ShortDirEntry>(),
            |dirent: &ShortDirEntry| dirent.status() == DirEntryStatus::TailFree,
        )
    }
}

#[derive(Debug, Clone)]
struct DirEntryRange {
    /// 最后一个长目录项的位置，仅当为相对目录时才是[`None`]
    last_long: DirEntryPos,
    /// 短目录项的位置
    short: DirEntryPos,
}

impl DirEntryRange {
    const ROOT: Self = Self {
        last_long: DirEntryPos::ROOT,
        short: DirEntryPos::ROOT,
    };

    const fn new(end: DirEntryPos, start: DirEntryPos) -> Self {
        Self {
            last_long: end,
            short: start,
        }
    }

    fn is_discrete(&self) -> bool {
        self.last_long.sector != self.short.sector
    }

    fn write_longs(&self, longs: &[LongDirEntry]) {
        let Self { last_long, short } = self;

        if self.is_discrete() {
            // NOTE: 离散情况下，`short.nth`等于当前扇区的长目录项个数
            let longs_in_prev = longs.len() - short.nth;

            let (prev_longs, next_longs) = longs.split_at(longs_in_prev);
            sector::get(last_long.sector)
                .lock()
                .map_mut_slice(|dirents: &mut [LongDirEntry]| {
                    dirents[last_long.nth..].copy_from_slice(prev_longs)
                });
            sector::get(short.sector)
                .lock()
                .map_mut_slice(|dirents: &mut [LongDirEntry]| {
                    dirents[..short.nth].copy_from_slice(next_longs)
                });
        } else {
            sector::get(short.sector)
                .lock()
                .map_mut_slice(|dirents: &mut [LongDirEntry]| {
                    dirents[last_long.nth..short.nth].copy_from_slice(longs)
                });
        }
    }

    fn clear(&self, free_as: &FreeDirEntry) {
        let Self { last_long, short } = self;

        if self.is_discrete() {
            sector::get(last_long.sector)
                .lock()
                .map_mut_slice(|dirents: &mut [FreeDirEntry]| {
                    dirents[last_long.nth..].fill(*free_as);
                });
            sector::get(short.sector)
                .lock()
                .map_mut_slice(|dirents: &mut [FreeDirEntry]| dirents[..=short.nth].fill(*free_as));
        } else {
            sector::get(short.sector)
                .lock()
                .map_mut_slice(|dirents: &mut [FreeDirEntry]| {
                    dirents[last_long.nth..=short.nth].fill(*free_as);
                });
        }
    }

    /* fn shrink_longs(&mut self, n_old_long: usize, new_longs: &[LongDirEntry]) {
        let is_discrete = self.is_discrete();
        let n_prune = n_old_long - new_longs.len();
        let Self { last_long, short } = self;

        let old_end = *last_long;

        if !is_discrete || old_end.nth + n_prune < sector_dirents() {
            // 收缩后last_long的扇区不变
            last_long.nth += n_prune;

            sector::get(last_long.sector)
                .lock()
                .map_mut_slice(|dirents: &mut [FreeDirEntry]| {
                    dirents[old_end.nth..last_long.nth].fill(FREE)
                });
        } else {
            // 收缩到了下一扇区
            *last_long = DirEntryPos::new(short.sector, short.nth - new_longs.len());

            sector::get(old_end.sector)
                .lock()
                .map_mut_slice(|dirents: &mut [FreeDirEntry]| dirents[old_end.nth..].fill(FREE));
            sector::get(last_long.sector)
                .lock()
                .map_mut_slice(|dirents: &mut [FreeDirEntry]| dirents[..last_long.nth].fill(FREE));
        }

        self.write_longs(new_longs);
    } */
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

    /* pub fn get(&self) -> ShortDirEntry {
        *sector::get(self.sector)
            .lock()
            .get(self.nth * mem::size_of::<ShortDirEntry>())
    } */
}

impl From<(DirEntryRange, &ShortDirEntry)> for Inode {
    fn from((range, dirent): (DirEntryRange, &ShortDirEntry)) -> Self {
        Self {
            start_id: dirent.cluster_id(),
            range,
            ty: if dirent.attr.contains(AttrFlag::Directory) {
                DirEntryType::Directory
            } else {
                DirEntryType::Regular
            },
        }
    }
}
