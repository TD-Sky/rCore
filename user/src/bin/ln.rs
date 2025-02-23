#![no_std]
#![no_main]

use user::fs::link;

#[unsafe(no_mangle)]
fn main(argc: usize, argv: &[&str]) -> i32 {
    assert_eq!(argc, 3);
    link(argv[1], argv[2]).expect("The linked file not found");
    0
}
