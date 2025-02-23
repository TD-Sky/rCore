#![no_std]
#![no_main]
#![feature(format_args_nl)]

use user::println;
use user::thread::sleep;
use user::time::get_time;

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("into sleep test!");
    let start = get_time();
    println!("current time_msec = {}", start);
    sleep(100);
    let end = get_time();
    println!(
        "time_msec = {} after sleeping 100 ticks, delta = {}ms!",
        end,
        end - start
    );
    println!("r_sleep passed!");
    0
}
