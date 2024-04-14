mod virtio_blk;

use alloc::sync::Arc;

use block_dev::BlockDevice;
use spin::Lazy;

use crate::sync::UpCell;

use self::virtio_blk::VirtIOBlock;

/// 初始化为轮询。
/// 因为中断IO需要利用休眠任务队列，而始祖进程创建前任务队列为空，
/// 所以必须通过轮询加载始祖进程，尔后才能利用中断IO
pub static DEV_IO_MODE: UpCell<IOMode> = UpCell::new(IOMode::Poll);

pub static BLOCK_DEVICE: Lazy<Arc<dyn BlockDevice>> = Lazy::new(|| Arc::new(VirtIOBlock::new()));

/// IO方式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IOMode {
    Interrupt,
    Poll,
}
