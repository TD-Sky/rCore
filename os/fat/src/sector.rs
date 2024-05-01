//! 扇区的抽象

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::mem;

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

impl SectorId {
    pub const fn new(raw: usize) -> Self {
        Self(raw)
    }

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

    pub fn get<T: Sized>(&self, offset: usize) -> &T {
        let type_size = mem::size_of::<T>();
        assert!(type_size + offset <= self.data.len());
        let addr = self.offset(offset).cast();
        unsafe { &*addr }
    }

    pub fn get_mut<T: Sized>(&mut self, offset: usize) -> &mut T {
        let type_size = mem::size_of::<T>();
        assert!(type_size + offset <= self.data.len());
        self.modified = true;
        let addr = self.offset(offset).cast_mut().cast();
        unsafe { &mut *addr }
    }

    #[inline]
    pub fn map<T: Sized, V>(&self, offset: usize, f: impl FnOnce(&T) -> V) -> V {
        f(self.get(offset))
    }

    #[inline]
    pub fn map_mut<T: Sized, V>(&mut self, offset: usize, f: impl FnOnce(&mut T) -> V) -> V {
        f(self.get_mut(offset))
    }
}

impl Sector {
    #[inline]
    fn offset(&self, count: usize) -> *const u8 {
        &self.data[count]
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
