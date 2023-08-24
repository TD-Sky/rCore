#![no_std]
#![no_main]
#![feature(format_args_nl)]

use user::fs::{fstat, open, OpenFlag};
use user::println;

#[no_mangle]
pub fn main(argc: usize, argv: &[&str]) -> i32 {
    assert_eq!(argc, 2);
    let fd = open(argv[1], OpenFlag::read_only()).expect("File not found");
    let stat = fstat(fd).unwrap();
    println!("{:?}", stat);
    0
}
