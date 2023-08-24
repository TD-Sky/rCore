use crate::status2option;
use crate::syscall::*;

pub fn read(fd: usize, buf: &mut [u8]) -> Option<usize> {
    status2option(sys_read(fd, buf))
}

pub fn write(fd: usize, buf: &[u8]) -> Option<usize> {
    status2option(sys_write(fd, buf))
}
