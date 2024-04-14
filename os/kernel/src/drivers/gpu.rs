use alloc::boxed::Box;
use alloc::vec::Vec;
use core::slice;

use embedded_graphics::pixelcolor::Rgb888;
use spin::Lazy;
use tinybmp::Bmp;
use virtio_drivers::{VirtIOGpu, VirtIOHeader};

use super::bus::VirtioHal;
use crate::board::IrqId;
use crate::config::IMG_MOUSE;
use crate::sync::UpCell;

pub static GPU_DEVICE: Lazy<Box<dyn GpuDevice>> = Lazy::new(|| Box::new(VirtIOGpuWrapper::new()));

pub trait GpuDevice: Send + Sync {
    #[allow(dead_code)]
    fn update_cursor(&self);

    #[allow(clippy::mut_from_ref)]
    fn framebuffer(&self) -> &mut [u8];

    fn flush(&self);
}

pub struct VirtIOGpuWrapper {
    base: UpCell<VirtIOGpu<'static, VirtioHal>>,
    framebuffer: &'static [u8],
}

impl VirtIOGpuWrapper {
    pub fn new() -> Self {
        unsafe {
            let mut virtio =
                VirtIOGpu::new(&mut *(IrqId::GPU.virtio_mmio_addr() as *mut VirtIOHeader)).unwrap();

            // 设置virtio-gpu设备的显存，初始化显存的一维字节数组引用
            let fb = virtio.setup_framebuffer().unwrap();
            let framebuffer = slice::from_raw_parts_mut(fb.as_mut_ptr(), fb.len());

            // 初始化光标图像的像素值
            let bmp = Bmp::<Rgb888>::from_slice(IMG_MOUSE).unwrap();
            let mut buffer = Vec::new();
            // 按RGB形式拆分
            for pixel in bmp.as_raw().image_data().chunks(3) {
                buffer.extend_from_slice(pixel);
                let transparency = if pixel == [255, 255, 255] {
                    // 是白色像素，设为透明
                    0x0
                } else {
                    // 其它颜色，不透明
                    0xff
                };
                buffer.push(transparency);
            }
            virtio.setup_cursor(&buffer, 50, 50, 50, 50).unwrap();

            Self {
                base: UpCell::new(virtio),
                framebuffer,
            }
        }
    }
}

impl GpuDevice for VirtIOGpuWrapper {
    fn update_cursor(&self) {}

    fn flush(&self) {
        self.base.exclusive_access().flush().unwrap()
    }

    // 得到显存的基于内核态虚地址的一维字节数组引用
    fn framebuffer(&self) -> &mut [u8] {
        unsafe {
            let ptr = self.framebuffer.as_ptr().cast_mut().cast();
            slice::from_raw_parts_mut(ptr, self.framebuffer.len())
        }
    }
}
