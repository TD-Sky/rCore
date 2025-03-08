#!/usr/bin/env -S nu --stdin

plugin use nuke

alias task = nuke task

let ROOT = pwd | path join ".." ".." | path expand
const TARGET = "riscv64gc-unknown-none-elf"
const MODE = "release"
let KERNEL_ELF = $"($ROOT)/os/target/($TARGET)/($MODE)/kernel"
const FS_FUSE = "fat-fuse"
let FS_IMG = $"($ROOT)/($FS_FUSE)/target/fs.img"

const PROFILE = if $MODE == "release" { "release" } else { "dev" }

# Board
const BOARD = "qemu"
let SBI = $env.SBI? | default "rustsbi"
let BOOTLOADER = $"($ROOT)/bootloader/($SBI)-($BOARD).bin"

# GUI
let GUI = $env.GUI? | default "off"
let GUI_OPTION = if $GUI == "off" { ["-display", "none"] } else { [] }

# Kernel entry
const KERNEL_ENTRY_PA = "0x80200000"

# Binutils
const OBJDUMP = "rust-objdump --arch-name=riscv64"
const OBJCOPY = "rust-objcopy --binary-architecture=riscv64"

# 将硬盘 x0 作为一个 VirtIO 总线中的一个块设备接入到虚拟机系统中，
#
# -device virtio-net-device,netdev=net0 \
# -netdev user,id=net0,hostfwd=udp::6200-:2000,hostfwd=tcp::6201-:80
let QEMU_ARGS = [
    "-machine", "virt",
    "-bios", $BOOTLOADER,
    "-serial", "stdio",
    ...$GUI_OPTION,
    "-device", $"loader,file=($KERNEL_ELF),addr=($KERNEL_ENTRY_PA)",
    "-drive", $"file=($FS_IMG),if=none,format=raw,id=x0",
    "-device", "virtio-blk-device,drive=x0",
    "-device", "virtio-gpu-device",
    "-device", "virtio-keyboard-device",
    "-device", "virtio-mouse-device"
]

task run --deps [build, fs-img] {
    qemu-system-riscv64 ...$QEMU_ARGS
}

# -s 可以使 Qemu 监听本地 TCP 端口 1234 等待 GDB 客户端连接；
# -S 可以使 Qemu 在收到 GDB 的请求后再开始运行。
task gdb-server --deps [build, fs-img] {
    qemu-system-riscv64 ...$QEMU_ARGS -s -S
}

task gdb-client {
    riscv64-elf-gdb -ex $"file ($KERNEL_ELF)" -ex 'set arch riscv:rv64' -ex 'target remote localhost:1234'
}

task build --deps [kernel]

task kernel {
    cd $"($ROOT)/user"
    cargo build --release
    cd -
    print $"Platfrom: ($BOARD)"
    cp $"src/linker-($BOARD).ld" src/linker.ld
    cargo build $"--profile=($PROFILE)"
    rm src/linker.ld
}

task fs-img {
    rm -rf $FS_IMG
    cd $"($ROOT)/($FS_FUSE)"
    cargo run -r -- -s $"($ROOT)/user/src/bin" -t $"($ROOT)/user/target/riscv64gc-unknown-none-elf/release" -O ./target
}

task clean {
    cargo clean
    cd $"($ROOT)/user"
    cargo clean
    cd -
    cd $"($ROOT)/($FS_FUSE)"
    cargo clean
    cd -
}
