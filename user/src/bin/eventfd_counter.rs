#![no_std]
#![no_main]
#![feature(format_args_nl)]

use enumflags2::BitFlags;
use user::fs::{close, eventfd, eventfd_read, eventfd_write};
use user::thread::{exit, waittid};

#[macro_use]
extern crate user;
extern crate alloc;

struct WriteArg {
    fd: usize,
    seq: &'static [u64],
}

struct ReadArg {
    fd: usize,
    max: u64,
}

fn writer(arg: *const WriteArg) -> ! {
    let arg = unsafe { &*arg };
    for &i in arg.seq {
        eventfd_write(arg.fd, i).unwrap();
    }
    println!("writer over");
    exit(0);
}

fn reader(arg: *const ReadArg) {
    let arg = unsafe { &*arg };
    let mut count = 0;
    while count < arg.max {
        count += eventfd_read(arg.fd).unwrap();
    }
    println!("The sum of count is {count}");
    exit(0);
}

#[no_mangle]
fn main() -> i32 {
    let fd = eventfd(0, BitFlags::empty()).unwrap();
    let seq = &[1, 3, 7, 9, 14];
    let warg = WriteArg { fd, seq };
    let rarg = ReadArg {
        fd,
        max: seq.iter().sum(),
    };

    let threads = [
        user::thread::spawn(reader as usize, &rarg as *const _ as usize),
        user::thread::spawn(writer as usize, &warg as *const _ as usize),
    ];
    for tid in threads {
        waittid(tid);
    }
    close(fd).unwrap();
    0
}
