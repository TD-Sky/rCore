#[allow(clippy::upper_case_acronyms, non_camel_case_types)]
mod ns16550a;

use alloc::boxed::Box;
use spin::Lazy;

use self::ns16550a::NS16550a;
use crate::board::MemMapEntity;

const VIRT_UART0: usize = MemMapEntity::UART0.addr;
type CharDeviceImpl = NS16550a<VIRT_UART0>;

pub static SERIAL: Lazy<Box<dyn CharDevice>> = Lazy::new(|| Box::new(CharDeviceImpl::new()));

pub trait CharDevice: Send + Sync {
    fn init(&self);
    fn read(&self) -> u8;
    fn write(&self, ch: u8);
    fn is_empty(&self) -> bool;
    fn handle_irq(&self);
}
