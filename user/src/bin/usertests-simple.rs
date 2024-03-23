#![no_std]
#![no_main]
#![feature(format_args_nl)]

use user::println;
use user::process::{exec, fork, waitpid};

static TESTS: &[&str] = &[
    "exit",
    "fantastic_text",
    "forktest",
    "forktest2",
    "forktest_simple",
    "hello_world",
    "matrix",
    "sleep",
    "sleep_simple",
    "stack_overflow",
    "yield",
];

#[no_mangle]
fn main() -> i32 {
    for test in TESTS {
        println!("Usertests: Running {test}");
        let pid = fork();
        if pid == 0 {
            exec::<&str, _>(test, []);
            panic!("unreachable!");
        } else {
            let mut exit_code: i32 = Default::default();
            let wait_pid = waitpid(pid, &mut exit_code);
            assert_eq!(Some(pid), wait_pid);
            println!(
                "\x1b[32mUsertests: Test {} in Process {} exited with code {}\x1b[0m",
                test, pid, exit_code
            );
        }
    }
    println!("Usertests passed!");
    0
}
