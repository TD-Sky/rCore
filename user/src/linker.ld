OUTPUT_ARCH(riscv)
ENTRY(_start)

/* 与空地址 0x0 相隔一段距离 */
BASE_ADDRESS = 0x10000;

/* 是否按页对齐，取决于前后段的权限是否一致 */
SECTIONS
{
    . = BASE_ADDRESS;

    .text : {
        *(.text.entry)
        *(.text .text.*)
    }
    . = ALIGN(4K);

    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }
    . = ALIGN(4K);

    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }
    .bss : {
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }

    /DISCARD/ : {
        *(.eh_frame)
        *(.debug*)
    }
}
