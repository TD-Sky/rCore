use alloc::vec::Vec;

use virtio_drivers::Hal;

use crate::memory::{
    self, PageTable,
    address::PhysAddr,
    frame_allocator::{self, Frame},
};
use crate::sync::UpCell;

static QUEUE_FRAMES: UpCell<Vec<Frame>> = UpCell::new(Vec::new());

pub struct VirtioHal;

impl Hal for VirtioHal {
    fn dma_alloc(pages: usize) -> virtio_drivers::PhysAddr {
        let mut frames = frame_allocator::alloc_continuous(pages).unwrap();
        let ppn_base = frames.last().unwrap().ppn;
        QUEUE_FRAMES.exclusive_access().append(&mut frames);
        PhysAddr::from(ppn_base).into()
    }

    fn dma_dealloc(paddr: virtio_drivers::PhysAddr, pages: usize) -> i32 {
        let paddr = PhysAddr::from(paddr);
        let mut ppn_base = paddr.page_number();
        for _ in 0..pages {
            frame_allocator::dealloc(ppn_base);
            ppn_base += 1;
        }
        0
    }

    fn phys_to_virt(paddr: virtio_drivers::PhysAddr) -> virtio_drivers::VirtAddr {
        paddr // 恒等映射
    }

    fn virt_to_phys(vaddr: virtio_drivers::VirtAddr) -> virtio_drivers::PhysAddr {
        PageTable::from_token(memory::kernel_token())
            .translate_virt_addr(vaddr.into())
            .unwrap()
            .into()
    }
}
