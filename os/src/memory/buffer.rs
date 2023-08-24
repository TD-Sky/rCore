use alloc::vec::Vec;

pub struct Buffer(Vec<&'static mut [u8]>);

impl Buffer {
    #[inline]
    pub fn new(raw: Vec<&'static mut [u8]>) -> Self {
        Self(raw)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.iter().map(|bs| bs.len()).sum()
    }
}

impl AsRef<[&'static mut [u8]]> for Buffer {
    fn as_ref(&self) -> &[&'static mut [u8]] {
        self.0.as_slice()
    }
}

impl AsMut<[&'static mut [u8]]> for Buffer {
    fn as_mut(&mut self) -> &mut [&'static mut [u8]] {
        self.0.as_mut_slice()
    }
}
