#![no_std]
#![no_main]

#[macro_use]
extern crate user;

use enumflags2::BitFlags;
use user::close;
use user::read;
use user::{open, OpenFlag};

#[no_mangle]
pub fn main() -> i32 {
    let fd = open("filea\0", BitFlags::from_bits_truncate(OpenFlag::RDONLY))
        .expect("Error occured when opening file");
    let mut buf = [0u8; 256];
    loop {
        let size = read(fd, &mut buf).unwrap();
        if size == 0 {
            break;
        }
        println!("{}", core::str::from_utf8(&buf[..size]).unwrap());
    }
    close(fd).unwrap();
    0
}
