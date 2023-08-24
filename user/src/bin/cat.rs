#![no_std]
#![no_main]

extern crate alloc;

#[macro_use]
extern crate user;
use user::fs::{close, open, OpenFlag};
use user::io::read;

#[no_mangle]
pub fn main(argc: usize, argv: &[&str]) -> i32 {
    assert!(argc == 2);
    let fd = open(argv[1], OpenFlag::read_only()).expect("Error occured when opening file");
    let fd = fd as usize;
    let mut buf = [0u8; 256];
    loop {
        let size = read(fd, &mut buf).unwrap();
        if size == 0 {
            break;
        }
        print!("{}", core::str::from_utf8(&buf[..size]).unwrap());
    }
    close(fd);
    0
}
