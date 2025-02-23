#![no_std]
#![no_main]

use embedded_graphics::prelude::Size;
use user::graph::{Display, RESOLUTION_X, RESOLUTION_Y};

#[unsafe(no_mangle)]
fn main() -> i32 {
    let mut display = Display::new(Size::new(RESOLUTION_X, RESOLUTION_Y));
    display.paint(|fb| {
        for y in 0..RESOLUTION_Y as usize {
            for x in 0..RESOLUTION_X as usize {
                let i = (y * RESOLUTION_X as usize + x) * 4;
                fb[i] = x as u8;
                fb[i + 1] = y as u8;
                fb[i + 2] = (x + y) as u8;
            }
        }
    });

    0
}
