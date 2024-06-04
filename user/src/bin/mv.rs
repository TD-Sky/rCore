#![no_std]
#![no_main]

extern crate alloc;

extern crate user;
use user::fs::rename;

#[no_mangle]
fn main(argc: usize, argv: &[&str]) -> i32 {
    assert!(argc == 3);
    rename(argv[1], argv[2]).unwrap();
    0
}
