#[derive(Debug, Default)]
#[repr(C)]
pub struct StatFs {
    /// Type of filesystem
    pub _ty: u64,

    /// Optimal transfer block size
    pub block_size: u64,
    /// Total data blocks in filesystem
    pub blocks: u64,
    /// Free blocks in filesystem
    pub blocks_free: u64,
    /// Free blocks available to unprivileged user
    pub blocks_available: u64,

    /// Total inodes in filesystem
    pub files: u64,
    /// Free inodes in filesystem
    pub files_free: u64,

    /// Filesystem ID
    pub _sid: [i32; 0],

    /// Maximum length of filenames
    pub name_cap: u64,
}
