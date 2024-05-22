#![no_std]
#![feature(step_trait)]

extern crate alloc;

mod cluster;
mod control;
mod sector;
mod vfs;
pub mod volume;

pub use self::{
    cluster::{ClusterError, ClusterId},
    control::FatFileSystem,
    sector::SectorId,
    vfs::Inode,
};
