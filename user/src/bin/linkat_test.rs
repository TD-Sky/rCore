#![no_std]
#![no_main]

use enumflags2::BitFlags;
use user::{fstat, link_at, open, println, OpenFlag};

#[no_mangle]
pub fn main() -> i32 {
    let newpath = "linkat2\0";
    link_at("linkat_test\0", newpath).unwrap();
    let linkat2 = open(newpath, BitFlags::from_bits_truncate(OpenFlag::RDONLY)).unwrap();
    let stat = fstat(linkat2).unwrap();
    println!("{:?}", stat);
    0
}
