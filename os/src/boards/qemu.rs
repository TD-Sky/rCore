//! Constants used in rCore for qemu

pub const CLOCK_FREQ: usize = 10_000_000; // Hz

/// 物理地址起始于`0x8000_0000`，我们现在有100M内存
pub const MEMORY_END: usize = 0x8100_0000;

// 内存映射I/O (MMIO) 指的是外设的设备寄存器可以通过特定的物理内存地址来访问
// https://github.com/qemu/qemu/blob/master/hw/riscv/virt.c
pub const MMIO: &[(usize, usize)] = &[
    (0x0010_0000, 0x1000), // VIRT_TEST
    (0x1000_1000, 0x1000), // VIRT_VIRTIO
];

pub type BlockDeviceImpl = crate::drivers::block::VirtIOBlock;
