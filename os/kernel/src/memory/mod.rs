pub mod address;
mod address_space;
mod buffer;
pub mod frame_allocator;
mod heap_allocator;
mod kernel_stack;
mod page_table;

pub use self::{
    address_space::{AddressSpace, MapPermission, KERNEL_SPACE},
    buffer::{write_any, UserBuffer},
    kernel_stack::{alloc_kernel_stack, kernel_token, KernelStack},
    page_table::{read_mut, read_ref, read_str, write_str, PageTable},
};

pub fn init() {
    heap_allocator::init();
    frame_allocator::init();
    KERNEL_SPACE.exclusive_access().activate();
}
