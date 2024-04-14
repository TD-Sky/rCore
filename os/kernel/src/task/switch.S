# 用当前寄存器的值更新当前任务上下文，并读取下一任务的上下文之值到当前寄存器内。
#
# 任务切换就是两个不同应用在内核中的Trap控制流之间的切换，
# 例如从 应用A 切换到 应用B：
# 应用A首先陷入操作系统，然后会出现两种情况：B还没开始，B正在进行。
# 无论哪种，都是先进入Trap上下文，然后再跳到用户模式。
.altmacro

.macro SAVE_SN n
    sd s\n, (\n+2)*8(a0)
.endm

.macro LOAD_SN n
    ld s\n, (\n+2)*8(a1)
.endm

    .section .text
    .globl __switch
__switch:
    # save kernel stack of current task
    sd sp, 8(a0)
    # save ra
    sd ra, 0(a0)
    # save s0~s11 of current execution
    # saved registers: s0~s11 == x9 & x18~x27
    .set n, 0
    .rept 12
        SAVE_SN %n
        .set n, n + 1
    .endr
    # restore ra & s0~s11 of next execution
    # __switch与普通函数的本质差异是**换栈**
    ld ra, 0(a1)
    .set n, 0
    .rept 12
        LOAD_SN %n
        .set n, n + 1
    .endr
    # 将下一个任务的内核栈地址放到sp
    ld sp, 8(a1)
    # ret 只做了 pc = x[ra]，没有副作用
    ret
