#![no_std]
#![feature(step_trait)]

extern crate alloc;

mod cluster;
mod control;
mod inode;
mod sector;
pub mod volume;

pub use self::{
    cluster::{ClusterError, ClusterId},
    control::FatFileSystem,
    inode::Inode,
    sector::SectorId,
};
