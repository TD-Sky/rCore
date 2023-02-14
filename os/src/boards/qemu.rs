use qemu_exit::RISCV64;

const VIRT_TEST: u64 = 0x100000;

pub const QEMU_EXIT_HANDLE: RISCV64 = RISCV64::new(VIRT_TEST);
