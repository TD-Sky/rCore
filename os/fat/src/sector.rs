//! 扇区的抽象

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::iter::Step;
use core::mem;
use core::slice;

use block_dev::BlockDevice;
use derive_more::{Add, From, Into};
use spin::Mutex;
use spin::Once;

use crate::volume::reserved::bpb;

const BLOCK_SIZE: usize = 512;

static CACHE_MANAGER: Once<CacheManager> = Once::new();

pub fn init_cache(dev: &Arc<dyn BlockDevice>) {
    CACHE_MANAGER.call_once(|| CacheManager {
        dev: dev.clone(),
        queue: Mutex::default(),
    });
}

#[derive(Debug)]
struct CacheManager {
    /// 底层块设备的引用
    dev: Arc<dyn BlockDevice>,
    queue: Mutex<Vec<(SectorId, Arc<Mutex<Sector>>)>>,
}

#[inline]
fn manager() -> &'static CacheManager {
    unsafe { CACHE_MANAGER.get_unchecked() }
}

#[inline]
pub fn get(id: SectorId) -> Arc<Mutex<Sector>> {
    manager().get(id)
}

/// 内存中的扇区
#[derive(Debug)]
pub struct Sector {
    /// 缓存的数据
    data: Box<[u8]>,
    /// 对应的块ID
    id: SectorId,
    /// 是否为脏块
    modified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Add, From, Into)]
#[repr(transparent)]
pub struct SectorId(usize);

impl core::ops::Add<usize> for SectorId {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        self + Self(rhs)
    }
}

impl Step for SectorId {
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        usize::steps_between(&start.0, &end.0)
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        usize::forward_checked(start.0, count).map(Self)
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        usize::backward_checked(start.0, count).map(Self)
    }
}

impl SectorId {
    pub const fn new(raw: usize) -> Self {
        Self(raw)
    }

    /// 拉伸扇区号至块ID
    pub fn block(self) -> usize {
        self.0 * (bpb().sector_bytes() / BLOCK_SIZE)
    }
}

impl Sector {
    pub fn new(id: SectorId) -> Self {
        let mgr = manager();
        let mut data = vec![0; bpb().sector_bytes()];
        mgr.dev.read_block(id.block(), &mut data);

        Self {
            data: data.into(),
            id,
            modified: false,
        }
    }

    pub fn sync(&mut self) {
        if self.modified {
            self.modified = false;
            manager().dev.write_block(self.id.block(), &self.data);
        }
    }

    pub fn get<T>(&self, offset: usize) -> &T {
        let type_size = mem::size_of::<T>();
        assert!(type_size + offset <= self.data.len());
        let addr = &self.data[offset];
        unsafe { mem::transmute(addr) }
    }

    pub fn get_mut<T>(&mut self, offset: usize) -> &mut T {
        let type_size = mem::size_of::<T>();
        assert!(type_size + offset <= self.data.len());
        self.modified = true;
        let addr = &mut self.data[offset];
        unsafe { mem::transmute(addr) }
    }

    pub fn as_slice<T>(&self) -> &[T] {
        let type_size = mem::size_of::<T>();
        let len = self.data.len() / type_size;
        assert_eq!(0, self.data.len() % type_size);
        unsafe { slice::from_raw_parts(self.data.as_ptr().cast(), len) }
    }

    pub fn as_mut_slice<T>(&mut self) -> &mut [T] {
        let type_size = mem::size_of::<T>();
        let len = self.data.len() / type_size;
        assert_eq!(0, self.data.len() % type_size);
        self.modified = true;
        unsafe { slice::from_raw_parts_mut(self.data.as_mut_ptr().cast(), len) }
    }

    #[inline]
    pub fn map<T, V>(&self, offset: usize, f: impl FnOnce(&T) -> V) -> V {
        f(self.get(offset))
    }

    #[inline]
    pub fn map_mut<T, V>(&mut self, offset: usize, f: impl FnOnce(&mut T) -> V) -> V {
        f(self.get_mut(offset))
    }

    #[inline]
    pub fn map_slice<T, V>(&self, f: impl FnOnce(&[T]) -> V) -> V {
        f(self.as_slice())
    }

    #[inline]
    pub fn map_mut_slice<T, V>(&mut self, f: impl FnOnce(&mut [T]) -> V) -> V {
        f(self.as_mut_slice())
    }

    #[inline]
    pub fn zeroize(&mut self) {
        self.data.fill(0);
        self.modified = true;
    }
}

impl Drop for Sector {
    fn drop(&mut self) {
        self.sync();
    }
}

impl CacheManager {
    /// 块缓存个数的上限
    const CAPACITY: usize = 16;

    // 块缓存调度策略：踢走闲置块
    fn get(&self, id: SectorId) -> Arc<Mutex<Sector>> {
        let mut queue = self.queue.lock();

        // 尝试从缓冲区中读取块
        if let Some(cache) = queue
            .iter()
            .find_map(|(sid, cache)| (id == *sid).then_some(cache))
        {
            return Arc::clone(cache);
        };

        // 触及上限，写回一个块
        if queue.len() == Self::CAPACITY {
            let index = queue
                .iter()
                .position(|(_, cache)| Arc::strong_count(cache) == 1) // 没有其它引用的才能写回
                .expect("run out of block cache");
            queue.remove(index);
        }

        // 缓存新块
        let block_cache = Arc::new(Mutex::new(Sector::new(id)));
        queue.push((id, block_cache.clone()));

        block_cache
    }
}
