use alloc::string::String;

#[derive(Debug, Clone)]
pub struct Directory {
    pub path: String,
    pub inode_id: u64,
}

impl Directory {
    pub fn fat_root() -> Self {
        Self {
            path: String::from("/"),
            inode_id: 2,
        }
    }
}
