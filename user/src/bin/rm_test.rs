#![no_std]
#![no_main]

use user::remove;

#[no_mangle]
pub fn main() -> i32 {
    remove("filea\0").unwrap();
    0
}
