//! # 块设备驱动层

mod block;
mod bus;
mod chardev;
mod gpu;
mod input;
mod plic;

pub use self::{
    block::{BLOCK_DEVICE, DEV_IO_MODE, IOMode},
    chardev::SERIAL,
    gpu::GPU_DEVICE,
    input::{KEYBOARD_DEVICE, MOUSE_DEVICE},
    plic::{init_device, irq_handler},
};
