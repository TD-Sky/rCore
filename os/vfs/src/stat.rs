use crate::DirEntryType;

#[derive(Debug)]
#[repr(C)]
pub struct Stat {
    pub mode: DirEntryType,
    /// Optimal I/O block size
    pub block_size: u64,
    /// Occupying blocks
    pub blocks: u64,
    /// File size
    pub size: u64,
}
