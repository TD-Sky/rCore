# 为了使用 .rept 命令
.altmacro

.macro SAVE_GP n
    # \n = 寄存器号码
    # 算式展开：\n * 8 + sp
    sd x\n, \n*8(sp)
.endm

.macro LOAD_GP n
    ld x\n, \n*8(sp)
.endm

    .section .text
    .globl __alltraps
    .globl __restore
    # 对齐到 2 的 n 次方，此处乃 2^2
    .align 2
__alltraps:
    # sscratch：Supervisor Scratch, 指向额外的内存的暂存空间。
    # sscratch 在内核完成各子系统初始化后，跳转至用户态之前设成内核栈地址
    csrrw sp, sscratch, sp
    # 交换后：sp -> 内核栈；sscratch -> 用户栈
    addi sp, sp, -34*8
    # 保存通用寄存器
    # 保存 ra
    sd x1, 1*8(sp)
    # 跳过 sp 即 x2, 待会再保存
    sd x3, 3*8(sp)
    # 跳过 tp 即 x4, 不需要
    # 保存 x5~x31
    .set n, 5
    .rept 27
        SAVE_GP %n
        .set n, n+1
    .endr
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 32*8(sp)
    sd t1, 33*8(sp)
    # 把用户栈地址保存到内核栈指定位置
    csrr t2, sscratch
    sd t2, 2*8(sp)
    # 设置 trap_handler 的入参，a0 是函数参数寄存器
    mv a0, sp
    call trap_handler
# __alltraps 结束后就到了 __restore
__restore:
    mv sp, a0
    # sp -> 分配好 Trap 环境的内核栈栈顶
    # 恢复 sstatus 与 sepc
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    ld t2, 2*8(sp)
    csrw sstatus, t0
    csrw sepc, t1
    csrw sscratch, t2
    # sscratch -> 用户栈
    # 恢复除 sp 和 tp 以外的通用寄存器
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr
    # 释放 Trap 环境
    addi sp, sp, 34*8
    # sp -> 内核栈
    # sscratch -> 用户栈
    csrrw sp, sscratch, sp
    sret
