use super::File;
use crate::memory::UserBuffer;
use crate::sbi::console_getchar;
use crate::task;

/// 标准输入
#[derive(Debug)]
pub struct Stdin;

/// 标准输出
#[derive(Debug)]
pub struct Stdout;

impl File for Stdin {
    #[inline]
    fn readable(&self) -> bool {
        true
    }

    fn read(&self, mut buf: UserBuffer) -> usize {
        assert_eq!(buf.len(), 1);
        let mut c: usize;
        loop {
            c = console_getchar();
            if c == 0 {
                task::suspend_current_and_run_next();
                continue;
            } else {
                break;
            }
        }
        let ch = c as u8;
        unsafe {
            buf.as_mut()[0].as_mut_ptr().write_volatile(ch);
        }
        1
    }
}

impl File for Stdout {
    #[inline]
    fn writable(&self) -> bool {
        true
    }

    fn write(&self, buf: UserBuffer) -> usize {
        for sub_buf in buf.as_ref() {
            print!("{}", core::str::from_utf8(sub_buf).unwrap());
        }
        buf.len()
    }
}
