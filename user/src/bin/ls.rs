#![no_std]
#![no_main]
#![feature(format_args_nl)]

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;
use easy_fs::DirEntry;

use user::fs::{close, getdents, open, OpenFlag};
use user::println;

#[no_mangle]
fn main(argc: usize, _argv: &[&str]) -> i32 {
    // 截至2023-09-02，不允许指定参数
    assert_eq!(argc, 1);
    let fd = open("/\0", OpenFlag::read_only()).expect("File not found");
    let mut dents = vec![DirEntry::default(); 64];
    let nread = getdents(fd, &mut dents).expect("Not a directory");
    let s = dents[..nread]
        .iter()
        .map(|e| e.name())
        .collect::<Vec<&str>>()
        .join("\n");
    println!("{s}");
    close(fd).unwrap();

    0
}
