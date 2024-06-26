use crate::syscall::*;

pub fn read(fd: usize, buf: &mut [u8]) -> Option<usize> {
    sys_read(fd, buf).status()
}

pub fn write(fd: usize, buf: &[u8]) -> Option<usize> {
    sys_write(fd, buf).status()
}
