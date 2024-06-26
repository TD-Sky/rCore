#![no_std]
#![feature(step_trait)]

extern crate alloc;

mod cluster;
mod control;
mod inode;
mod sector;
mod volume;

pub use self::{
    cluster::{ClusterError, ClusterId},
    control::FatFileSystem,
    inode::{Inode, ROOT},
    sector::SectorId,
};
