use alloc::vec::Vec;

use crate::volume::data::{dir_entry_name, DirEntry, DirEntryStatus, LongDirEntry, ShortDirEntry};
use crate::{sector, ClusterId, FatFileSystem};

/// 目录项会指向一个簇链表，这就是FAT文件系统中的inode
#[derive(Debug)]
pub struct Inode {
    start_id: ClusterId<u32>,
}

impl Inode {
    pub const fn new(id: ClusterId<u32>) -> Self {
        Self { start_id: id }
    }

    pub fn find(&self, name: &str, sb: &FatFileSystem) -> Option<ShortDirEntry> {
        let checksum = ShortDirEntry::checksum_from(name.as_bytes());
        let sectors = sb.data_sectors(self.start_id);

        let mut prev_sector = None;
        for sid in sectors {
            let dent = sector::get(sid);
            let dent = dent.lock();
            let dents: &[DirEntry] = dent.as_slice();

            for (i, dent) in dents
                .iter()
                .take_while(|dent| unsafe { dent.short.status() != DirEntryStatus::FreeHead })
                .enumerate()
            {
                if unsafe {
                    dent.short.status() == DirEntryStatus::Occupied
                        && dent.attr() != LongDirEntry::attr()
                        && dent.short.checksum() == checksum
                } {
                    let mut longs = Vec::with_capacity(10);

                    let mut discrete = true;

                    for dent in dents[..i].iter().rev().take_while(|dent| unsafe {
                        dent.attr() == LongDirEntry::attr() && dent.long.chksum == checksum
                    }) {
                        longs.push(unsafe { LongDirEntry::clone(&dent.long) });
                        if unsafe {
                            dent.long.ord & LongDirEntry::LAST_MASK == LongDirEntry::LAST_MASK
                        } {
                            discrete = false;
                            break;
                        }
                    }

                    if discrete {
                        let prev = prev_sector.unwrap();
                        sector::get(prev).lock().map_slice(|dents: &[DirEntry]| {
                            let end = dents
                                .iter()
                                .rposition(|dent| unsafe {
                                    dent.attr() == LongDirEntry::attr()
                                        && dent.long.chksum == checksum
                                        && (dent.long.ord & LongDirEntry::LAST_MASK
                                            == LongDirEntry::LAST_MASK)
                                })
                                .expect("The last long entry was lost");
                            longs.extend(
                                dents[end..]
                                    .iter()
                                    .rev()
                                    .map(|dent| unsafe { LongDirEntry::clone(&dent.long) }),
                            );
                        });
                    }

                    let dname = dir_entry_name(&longs);
                    if name == dname {
                        return Some(ShortDirEntry::clone(unsafe { &dent.short }));
                    }
                }
            }

            prev_sector = Some(sid);
        }

        None
    }
}
