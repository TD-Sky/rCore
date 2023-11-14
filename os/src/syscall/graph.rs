use crate::config::FRAMEBUFFER_VA;
use crate::drivers::GPU_DEVICE;
use crate::memory::address::{PhysAddr, VirtAddr};
use crate::memory::MapPermission;
use crate::task::processor;

pub fn sys_framebuffer() -> isize {
    let fb = GPU_DEVICE.framebuffer();

    let fb_start_pa = PhysAddr::from(fb.as_ptr() as usize);
    assert!(fb_start_pa.is_aligned());
    let fb_start_ppn: usize = fb_start_pa.page_number().into();
    let fb_start_vpn: usize = VirtAddr::from(FRAMEBUFFER_VA).page_number().into();
    let fb_offset = fb_start_ppn as isize - fb_start_vpn as isize;

    let process = processor::current_process();
    process
        .inner()
        .exclusive_access()
        .address_space
        .insert_linear(
            FRAMEBUFFER_VA.into(),
            fb.len(),
            fb_offset,
            MapPermission::R | MapPermission::W | MapPermission::U,
        )
        .unwrap();

    FRAMEBUFFER_VA as isize
}

pub fn sys_framebuffer_flush() -> isize {
    GPU_DEVICE.flush();
    0
}
