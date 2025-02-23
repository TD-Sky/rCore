pub mod address;
mod address_space;
mod buffer;
pub mod frame_allocator;
mod heap_allocator;
mod kernel_stack;
mod page_table;

pub use self::{
    address_space::{AddressSpace, KERNEL_SPACE, MapPermission},
    buffer::{UserBuffer, write_any},
    kernel_stack::{KernelStack, alloc_kernel_stack, kernel_token},
    page_table::{PageTable, read_mut, read_ref, read_str, write_str},
};

pub fn init() {
    heap_allocator::init();
    frame_allocator::init();
    KERNEL_SPACE.exclusive_access().activate();
}
