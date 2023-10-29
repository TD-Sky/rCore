#![no_std]
#![no_main]
#![feature(format_args_nl)]

#[macro_use]
extern crate user;

use enumflags2::BitFlags;
use user::fs::{close, open, OpenFlag};
use user::io::{read, write};

#[no_mangle]
fn main() -> i32 {
    let test_str = "Hello, world!";
    let filea = "filea\0";
    let fd = open(filea, OpenFlag::CREATE | OpenFlag::WRONLY).unwrap();
    write(fd, test_str.as_bytes()).unwrap();
    close(fd).unwrap();

    let fd = open(filea, BitFlags::from_bits_truncate(OpenFlag::RDONLY)).unwrap();
    let mut buffer = [0u8; 128];
    let read_len = read(fd, &mut buffer).unwrap();
    close(fd).unwrap();

    assert_eq!(test_str, core::str::from_utf8(&buffer[..read_len]).unwrap());
    println!("file_test passed!");
    0
}
