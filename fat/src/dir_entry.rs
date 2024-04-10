use enumflags2::{bitflags, BitFlags};

#[derive(Debug)]
pub struct DirEntry {
    name: [u8; 11],

    attr: u8,

    /// Reserved, must be 0
    _ntres: u8,

    /// Count of tenths of a second.
    /// Range: [0, 199]
    crt_time_tenth: u8,

    /// Creation time, granularity is 2 seconds
    crt_time: u16,

    /// Creation date
    crt_date: u16,

    /// Last access date
    lst_acc_date: u16,

    /// High word of first data cluster number
    /// for file/directory described by this entry
    fst_clus_hi: u16,

    /// Last modification time
    wrt_time: u16,

    /// Last modification date
    wrt_date: u16,

    /// Low word of first data cluster number
    /// for file/directory described by this entry
    fst_clus_lo: u16,

    /// Quantity containing size in bytes
    /// of file/directory described by this entry
    file_size: u32,
}

impl DirEntry {
    pub fn attr(&self) -> BitFlags<AttrFlag> {
        BitFlags::from_bits_truncate(self.attr)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[bitflags]
#[repr(u8)]
pub enum AttrFlag {
    ReadOnly = 0b0000_0001,
    Hidden = 0b0000_0010,
    /// The corresponding file is tagged as a component of the operating system
    System = 0b0000_0100,
    /// The corresponding entry contains the volume label
    VolumeID = 0b0000_1000,
    Directory = 0b0001_0000,
    /// Indicates that properties of the associated file have been modified
    Archive = 0b0010_0000,
}
