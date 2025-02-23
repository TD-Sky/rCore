use alloc::collections::BTreeMap;

use block_dev::BlockDevice;
use virtio_drivers::{BlkResp, RespStatus, VirtIOBlk, VirtIOHeader};

use super::{DEV_IO_MODE, IOMode};
use crate::board::IrqId;
use crate::drivers::bus::VirtioHal;
use crate::sync::{Condvar, UpCell};
use crate::task::processor;

pub struct VirtIOBlock {
    base: UpCell<VirtIOBlk<'static, VirtioHal>>,
    condvars: BTreeMap<u16, Condvar>,
}

impl core::fmt::Debug for VirtIOBlock {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VirtIOBlock")
            .field("base", &"Virtio HAL")
            .field("condvars", &self.condvars)
            .finish()
    }
}

// VirtIO 设备需要占用部分内存作为一个公共区域从而更好的和 CPU 进行合作。
// 在 VirtIO 架构下，需要在公共区域中放置一种叫做 `VirtQueue` 的环形队列，
// CPU 可以向此环形队列中向 VirtIO 设备提交请求，也可以从队列中取得请求的结果。
//
// 使用 `VirtQueue` 涉及到物理内存的分配和回收，
// 但这并不在 VirtIO 驱动 virtio-drivers 的职责范围之内，
// 因此它声明了数个相关的接口，需要库的使用者自己来实现。
// struct VirtioHal;

impl BlockDevice for VirtIOBlock {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        match *DEV_IO_MODE.exclusive_access() {
            IOMode::Interrupt => {
                let mut resp = BlkResp::default();
                let task_ctx_ptr = self.base.exclusive_session(|blk| {
                    let token = unsafe { blk.read_block_nb(block_id, buf, &mut resp).unwrap() };
                    self.condvars.get(&token).unwrap().wait()
                });
                processor::schedule(task_ctx_ptr);
                assert_eq!(resp.status(), RespStatus::Ok);
            }
            IOMode::Poll => {
                self.base
                    .exclusive_access()
                    .read_block(block_id, buf)
                    .unwrap();
            }
        }
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        match *DEV_IO_MODE.exclusive_access() {
            IOMode::Interrupt => {
                let mut resp = BlkResp::default();
                let task_ctx_ptr = self.base.exclusive_session(|blk| {
                    let token = unsafe { blk.write_block_nb(block_id, buf, &mut resp).unwrap() };
                    self.condvars.get(&token).unwrap().wait()
                });
                processor::schedule(task_ctx_ptr);
                assert_eq!(resp.status(), RespStatus::Ok);
            }
            IOMode::Poll => {
                self.base
                    .exclusive_access()
                    .write_block(block_id, buf)
                    .unwrap();
            }
        }
    }

    fn handle_irq(&self) {
        let mut blk = self.base.exclusive_access();
        while let Ok(token) = blk.pop_used() {
            self.condvars.get(&token).unwrap().signal()
        }
    }
}

impl VirtIOBlock {
    pub fn new() -> Self {
        let virtio_blk = unsafe {
            VirtIOBlk::<VirtioHal>::new(
                &mut *(IrqId::BLOCK.virtio_mmio_addr() as *mut VirtIOHeader),
            )
            .unwrap()
        };
        let condvars =
            BTreeMap::from_iter((0..virtio_blk.virt_queue_size()).map(|i| (i, Condvar::new())));

        Self {
            base: UpCell::new(virtio_blk),
            condvars,
        }
    }
}
