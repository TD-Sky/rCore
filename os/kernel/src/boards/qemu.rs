//! QEMU exporting items

pub use self::virt::*;
pub use crate::drivers::{init_device, irq_handler};

pub const CLOCK_FREQ: usize = 10_000_000; // Hz

/// 物理地址起始于`0x8000_0000`，我们现在有100M内存
pub const MEMORY_END: usize = 0x8100_0000;

/// [virtio 常量](https://github.com/qemu/qemu/blob/master/include/hw/riscv/virt.h)
#[allow(dead_code)]
mod virt {
    const CPUS_MAX_BITS: usize = 9;
    const CPUS_MAX: usize = 1 << CPUS_MAX_BITS;
    const SOCKETS_MAX_BITS: usize = 2;
    const SOCKETS_MAX: usize = 1 << SOCKETS_MAX_BITS;

    const PLIC_PRIORITY_BASE: usize = 0x00;
    const PLIC_PENDING_BASE: usize = 0x1000;
    pub const PLIC_ENABLE_BASE: usize = 0x2000;
    pub const PLIC_ENABLE_STRIDE: usize = 0x80;
    pub const PLIC_CONTEXT_BASE: usize = 0x20_0000;
    pub const PLIC_CONTEXT_STRIDE: usize = 0x1000;
    #[allow(non_snake_case)]
    const fn PLIC_SIZE(num: usize) -> usize {
        PLIC_CONTEXT_BASE + num * PLIC_CONTEXT_STRIDE
    }

    pub struct MemMapEntity {
        pub addr: usize,
        pub offset: usize,
    }

    impl MemMapEntity {
        pub const TEST: MemMapEntity = Self::new(0x10_0000, 0x1000);
        pub const RTC: MemMapEntity = Self::new(0x10_1000, 0x1000);
        pub const CLINT: MemMapEntity = Self::new(0x200_0000, 0x10000);
        pub const PLIC: MemMapEntity = Self::new(0xc00_0000, PLIC_SIZE(CPUS_MAX * 2));
        pub const UART0: MemMapEntity = Self::new(0x1000_0000, 0x100);
        // 此处的偏移量与`virt.h`内的不同，它涵盖了 0x1000_1000 ~ 0x1000_8000 的八个槽位
        pub const VIRTIO: MemMapEntity = Self::new(0x1000_1000, 0x8000);

        pub const fn new(addr: usize, offset: usize) -> Self {
            Self { addr, offset }
        }

        pub const fn segment(&self) -> (usize, usize) {
            (self.addr, self.addr + self.offset)
        }
    }

    /// 内存映射I/O (MMIO) 指的是外设的设备寄存器可以通过特定的物理内存地址来访问
    /// https://github.com/qemu/qemu/blob/master/hw/riscv/virt.c
    pub fn mmio_segments() -> impl Iterator<Item = (usize, usize)> {
        [
            MemMapEntity::TEST.segment(),
            MemMapEntity::RTC.segment(),
            MemMapEntity::CLINT.segment(),
            MemMapEntity::PLIC.segment(),
            MemMapEntity::UART0.segment(),
            MemMapEntity::VIRTIO.segment(),
        ]
        .into_iter()
    }

    /// 见头文件里的IRQ枚举
    #[derive(Debug, PartialEq, Eq)]
    pub struct IrqId(pub u32);

    impl IrqId {
        pub const KEYBOARD: IrqId = Self(5);
        pub const MOUSE: IrqId = Self(6);
        pub const GPU: IrqId = Self(7);
        pub const BLOCK: IrqId = Self(8);
        pub const SERIAL: IrqId = Self(10);

        pub const fn virtio_mmio_addr(&self) -> usize {
            assert!(1 <= self.0 && self.0 <= 8);
            MemMapEntity::VIRTIO.addr - 0x1000 + self.0 as usize * 0x1000
        }
    }

    pub fn irq_ids() -> impl Iterator<Item = IrqId> {
        [IrqId::KEYBOARD, IrqId::MOUSE, IrqId::BLOCK, IrqId::SERIAL].into_iter()
    }
}
