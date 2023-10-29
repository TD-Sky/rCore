#![no_std]
#![no_main]
#![feature(format_args_nl)]

use user::process;
use user::thread::exit;
use user::signal::*;

#[macro_use]
extern crate user;

fn func() {
    println!("user_sig_test passed");
    sigreturn();
}

#[no_mangle]
fn main() -> i32 {
    let mut new = SignalAction::default();
    let mut old = SignalAction::default();
    new.handler = func as usize;

    println!("signal_simple: sigaction");
    sigaction(SIGUSR1, &new, &mut old).expect("Sigaction failed!");
    println!("signal_simple: kill");
    if kill(process::getpid(), SIGUSR1).is_none() {
        println!("Kill failed!");
        exit(1);
    }
    println!("signal_simple: Done");
    0
}
