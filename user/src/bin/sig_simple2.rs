#![no_std]
#![no_main]
#![feature(format_args_nl)]

#[macro_use]
extern crate user;

use user::process::{exit, fork, waitpid};
use user::signal::*;
use user::thread::sleep;

fn func() {
    println!("user_sig_test passed");
    sigreturn();
}

#[no_mangle]
pub fn main() -> i32 {
    let pid = fork();
    if pid == 0 {
        let mut new = SignalAction::default();
        let mut old = SignalAction::default();
        new.handler = func as usize;

        println!("signal_simple2: child sigaction");
        sigaction(SIGUSR1, &new, &mut old).expect("Sigaction failed!");
        sleep(1000);
        println!("signal_simple2: child done");
    } else {
        println!("signal_simple2: parent kill child");
        sleep(500);
        if kill(pid, SIGUSR1).is_none() {
            println!("Kill failed!");
            exit(1);
        }
        println!("signal_simple2: parent wait child");
        let mut exit_code = 0;
        waitpid(pid, &mut exit_code);
        println!("signal_simple2: parent Done");
    }

    0
}
