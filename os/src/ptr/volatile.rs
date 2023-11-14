/// An MMIO register which can only be read from.
#[repr(transparent)]
pub struct ReadOnly<T>(T);

/// An MMIO register which can only be written to.
#[repr(transparent)]
pub struct WriteOnly<T: Copy>(T);

/// An MMIO register which may be both read and written.
#[repr(transparent)]
pub struct Volatile<T: Copy>(T);

impl<T: Copy> ReadOnly<T> {
    /// volatile read
    pub fn vread(&self) -> T {
        unsafe { core::ptr::read_volatile(&self.0) }
    }
}

impl<T: Copy> WriteOnly<T> {
    /// volatile write
    pub fn vwrite(&mut self, value: T) {
        unsafe {
            core::ptr::write_volatile(&mut self.0, value);
        }
    }
}

impl<T: Copy> Volatile<T> {
    #[allow(dead_code)]
    pub fn read(&self) -> T {
        unsafe { core::ptr::read_volatile(&self.0) }
    }

    pub fn write(&mut self, value: T) {
        unsafe {
            core::ptr::write_volatile(&mut self.0, value);
        }
    }
}
