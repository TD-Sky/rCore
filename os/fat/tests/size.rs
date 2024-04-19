use std::mem;

use fat::volume::{data::DirEntry, reserved::Bpb};

#[test]
fn bpb() {
    assert_eq!(512, mem::size_of::<Bpb>());
}

#[test]
fn dir_entry() {
    assert_eq!(32, mem::size_of::<DirEntry>());
}
