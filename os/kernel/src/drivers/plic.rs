//! Platform-Level Interrupt Controller 平台中断控制器
//!
//! Reference
//! - [Spec](https://github.com/riscv/riscv-plic-spec/blob/master/riscv-plic.adoc)

use riscv::register::sie;

use super::{BLOCK_DEVICE, KEYBOARD_DEVICE, MOUSE_DEVICE, SERIAL};
use crate::board::{
    IrqId, MemMapEntity, PLIC_CONTEXT_BASE, PLIC_CONTEXT_STRIDE, PLIC_ENABLE_BASE,
    PLIC_ENABLE_STRIDE, irq_ids,
};

pub fn init_device() {
    let mut plic = PLIC::new(MemMapEntity::PLIC.addr);
    let hart_id = 0;

    plic.set_threshold(hart_id, InterruptTargetPriority::Supervisor, 0);
    plic.set_threshold(hart_id, InterruptTargetPriority::Machine, 1);

    for source_id in irq_ids().map(|id| id.0 as usize) {
        plic.enable(hart_id, InterruptTargetPriority::Supervisor, source_id);
        plic.set_priority(source_id, 1);
    }

    unsafe {
        // 允许S级外部中断
        sie::set_sext();
    }
}

pub fn irq_handler() {
    let mut plic = PLIC::new(MemMapEntity::PLIC.addr);
    let hart_id = 0;

    let source_id = plic.claim(hart_id, InterruptTargetPriority::Supervisor);
    match IrqId(source_id) {
        IrqId::KEYBOARD => KEYBOARD_DEVICE.handle_irq(),
        IrqId::MOUSE => MOUSE_DEVICE.handle_irq(),
        IrqId::BLOCK => BLOCK_DEVICE.handle_irq(),
        IrqId::SERIAL => SERIAL.handle_irq(),
        _ => panic!("Unsupported IRQ {source_id}"),
    }
    plic.complete(hart_id, InterruptTargetPriority::Supervisor, source_id);
}

#[allow(clippy::upper_case_acronyms)]
struct PLIC {
    base_addr: usize,
}

struct InterruptTarget<'a> {
    plic: &'a PLIC,
    hart_id: usize,
    priority: InterruptTargetPriority,
}

#[derive(Clone, Copy)]
enum InterruptTargetPriority {
    Machine = 0,
    Supervisor = 1,
}

impl InterruptTargetPriority {
    const AVAILABLE_MODES: usize = 2;
}

impl PLIC {
    const fn new(base_addr: usize) -> Self {
        Self { base_addr }
    }

    const fn priority(&self, source_id: usize) -> *mut u32 {
        assert!(0 < source_id && source_id <= 132);
        (self.base_addr + source_id * 4) as *mut u32
    }

    /// 优先级为0意味着不中断
    fn set_priority(&mut self, intr_source_id: usize, priority: u32) {
        assert!(priority < 8);
        unsafe {
            self.priority(intr_source_id).write_volatile(priority);
        }
    }

    fn enable(&mut self, hart_id: usize, priority: InterruptTargetPriority, source_id: usize) {
        let reg = InterruptTarget::new(self, hart_id, priority).enable(source_id);
        let shift = source_id % 32;
        unsafe {
            reg.write_volatile(reg.read_volatile() | 1 << shift);
        }
    }

    /// PLIC会遮蔽所有小于等于预设优先级的中断
    fn set_threshold(&mut self, hart_id: usize, priority: InterruptTargetPriority, threshold: u32) {
        assert!(threshold < 8);
        let reg = InterruptTarget::new(self, hart_id, priority).threshold();
        unsafe {
            reg.write_volatile(threshold);
        }
    }

    /// 返回优先级更高的中断。同等优先级下，中断号更小者优先。
    fn claim(&mut self, hart_id: usize, priority: InterruptTargetPriority) -> u32 {
        let reg = InterruptTarget::new(self, hart_id, priority).claim_complete();
        unsafe { reg.read_volatile() }
    }

    fn complete(&mut self, hart_id: usize, priority: InterruptTargetPriority, completion: u32) {
        let reg = InterruptTarget::new(self, hart_id, priority).claim_complete();
        unsafe {
            reg.write_volatile(completion);
        }
    }
}

impl<'a> InterruptTarget<'a> {
    const fn new(plic: &'a PLIC, hart_id: usize, priority: InterruptTargetPriority) -> Self {
        Self {
            plic,
            hart_id,
            priority,
        }
    }

    /// PLIC规范里没有对中断优先级参与编号的说明，因此通过该方法计算出上下文ID
    const fn id(&self) -> usize {
        self.hart_id * InterruptTargetPriority::AVAILABLE_MODES + self.priority as usize
    }

    const fn enable(&self, source_id: usize) -> *mut u32 {
        let reg_id = source_id / 32;
        (self.plic.base_addr + PLIC_ENABLE_BASE + self.id() * PLIC_ENABLE_STRIDE + 0x4 * reg_id)
            as *mut u32
    }

    const fn threshold(&self) -> *mut u32 {
        (self.plic.base_addr + PLIC_CONTEXT_BASE + self.id() * PLIC_CONTEXT_STRIDE) as *mut u32
    }

    const fn claim_complete(&self) -> *mut u32 {
        (self.plic.base_addr + PLIC_CONTEXT_BASE + 0x4 + self.id() * PLIC_CONTEXT_STRIDE)
            as *mut u32
    }
}
