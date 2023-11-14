use core::convert::Infallible;
use core::slice;

use crate::syscall::{sys_framebuffer, sys_framebuffer_flush, sys_get_event, sys_key_pressed};
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::OriginDimensions,
    pixelcolor::{Rgb888, RgbColor},
    prelude::Size,
};
use virtio_input_decoder::{DecodeType, Decoder};

pub const RESOLUTION_X: u32 = 1280;
pub const RESOLUTION_Y: u32 = 800;
const FRAMEBUFFER_LEN: usize = (RESOLUTION_X * RESOLUTION_Y * 4) as usize;

pub fn get_event() -> Option<u64> {
    let event = sys_get_event() as u64;
    (event > 0).then_some(event)
}

pub fn key_pressed() -> bool {
    sys_key_pressed() != 0
}

pub struct Display {
    size: Size,
    framebuffer: &'static mut [u8],
}

fn framebuffer() -> &'static mut [u8] {
    let ptr = sys_framebuffer() as usize as *mut u8;
    unsafe { slice::from_raw_parts_mut(ptr, FRAMEBUFFER_LEN) }
}

fn flush_framebuffer() {
    sys_framebuffer_flush();
}

impl Display {
    pub fn new(size: Size) -> Self {
        Self {
            size,
            framebuffer: framebuffer(),
        }
    }

    pub fn framebuffer(&mut self) -> &mut [u8] {
        self.framebuffer
    }

    pub fn paint<F>(&mut self, f: F)
    where
        F: FnOnce(&mut [u8]),
    {
        f(self.framebuffer);
        flush_framebuffer()
    }
}

impl OriginDimensions for Display {
    fn size(&self) -> Size {
        self.size
    }
}

impl DrawTarget for Display {
    type Color = Rgb888;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::prelude::Pixel<Self::Color>>,
    {
        for pixel in pixels {
            let i = (pixel.0.y * RESOLUTION_X as i32 + pixel.0.x) as usize * 4;
            if i + 2 >= self.framebuffer.len() {
                break;
            }
            self.framebuffer[i] = pixel.1.b();
            self.framebuffer[i + 1] = pixel.1.g();
            self.framebuffer[i + 2] = pixel.1.r();
        }
        flush_framebuffer();

        Ok(())
    }
}

#[repr(C)]
pub struct InputEvent {
    pub event_type: u16,
    pub code: u16,
    pub value: u32,
}

impl From<u64> for InputEvent {
    fn from(mut v: u64) -> Self {
        let value = v as u32;
        v >>= 32;
        let code = v as u16;
        v >>= 16;
        let event_type = v as u16;

        Self {
            event_type,
            code,
            value,
        }
    }
}

impl InputEvent {
    pub fn decode(&self) -> Option<DecodeType> {
        let Self {
            event_type,
            code,
            value,
        } = self;
        Decoder::decode(*event_type as usize, *code as usize, *value as usize).ok()
    }
}
