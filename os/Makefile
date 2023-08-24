# Building
TARGET := riscv64gc-unknown-none-elf
MODE := release
KERNEL_ELF := target/$(TARGET)/$(MODE)/os
FS_IMG := ../easy-fs-fuse/target/fs.img

ifeq ($(MODE), release)
	MODE_ARG := --release
endif

# Board
BOARD := qemu
SBI ?= rustsbi
BOOTLOADER := ../bootloader/$(SBI)-$(BOARD).bin

# Kernel entry
KERNEL_ENTRY_PA := 0x80200000

# Bin-utils
OBJDUMP := rust-objdump --arch-name=riscv64
OBJCOPY := rust-objcopy --binary-architecture=riscv64

QEMU_ARGS := -machine virt \
			 -nographic \
			 -bios $(BOOTLOADER) \
			 -device loader,file=$(KERNEL_ELF),addr=$(KERNEL_ENTRY_PA) \
			 -drive file=$(FS_IMG),if=none,format=raw,id=x0 \
			 -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0
			 # 将硬盘 x0 作为一个 VirtIO 总线中的一个块设备接入到虚拟机系统中，
			 # virtio-mmio-bus.0 表示 VirtIO 总线通过 MMIO 进行控制，且该块设备在总线中的编号为0

run: build pack-fs-img
	@qemu-system-riscv64 $(QEMU_ARGS)

# -s 可以使 Qemu 监听本地 TCP 端口 1234 等待 GDB 客户端连接；
# -S 可以使 Qemu 在收到 GDB 的请求后再开始运行。
gdb-server: build
	@qemu-system-riscv64 $(QEMU_ARGS) -s -S

gdb-client:
	@riscv64-elf-gdb \
		-ex 'file $(KERNEL_ELF)' \
		-ex 'set arch riscv:rv64' \
		-ex 'target remote localhost:1234'

build: kernel

kernel:
	@cd ../user && cargo build --release
	@echo Platfrom: $(BOARD)
	@cp src/linker-$(BOARD).ld src/linker.ld
	@cargo build $(MODE_ARG)
	@rm src/linker.ld

pack-fs-img:
	@rm -rf $(FS_IMG)
	@cd ../easy-fs-fuse && \
		cargo run -r \
			--bin easy-fs-packer \
			-- \
			-s ../user/src/bin \
			-t ../user/target/riscv64gc-unknown-none-elf/release \
			-O ./target

clean:
	@cargo clean
	@cd ../user && cargo clean
	@cd ../easy-fs-fuse && cargo clean

.PHONY: build kernel clean run gdb-server gdb-client pack-fs-img
