use std::mem;

use fat::volume::{
    data::DirEntry,
    reserved::{Bpb, FsInfo},
};

#[test]
fn volume() {
    assert_eq!(512, mem::size_of::<Bpb>());
    assert_eq!(512, mem::size_of::<FsInfo>());
    assert_eq!(32, mem::size_of::<DirEntry>());
}
