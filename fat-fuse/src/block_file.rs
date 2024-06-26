use std::cell::RefCell;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};

use block_dev::BlockDevice;
use send_wrapper::SendWrapper;

/// The standard sector size of a VirtIO block device. Data is read and written in multiples of this size.
const SECTOR_SIZE: usize = 512;

#[derive(Debug)]
pub struct BlockFile {
    inner: SendWrapper<RefCell<File>>,
}

impl BlockFile {
    pub fn new(fd: File) -> Self {
        Self {
            inner: SendWrapper::new(RefCell::new(fd)),
        }
    }
}

impl BlockDevice for BlockFile {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        let mut file = self.inner.borrow_mut();
        file.seek(SeekFrom::Start((block_id * SECTOR_SIZE) as u64))
            .expect("seeking error");
        assert_eq!(
            file.read(buf).unwrap(),
            SECTOR_SIZE,
            "not a complete block!"
        );
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        let mut file = self.inner.borrow_mut();
        file.seek(SeekFrom::Start((block_id * SECTOR_SIZE) as u64))
            .expect("seeking error");
        assert_eq!(
            file.write(buf).unwrap(),
            SECTOR_SIZE,
            "not a complete block!"
        );
    }

    fn handle_irq(&self) {}
}
