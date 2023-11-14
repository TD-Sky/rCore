#[cfg(test)]
mod tests;

use std::fs::File;
use std::io::{Read, Write};
use std::io::{Seek, SeekFrom};
use std::sync::Mutex;

use easy_fs::BlockDevice;
use easy_fs::BLOCK_SIZE;

pub struct BlockFile(pub Mutex<File>);

impl BlockDevice for BlockFile {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        let mut file = self.0.lock().unwrap();
        file.seek(SeekFrom::Start((block_id * BLOCK_SIZE) as u64))
            .expect("seeking error");
        assert_eq!(file.read(buf).unwrap(), BLOCK_SIZE, "not a complete block!");
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        let mut file = self.0.lock().unwrap();
        file.seek(SeekFrom::Start((block_id * BLOCK_SIZE) as u64))
            .expect("seeking error");
        assert_eq!(
            file.write(buf).unwrap(),
            BLOCK_SIZE,
            "not a complete block!"
        );
    }

    fn handle_irq(&self) {
        unimplemented!()
    }
}
