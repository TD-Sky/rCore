use std::mem;

use fat::volume::reversed::Bpb;

#[test]
fn bpb() {
    assert_eq!(512, mem::size_of::<Bpb>());
}
