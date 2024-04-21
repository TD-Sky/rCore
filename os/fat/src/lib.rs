#![no_std]

extern crate alloc;

mod control;
mod sector;
pub mod volume;

pub use self::control::FatFileSystem;
