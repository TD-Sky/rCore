#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ClusterId<T = u32>(T);

#[derive(Debug, PartialEq, Eq)]
pub enum ClusterError {
    Free,
    Defective,
    Reserved,
    Eof,
}

impl From<u32> for ClusterId<u32> {
    fn from(raw: u32) -> Self {
        Self(raw & 0x0FFF_FFFF)
    }
}

impl From<ClusterId<u32>> for u32 {
    fn from(id: ClusterId<u32>) -> Self {
        id.0
    }
}

impl From<ClusterId<u32>> for usize {
    fn from(id: ClusterId<u32>) -> Self {
        id.0 as usize
    }
}

impl ClusterId<u32> {
    pub const FREE: Self = Self(0);

    /// 最小的可用簇号
    pub const MIN: Self = Self(2);

    pub const EOF: Self = Self(u32::MAX);

    pub const BAD: Self = Self(0x0FFF_FFF7);

    pub const fn new(raw: u32) -> Self {
        Self(raw & 0x0FFF_FFFF)
    }

    /// NOTE: 没有FAT提供的真实最大可用簇编号，无法得知全部保留簇
    pub fn is_unavailable(&self) -> bool {
        *self < Self::MIN || (Self(0x0FFF_FFF8)..=Self(0x0FFF_FFFE)).contains(self)
    }

    pub fn validate(self) -> Result<Self, ClusterError> {
        match self {
            ClusterId::FREE => Err(ClusterError::Free),
            ClusterId::BAD => Err(ClusterError::Defective),
            ClusterId::EOF => Err(ClusterError::Eof),
            id if id.is_unavailable() => Err(ClusterError::Reserved),
            id => Ok(id),
        }
    }
}
