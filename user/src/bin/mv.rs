#![no_std]
#![no_main]

extern crate alloc;

#[macro_use]
extern crate user;
use user::fs::{close, open, rename, OpenFlag};
use user::io::read;

#[no_mangle]
fn main(argc: usize, argv: &[&str]) -> i32 {
    assert!(argc == 3);
    rename(argv[1], argv[2]).unwrap();
    0
}
