# Building
TARGET := riscv64gc-unknown-none-elf
MODE := release
KERNEL_ELF := target/$(TARGET)/$(MODE)/os
KERNEL_BIN := $(KERNEL_ELF).bin

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

run: build
	@qemu-system-riscv64 \
		-machine virt \
		-nographic \
		-bios $(BOOTLOADER) \
		-device loader,file=$(KERNEL_BIN),addr=$(KERNEL_ENTRY_PA)

# -s 可以使 Qemu 监听本地 TCP 端口 1234 等待 GDB 客户端连接；
# -S 可以使 Qemu 在收到 GDB 的请求后再开始运行。
gdb-server: build
	@qemu-system-riscv64 \
		-machine virt \
		-nographic \
		-bios $(BOOTLOADER) \
		-device loader,file=$(KERNEL_BIN),addr=$(KERNEL_ENTRY_PA) \
		-s -S

gdb-client:
	@riscv64-elf-gdb \
		-ex 'file $(KERNEL_ELF)' \
		-ex 'set arch riscv:rv64' \
		-ex 'target remote localhost:1234'

build: $(KERNEL_BIN)

# 我们直接将内核可执行文件 os 提交给 Qemu ，
# 而 Qemu 会将整个可执行文件不加处理的加载到 Qemu 内存的 0x80200000 处，
# 由于内核可执行文件的开头是一段元数据，
# 这会导致 Qemu 内存 0x80200000 处无法找到内核第一条指令，
# 也就意味着 RustSBI 无法正常将计算机控制权转交给内核。
# 相反，图示的下半部分中，
# 将元数据丢弃得到的内核镜像 os.bin 被加载到 Qemu 之后，
# 则可以在 0x80200000 处正确找到内核第一条指令。
$(KERNEL_BIN): kernel
	@$(OBJCOPY) $(KERNEL_ELF) --strip-all -O binary $(KERNEL_BIN)

kernel:
	@cd ../user && make build
	@echo Platfrom: $(BOARD)
	@cp src/linker-$(BOARD).ld src/linker.ld
	@cargo build $(MODE_ARG)
	@rm src/linker.ld

clean:
	@cargo clean

.PHONY: build kernel clean run gdb-server gdb-client
