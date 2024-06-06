#![no_std]
#![no_main]
#![feature(format_args_nl)]

extern crate alloc;

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt::Write;

use user::fs::{close, getdents, open, OpenFlag};
use user::println;
use vfs::{CDirEntry, DirEntryType};

#[no_mangle]
fn main(_: usize, argv: &[&str]) -> i32 {
    let path = argv.get(1).copied().unwrap_or("/");

    let mut names = Vec::new();
    let fd = open(path, OpenFlag::read_only()).expect("Not found");

    loop {
        let mut raw_names = vec![[0u8; 256]; 8];
        let mut c_dirents: Vec<_> = raw_names
            .iter_mut()
            .map(|name| CDirEntry {
                inode: 0,
                ty: DirEntryType::Regular,
                name: name.as_mut_ptr(),
            })
            .collect();

        let n = getdents(fd, &mut c_dirents).unwrap();

        if n == 0 {
            break;
        }

        names.extend(raw_names.iter().take(n).map(|name| {
            let end = name.iter().position(|&b| b == b'\0').unwrap();
            core::str::from_utf8(&name[..end]).unwrap().to_owned()
        }))
    }

    close(fd).unwrap();

    let Some(max_len) = names.iter().map(String::len).max() else {
        return 0;
    };
    let mut buf = String::with_capacity(max_len * names.len());
    for (i, name) in names.iter().enumerate() {
        write!(buf, "{name:<max_len$}").unwrap();
        let sep = if (i + 1) % 5 == 0 { '\n' } else { ' ' };
        buf.push(sep);
    }
    println!("{}", buf.trim_end());

    0
}
