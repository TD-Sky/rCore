//! References
//! - [Tutorial](https://www.lammertbies.nl/comm/info/serial-uart)
//! - [16550 cheatsheet](http://www.byterunner.com/16550.html)
//! - [ns16550a datasheet (PDF)](https://datasheetspdf.com/pdf-file/605590/NationalSemiconductor/NS16550A/1)
//! - [ns16450 datasheet (PDF)](https://datasheetspdf.com/pdf-file/1311818/NationalSemiconductor/NS16450/1)

use crate::ptr::volatile::*;
use alloc::collections::VecDeque;
use enumflags2::{bitflags, BitFlags};

use crate::{
    sync::{Condvar, UpCell},
    task::processor,
};

use super::CharDevice;

pub struct NS16550a<const BASE_ADDR: usize> {
    inner: UpCell<NS16550aInner>,
    condvar: Condvar,
}

struct NS16550aInner {
    raw: NS16550aRaw,
    read_buffer: VecDeque<u8>,
}

struct NS16550aRaw {
    base_addr: usize,
}

impl<const BASE_ADDR: usize> NS16550a<BASE_ADDR> {
    pub const fn new() -> Self {
        let inner = NS16550aInner {
            raw: NS16550aRaw {
                base_addr: BASE_ADDR,
            },
            read_buffer: VecDeque::new(),
        };

        Self {
            inner: UpCell::new(inner),
            condvar: Condvar::new(),
        }
    }
}

/// Interrupt Enable Register
#[rustfmt::skip]
#[bitflags]
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum IER {
    RX_AVAILABLE = 1 << 0,
    TX_EMPTY     = 1 << 1,
}

/// Line Status Register
#[rustfmt::skip]
#[bitflags]
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum LSR {
    DATA_AVAILABLE = 1 << 0,
    THR_EMPTY      = 1 << 5,
}

/// Model Control Register
#[rustfmt::skip]
#[bitflags]
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum MCR {
    DATA_TERMINAL_READY = 1 << 0,
    REQUEST_TO_SEND     = 1 << 1,
    AUX_OUTPUT1         = 1 << 2,
    AUX_OUTPUT2         = 1 << 3,
}

/// Read ports when DLAB = 0
#[repr(C)]
struct ReadDLAB0 {
    /// receiver buffer register
    pub rbr: ReadOnly<u8>,
    /// interrupt enable register
    pub ier: Volatile<BitFlags<IER>>,
    /// interrupt identification register
    pub iir: ReadOnly<u8>,
    /// line control register
    pub lcr: Volatile<u8>,
    /// model control register
    pub mcr: Volatile<BitFlags<MCR>>,
    /// line status register
    pub lsr: ReadOnly<BitFlags<LSR>>,
    /// ignore MSR
    _padding1: ReadOnly<u8>,
    /// ignore SCR
    _padding2: ReadOnly<u8>,
}

/// Write ports when DLAB = 0
#[repr(C)]
struct WriteDLAB0 {
    /// transmitter holding register
    pub thr: WriteOnly<u8>,
    /// interrupt enable register
    pub ier: Volatile<BitFlags<IER>>,
    /// ignore FCR
    _padding0: ReadOnly<u8>,
    /// line control register
    pub lcr: Volatile<u8>,
    /// modem control register
    pub mcr: Volatile<BitFlags<MCR>>,
    /// line status register
    pub lsr: ReadOnly<BitFlags<LSR>>,
    /// not used
    _padding1: ReadOnly<u8>,
    /// ignore SCR
    _padding2: ReadOnly<u8>,
}

impl NS16550aRaw {
    fn init(&mut self) {
        let read_end = self.read_end();

        let mcr = MCR::DATA_TERMINAL_READY | MCR::REQUEST_TO_SEND | MCR::AUX_OUTPUT2;
        read_end.mcr.write(mcr);

        // 当接收到数据时，产生中断
        let ier = BitFlags::from(IER::RX_AVAILABLE);
        read_end.ier.write(ier);
    }

    fn read_end(&mut self) -> &mut ReadDLAB0 {
        unsafe { &mut *(self.base_addr as *mut ReadDLAB0) }
    }

    fn write_end(&mut self) -> &mut WriteDLAB0 {
        unsafe { &mut *(self.base_addr as *mut WriteDLAB0) }
    }

    fn read(&mut self) -> Option<u8> {
        let read_end = self.read_end();
        read_end
            .lsr
            .vread()
            .contains(LSR::DATA_AVAILABLE)
            .then(|| read_end.rbr.vread())
    }

    fn write(&mut self, ch: u8) {
        let write_end = self.write_end();
        loop {
            if write_end.lsr.vread().contains(LSR::THR_EMPTY) {
                write_end.thr.vwrite(ch);
                break;
            }
        }
    }
}

impl<const BASE_ADDR: usize> CharDevice for NS16550a<BASE_ADDR> {
    fn init(&self) {
        self.inner.exclusive_access().raw.init();
    }

    fn read(&self) -> u8 {
        loop {
            let mut inner = self.inner.exclusive_access();
            if let Some(ch) = inner.read_buffer.pop_front() {
                break ch;
            } else {
                let task_ctx_ptr = self.condvar.wait();
                drop(inner);
                processor::schedule(task_ctx_ptr);
            }
        }
    }

    fn write(&self, ch: u8) {
        self.inner.exclusive_access().raw.write(ch);
    }

    fn is_empty(&self) -> bool {
        self.inner.exclusive_access().read_buffer.is_empty()
    }

    fn handle_irq(&self) {
        let mut count = 0;

        self.inner.exclusive_session(|inner| {
            while let Some(ch) = inner.raw.read() {
                count += 1;
                inner.read_buffer.push_back(ch);
            }
        });

        if count > 0 {
            self.condvar.signal();
        }
    }
}
