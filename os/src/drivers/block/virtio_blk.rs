use alloc::vec::Vec;
use easy_fs::BlockDevice;
use lazy_static::lazy_static;
use virtio_drivers::{Hal, VirtIOBlk, VirtIOHeader};

use crate::board::MMIO;
use crate::memory::address::{PhysAddr, PhysPageNum, VirtAddr};
use crate::memory::frame_allocator;
use crate::memory::frame_allocator::Frame;
use crate::memory::{PageTable, KERNEL_SPACE};
use crate::sync::UPSafeCell;

lazy_static! {
    static ref QUEUE_FRAMES: UPSafeCell<Vec<Frame>> = unsafe { UPSafeCell::new(Vec::new()) };
}

pub struct VirtIOBlock(UPSafeCell<VirtIOBlk<'static, VirtioHal>>);

// VirtIO 设备需要占用部分内存作为一个公共区域从而更好的和 CPU 进行合作。
// 在 VirtIO 架构下，需要在公共区域中放置一种叫做 `VirtQueue` 的环形队列，
// CPU 可以向此环形队列中向 VirtIO 设备提交请求，也可以从队列中取得请求的结果。
//
// 使用 `VirtQueue` 涉及到物理内存的分配和回收，
// 但这并不在 VirtIO 驱动 virtio-drivers 的职责范围之内，
// 因此它声明了数个相关的接口，需要库的使用者自己来实现。
struct VirtioHal;

impl BlockDevice for VirtIOBlock {
    #[inline]
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        self.0
            .exclusive_access()
            .read_block(block_id, buf)
            .expect("error when reading virtio block");
    }

    #[inline]
    fn write_block(&self, block_id: usize, buf: &[u8]) {
        self.0
            .exclusive_access()
            .write_block(block_id, buf)
            .expect("error when writting virtio block");
    }
}

impl VirtIOBlock {
    const VIRIO0: usize = MMIO[1].0;

    #[inline]
    pub fn new() -> Self {
        unsafe {
            Self(UPSafeCell::new(
                VirtIOBlk::<VirtioHal>::new(&mut *(Self::VIRIO0 as *mut VirtIOHeader)).unwrap(),
            ))
        }
    }
}

// 直接存储器访问 (DMA)
impl Hal for VirtioHal {
    fn dma_alloc(pages: usize) -> virtio_drivers::PhysAddr {
        let mut ppn_base = PhysPageNum::from_raw(0);
        for i in 0..pages {
            let frame = frame_allocator::alloc().unwrap();
            if i == 0 {
                ppn_base = frame.ppn;
            }
            assert_eq!(frame.ppn, ppn_base + i);
            QUEUE_FRAMES.exclusive_access().push(frame);
        }

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

    #[inline]
    fn phys_to_virt(paddr: virtio_drivers::PhysAddr) -> virtio_drivers::VirtAddr {
        // 恒等映射
        paddr
    }

    fn virt_to_phys(vaddr: virtio_drivers::VirtAddr) -> virtio_drivers::PhysAddr {
        PageTable::from_token(KERNEL_SPACE.exclusive_access().token())
            .translate_virt_addr(VirtAddr::from(vaddr))
            .unwrap()
            .into()
    }
}

// unsafe impl Hal for VirtioHal {
//     fn dma_alloc(
//         pages: usize,
//         _direction: BufferDirection,
//     ) -> (virtio_drivers::PhysAddr, NonNull<u8>) {
//         let mut ppn_base = PhysPageNum::from_raw(0);
//         for i in 0..pages {
//             let frame = frame_allocator::alloc().unwrap();
//             if i == 0 {
//                 ppn_base = frame.ppn;
//             }
//             assert_eq!(frame.ppn, ppn_base + i);
//             QUEUE_FRAMES.exclusive_access().push(frame);
//         }
//
//         let pa_base: usize = PhysAddr::from(ppn_base).into();
//         let va_base: VirtAddr = pa_base.into();
//         // DMA的地址是恒等映射的
//         (
//             pa_base,
//             NonNull::new(usize::from(va_base) as *mut u8).unwrap(),
//         )
//     }
//
//     unsafe fn dma_dealloc(
//         paddr: virtio_drivers::PhysAddr,
//         _vaddr: NonNull<u8>,
//         pages: usize,
//     ) -> i32 {
//         let paddr = PhysAddr::from(paddr);
//         let mut ppn_base = paddr.page_number();
//         for _ in 0..pages {
//             frame_allocator::dealloc(ppn_base);
//             ppn_base += 1;
//         }
//         0
//     }
//
//     unsafe fn mmio_phys_to_virt(paddr: virtio_drivers::PhysAddr, _size: usize) -> NonNull<u8> {
//         let paddr = PhysAddr::from(paddr);
//         let vaddr: VirtAddr = usize::from(paddr).into();
//         NonNull::new(usize::from(vaddr) as *mut u8).unwrap()
//     }
//
//     unsafe fn share(
//         buffer: NonNull<[u8]>,
//         _direction: BufferDirection,
//     ) -> virtio_drivers::PhysAddr {
//         let va: VirtAddr = (buffer.as_ref().as_ptr() as usize).into();
//         PageTable::from_token(KERNEL_SPACE.exclusive_access().token())
//             .translate_virt_addr(va)
//             .unwrap()
//             .into()
//     }
//
//     unsafe fn unshare(
//         _paddr: virtio_drivers::PhysAddr,
//         _buffer: NonNull<[u8]>,
//         _direction: BufferDirection,
//     ) {
//         // Nothing to do,
//         // as the host already has access to all memory and
//         // we didn't copy the buffer anywhere else.
//     }
// }
