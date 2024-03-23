#![no_std]
#![no_main]
#![feature(format_args_nl)]

#[macro_use]
extern crate user;
use user::fs::{close, open, OpenFlag};
use user::io::write;
use user::time::get_time;

#[no_mangle]
fn main() -> i32 {
    let mut buffer = [0u8; 1024]; // 1KiB
    for (i, ch) in buffer.iter_mut().enumerate() {
        *ch = i as u8;
    }
    let fd = open("testf", OpenFlag::CREATE | OpenFlag::WRONLY).unwrap();
    let start = get_time();

    let size_mb = 1usize;
    for _ in 0..1024 * size_mb {
        write(fd, &buffer).unwrap();
    }

    close(fd).unwrap();

    let time_ms = (get_time() - start) as usize;
    let speed_kbs = (size_mb * 1024) / (time_ms / 1000);
    println!(
        "{}MiB written, time cost = {}ms, write speed = {}KiB/s",
        size_mb, time_ms, speed_kbs
    );

    0
}
