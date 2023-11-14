//! # 块缓存层
//!
//! 块设备读写速度一般慢于内存读写速度，因此我们在内存中开辟缓冲区，
//! 把即将操作的块复制到内存中，提高对块设备的操作效率。
//! 同时，块缓存层也会尝试返回已缓存的块。
//!
//! 块缓存层对使用者来说是透明的，使用者对块设备的操作都经过块缓存层，
//! 且**操作块时一定在缓冲区当中**。
//!
//! 缓存与块设备同步后并不会移除块缓存，该操作由缓存管理器调度执行。

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::mem;

use spin::Mutex;

use crate::BlockDevice;
use crate::BLOCK_SIZE;

static BLOCK_CACHE_MANAGER: Mutex<BlockCacheManager> = Mutex::new(BlockCacheManager::new());

/// 块缓存全局管理，缓存、调度块缓存
struct BlockCacheManager {
    queue: Vec<(usize, Arc<Mutex<BlockCache>>)>,
}

#[inline]
pub fn get(block_id: usize, block_device: Arc<dyn BlockDevice>) -> Arc<Mutex<BlockCache>> {
    BLOCK_CACHE_MANAGER.lock().get(block_id, block_device)
}

pub fn sync_all() {
    BLOCK_CACHE_MANAGER
        .lock()
        .queue
        .iter()
        .for_each(|(_, cache)| cache.lock().sync());
}

/// 内存中的块缓存
pub struct BlockCache {
    /// 缓存的数据
    data: [u8; BLOCK_SIZE],
    /// 对应的块ID
    block_id: usize,
    /// 底层块设备的引用
    block_device: Arc<dyn BlockDevice>,
    /// 是否为脏块
    modified: bool,
}

impl BlockCache {
    pub fn new(block_id: usize, block_device: Arc<dyn BlockDevice>) -> Self {
        let mut data = [0; BLOCK_SIZE];
        block_device.read_block(block_id, &mut data);

        Self {
            data,
            block_id,
            block_device,
            modified: false,
        }
    }

    pub fn sync(&mut self) {
        if self.modified {
            self.modified = false;
            self.block_device.write_block(self.block_id, &self.data);
        }
    }

    pub fn get<T: Sized>(&self, offset: usize) -> &T {
        let type_size = mem::size_of::<T>();
        assert!(type_size + offset <= BLOCK_SIZE);
        let addr = self.offset(offset).cast();
        unsafe { &*addr }
    }

    pub fn get_mut<T: Sized>(&mut self, offset: usize) -> &mut T {
        let type_size = mem::size_of::<T>();
        assert!(type_size + offset <= BLOCK_SIZE);
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

impl BlockCache {
    #[inline]
    fn offset(&self, count: usize) -> *const u8 {
        &self.data[count]
    }
}

impl Drop for BlockCache {
    fn drop(&mut self) {
        self.sync();
    }
}

impl BlockCacheManager {
    /// 块缓存个数的上限
    const CAPACITY: usize = 16;

    const fn new() -> Self {
        Self { queue: Vec::new() }
    }

    // 块缓存调度策略：踢走闲置块
    fn get(
        &mut self,
        block_id: usize,
        block_device: Arc<dyn BlockDevice>,
    ) -> Arc<Mutex<BlockCache>> {
        // 尝试从缓冲区中读取块
        if let Some(cache) = self
            .queue
            .iter()
            .find_map(|(id, cache)| (block_id == *id).then_some(cache))
        {
            return Arc::clone(cache);
        };

        // 触及上限，写回一个块
        if self.queue.len() == Self::CAPACITY {
            let index = self
                .queue
                .iter()
                .position(|(_, cache)| Arc::strong_count(cache) == 1) // 没有其它引用的才能写回
                .expect("run out of block cache");
            self.queue.remove(index);
        }

        // 缓存新块
        let block_cache = Arc::new(Mutex::new(BlockCache::new(block_id, block_device)));
        self.queue.push((block_id, block_cache.clone()));

        block_cache
    }
}
