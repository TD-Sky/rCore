#![no_std]
#![no_main]

extern crate alloc;
use alloc::string::String;

#[macro_use]
extern crate user;
use user::console::getchar;
use user::{exec, fork, waitpid};

const CTRL_D: u8 = 0x04;
/// line feed
const LF: u8 = 0x0a;
/// carriage return
const CR: u8 = 0x0d;
/// backspace
const BS: u8 = 0x08;

#[no_mangle]
fn main() -> i32 {
    println!("Rust user shell");
    let mut line = String::new();
    print!(">> ");

    loop {
        let c = getchar();

        match c {
            LF | CR => {
                println!("");

                if !line.is_empty() {
                    line.push('\0');
                    let pid = fork().unwrap();

                    if pid == 0 {
                        // 找不到程序，退出 shell
                        if exec(line.as_str()).is_none() {
                            println!("Error when executing!");
                            return -4;
                        }
                    } else {
                        let mut exit_code = 0;
                        let exit_pid = waitpid(pid, &mut exit_code).unwrap();
                        assert_eq!(pid, exit_pid);
                        println!("Shell: Process {} exited with code {}", pid, exit_code);
                    }

                    line.clear();
                }

                print!(">> ");
            }
            BS => {
                if !line.is_empty() {
                    print!("{} {}", BS as char, BS as char);
                    line.pop();
                }
            }
            CTRL_D => {
                if line.is_empty() {
                    break 0;
                }
            }
            _ => {
                // echo
                print!("{}", c as char);
                line.push(c as char);
            }
        }
    }
}
