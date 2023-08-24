mod address_space;
pub use address_space::AddressSpace;
pub use address_space::MapPermission;
pub use address_space::KERNEL_SPACE;

pub mod address;
pub mod kernel_stack;

pub mod frame_allocator;
mod heap_allocator;

mod page_table;
pub use page_table::PageTable;
pub use page_table::{read_mut, read_ref, read_str, write_str};

mod buffer;
pub use buffer::UserBuffer;

pub fn init() {
    heap_allocator::init();
    frame_allocator::init();
    KERNEL_SPACE.exclusive_access().activate();
}
