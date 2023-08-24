use core::fmt;
use core::fmt::Write;

use crate::io::{read, write};

const STDIN: usize = 0;
const STDOUT: usize = 1;

struct Stdout;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write(STDOUT, s.as_bytes()).unwrap();
        Ok(())
    }
}

pub fn print(args: fmt::Arguments) {
    Stdout.write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::console::print(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {
        $crate::console::print(format_args_nl!($($arg)*))
    };
}

pub fn getchar() -> u8 {
    let mut c = [0; 1];
    read(STDIN, &mut c).unwrap();
    c[0]
}
