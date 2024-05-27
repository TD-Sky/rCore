#![no_std]
#![no_main]
#![feature(never_type)]
#![feature(format_args_nl)]

extern crate alloc;
use alloc::string::String;

#[macro_use]
extern crate user;
use alloc::vec::Vec;
use user::console::getchar;
use user::fs::*;
use user::process::{exec, fork, waitpid};

const CTRL_D: u8 = 0x04;
/// line feed
const LF: u8 = 0x0a;
/// carriage return
const CR: u8 = 0x0d;
/// backspace
const DL: u8 = 0x7f;
const BS: u8 = 0x08;
const LINE_START: &str = ">> ";

#[no_mangle]
fn main() -> i32 {
    println!("Rust user shell");
    let mut line = String::new();
    print!("{LINE_START}");

    loop {
        let c = getchar();

        match c {
            LF | CR => {
                println!("");

                'block: {
                    if line.is_empty() {
                        break 'block;
                    }

                    let process_args_list: Vec<_> =
                        line.as_str().split('|').map(ProcessArgs::new).collect();

                    if !commands_are_valid(&process_args_list) {
                        println!("Invalid command(s): Input/Output cannot be correctly binded!");
                        break 'block;
                    }

                    let pipes: Vec<[usize; 2]> = (!process_args_list.is_empty())
                        .then(|| {
                            // 管道放在各命令中间，所以是n-1个；
                            // 注意，如果只有一条命令，那么管道数组将为空
                            (0..process_args_list.len() - 1)
                                .map(|_| {
                                    let mut pipe_fd = [0usize; 2];
                                    pipe(&mut pipe_fd);
                                    pipe_fd
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    let mut children = Vec::with_capacity(process_args_list.len() - 1);
                    let end = process_args_list.len() - 1;
                    for (i, process_args) in process_args_list.iter().enumerate() {
                        let pid = fork();
                        if pid != 0 {
                            children.push(pid);
                            continue;
                        }

                        /* 子进程 */
                        if let Err(e) = sub_process(i, process_args, &pipes, end) {
                            return e;
                        }
                        /* 子进程exec */
                    }

                    for pipe in &pipes {
                        close(pipe[0]).unwrap();
                        close(pipe[1]).unwrap();
                    }

                    let mut exit_code = 0;
                    for pid in children {
                        let exit_pid = waitpid(pid, &mut exit_code);
                        assert_eq!(exit_pid, Some(pid));
                        println!("Shell: Process {} exited with code {}", pid, exit_code);
                    }

                    line.clear();
                }

                print!("{LINE_START}");
            }
            BS | DL => {
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

type Pipe = [usize; 2];

struct ProcessArgs {
    input: Option<String>,
    output: Option<String>,
    args: Vec<String>,
}

impl ProcessArgs {
    fn new(command: &str) -> Self {
        let mut args: Vec<String> = command
            .split_whitespace()
            .filter(|arg| !arg.is_empty())
            .map(String::from)
            .collect();

        let input = args.iter().position(|arg| arg == "<").map(|i| {
            let input = args[i + 1].clone();
            // 带上重定向符后的文件名
            args.drain(i..=i + 1);
            input
        });

        let output = args.iter().position(|arg| arg == ">").map(|i| {
            let output = args[i + 1].clone();
            args.drain(i..=i + 1);
            output
        });

        Self {
            input,
            output,
            args,
        }
    }
}

fn commands_are_valid(list: &[ProcessArgs]) -> bool {
    if list.len() == 1 {
        return true;
    }

    let end = list.len() - 1;
    for (i, process_args) in list.iter().enumerate() {
        // 多组命令情况下，第一条不能重定向输出
        if i == 0 {
            if process_args.output.is_some() {
                return false;
            }
        }
        // 多组命令情况下，最后一条不能重定向输入
        else if i == end {
            if process_args.input.is_some() {
                return false;
            }
        }
        // 多组命令情况下，中间的命令不能重定向
        else if process_args.output.is_some() || process_args.input.is_some() {
            return false;
        }
    }

    true
}

fn sub_process(i: usize, process_args: &ProcessArgs, pipes: &[Pipe], end: usize) -> Result<!, i32> {
    // 重定向输入
    if let Some(input) = &process_args.input {
        let Some(input_fd) = open(input, OpenFlag::read_only()) else {
            println!("Error when opening file {input}");
            return Err(-4);
        };
        // 关掉标准输入
        close(0).unwrap();
        // 替换标准输入为文件
        assert_eq!(dup(input_fd), Some(0));
        close(input_fd).unwrap();
    }

    // 重定向输出
    if let Some(output) = &process_args.output {
        let Some(output_fd) = open(output, OpenFlag::CREATE | OpenFlag::WRONLY) else {
            println!("Error when opening file {output}");
            return Err(-4);
        };
        // 关掉标准输出
        close(1).unwrap();
        // 替换标准输出为文件
        assert_eq!(dup(output_fd), Some(1));
        close(output_fd).unwrap();
    }

    // 从管道读取前一进程的输出作为输入
    if i > 0 {
        close(0).unwrap();
        let read_end = pipes[i - 1][0];
        assert_eq!(dup(read_end), Some(0));
    }

    // 输出至管道作为下一进程的输入
    // (仅执行一条命令时，`i == end == 0`，故这里不会执行)
    if i < end {
        close(1).unwrap();
        let write_end = pipes[i][1];
        assert_eq!(dup(write_end), Some(1));
    }

    // 关闭所有管道，它们继承自父进程
    for pipe in pipes {
        close(pipe[0]).unwrap();
        close(pipe[1]).unwrap();
    }

    if exec(&process_args.args[0], &process_args.args).is_none() {
        println!("Error when executing!");
        return Err(-4);
    }
    unreachable!()
}
