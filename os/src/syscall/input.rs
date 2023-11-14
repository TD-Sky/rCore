use crate::drivers::SERIAL;
use crate::drivers::{KEYBOARD_DEVICE, MOUSE_DEVICE};

pub fn sys_get_event() -> isize {
    if !KEYBOARD_DEVICE.is_empty() {
        KEYBOARD_DEVICE.read_event() as isize
    } else if !MOUSE_DEVICE.is_empty() {
        MOUSE_DEVICE.read_event() as isize
    } else {
        0
    }
}

pub fn sys_key_pressed() -> isize {
    (!SERIAL.is_empty()).into()
}
