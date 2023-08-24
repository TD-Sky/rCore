use alloc::sync::{Arc, Weak};

use super::File;
use crate::memory::UserBuffer;
use crate::sync::UPSafeCell;
use crate::task;

pub struct Pipe {
    readable: bool,
    writable: bool,
    buffer: Arc<UPSafeCell<PipeRingBuffer>>,
}

#[derive(Debug, Default)]
pub struct PipeRingBuffer {
    base: [u8; PipeRingBuffer::CAP],
    head: usize,
    tail: usize,
    status: RingBufferStatus,
    write_end: Weak<Pipe>,
}

#[derive(Debug, Default, PartialEq, Eq)]
enum RingBufferStatus {
    Full,
    #[default]
    Empty,
    Normal,
}

impl File for Pipe {
    #[inline]
    fn readable(&self) -> bool {
        self.readable
    }

    #[inline]
    fn writable(&self) -> bool {
        self.writable
    }

    fn read(&self, mut buf: UserBuffer) -> usize {
        assert!(self.readable());
        let buf_len = buf.len();
        let mut buf_iter = buf.iter_mut();
        let mut read_len = 0;

        loop {
            let mut ring_buffer = self.buffer.exclusive_access();
            let readables = ring_buffer.hit_readables();

            if readables == 0 {
                if ring_buffer.write_end_closed() {
                    return read_len;
                }
                drop(ring_buffer);
                // 管道缓冲区的大小是有限的，
                // 一次可能无法满足`Buffer`的需求量
                task::suspend_current_and_run_next();
                continue;
            }

            for _ in 0..readables {
                let Some(byte) = buf_iter.next() else {
                    return read_len;
                };

                *byte = ring_buffer.pop();
                read_len += 1;
                if read_len == buf_len {
                    return buf_len;
                }
            }
        }
    }

    fn write(&self, buf: UserBuffer) -> usize {
        assert!(self.writable);
        let buf_len = buf.len();
        let mut buf_iter = buf.iter();
        let mut written_len = 0;

        loop {
            let mut ring_buffer = self.buffer.exclusive_access();
            let writables = ring_buffer.hint_writables();

            if writables == 0 {
                drop(ring_buffer);
                task::suspend_current_and_run_next();
                continue;
            }

            for _ in 0..writables {
                let Some(&byte) = buf_iter.next() else {
                    return written_len;
                };

                ring_buffer.push(byte);
                written_len += 1;

                if written_len == buf_len {
                    return written_len;
                }
            }
        }
    }

    fn stat(&self) -> easy_fs::Stat {
        unimplemented!()
    }
}

impl Pipe {
    #[inline]
    fn read_end(buffer: Arc<UPSafeCell<PipeRingBuffer>>) -> Self {
        Self {
            readable: true,
            writable: false,
            buffer,
        }
    }

    #[inline]
    fn write_end(buffer: Arc<UPSafeCell<PipeRingBuffer>>) -> Self {
        Self {
            readable: false,
            writable: true,
            buffer,
        }
    }
}

impl PipeRingBuffer {
    pub fn make_pipe() -> (Arc<Pipe>, Arc<Pipe>) {
        let buffer = Arc::new(unsafe { UPSafeCell::new(PipeRingBuffer::default()) });
        let read_end = Arc::new(Pipe::read_end(buffer.clone()));
        let write_end = Arc::new(Pipe::write_end(buffer.clone()));
        buffer.exclusive_access().write_end = Arc::downgrade(&write_end);

        (read_end, write_end)
    }
}

impl PipeRingBuffer {
    const CAP: usize = 32;

    fn hit_readables(&self) -> usize {
        if self.status == RingBufferStatus::Empty {
            0
        } else if self.tail > self.head {
            self.tail - self.head
        } else {
            self.tail + Self::CAP - self.head
        }
    }

    fn hint_writables(&self) -> usize {
        if self.status == RingBufferStatus::Full {
            0
        } else {
            Self::CAP - self.hit_readables()
        }
    }

    fn write_end_closed(&self) -> bool {
        self.write_end.strong_count() == 0
    }

    fn pop(&mut self) -> u8 {
        let byte = self.base[self.head];
        self.head = (self.head + 1) % Self::CAP;

        self.status = if self.head == self.tail {
            RingBufferStatus::Empty
        } else {
            RingBufferStatus::Normal
        };

        byte
    }

    fn push(&mut self, byte: u8) {
        self.base[self.head] = byte;
        self.tail = (self.tail + 1) % Self::CAP;

        self.status = if self.tail == self.head {
            RingBufferStatus::Full
        } else {
            RingBufferStatus::Normal
        };
    }
}
