use alloc::boxed::Box;
use alloc::collections::VecDeque;

use spin::Lazy;

use virtio_drivers::VirtIOHeader;
use virtio_drivers::VirtIOInput;

use super::bus::VirtioHal;
use crate::board::IrqId;
use crate::sync::{Condvar, UpCell};
use crate::task::processor;

pub static KEYBOARD_DEVICE: Lazy<Box<dyn InputDevice>> =
    Lazy::new(|| Box::new(VirtIOInputWrapper::new(IrqId::KEYBOARD.virtio_mmio_addr())));

pub static MOUSE_DEVICE: Lazy<Box<dyn InputDevice>> =
    Lazy::new(|| Box::new(VirtIOInputWrapper::new(IrqId::MOUSE.virtio_mmio_addr())));

pub trait InputDevice: Send + Sync {
    fn is_empty(&self) -> bool;
    fn read_event(&self) -> u64;
    fn handle_irq(&self);
}

struct VirtIOInputWrapper {
    inner: UpCell<VirtIOInputInner>,
    condvar: Condvar,
}

struct VirtIOInputInner {
    base: VirtIOInput<'static, VirtioHal>,
    events: VecDeque<u64>,
}

impl VirtIOInputWrapper {
    fn new(addr: usize) -> Self {
        Self {
            inner: UpCell::new(VirtIOInputInner {
                base: VirtIOInput::new(unsafe { &mut *(addr as *mut VirtIOHeader) }).unwrap(),
                events: VecDeque::new(),
            }),
            condvar: Condvar::new(),
        }
    }
}

impl InputDevice for VirtIOInputWrapper {
    fn is_empty(&self) -> bool {
        self.inner.exclusive_access().events.is_empty()
    }

    fn read_event(&self) -> u64 {
        loop {
            let mut inner = self.inner.exclusive_access();
            if let Some(event) = inner.events.pop_front() {
                break event;
            } else {
                let task_ctx_ptr = self.condvar.wait();
                drop(inner);
                processor::schedule(task_ctx_ptr);
            }
        }
    }

    fn handle_irq(&self) {
        let mut count = 0;
        let mut result = 0;
        self.inner.exclusive_session(|inner| {
            inner.base.ack_interrupt();
            while let Some((_, event)) = inner.base.pop_pending_event() {
                count += 1;
                result = (event.event_type as u64) << 48
                    | (event.code as u64) << 32
                    | event.value as u64;
                inner.events.push_back(result);
            }
        });
        if count > 0 {
            self.condvar.signal();
        }
    }
}
