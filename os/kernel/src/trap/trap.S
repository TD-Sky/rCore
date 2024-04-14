# 在内核完成各子系统初始化后，跳转至用户态前，sscratch被设置成内核栈地址

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

    # 选择 .text.trampoline 节，在开头放置
    .section .text.trampoline
    .globl __alltraps
    .globl __restore
    .globl __alltraps_k
    .globl __restore_k

    # 对齐到 2 的 n 次方，此处乃 2^2
    .align 2
__alltraps:
    # sscratch：Supervisor Scratch, 指向额外的内存的暂存空间。
    csrrw sp, sscratch, sp
    # 交换后：sp -> 内核栈；sscratch -> 用户栈
    # 保存通用寄存器
    ## 保存 ra
    sd x1, 1*8(sp)
    ## 跳过 sp 即 x2, 待会再保存
    sd x3, 3*8(sp)
    ## 跳过 tp 即 x4, 不需要
    ## 保存 x5~x31
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
    # load kernel_satp into t0
    ld t0, 34*8(sp)
    # load trap_handler into t1
    ld t1, 36*8(sp)
    # move to kernel_sp
    ld sp, 35*8(sp)
    # switch to kernel space
    csrw satp, t0
    sfence.vma
    # 编译时：Trap引导与Trap处理者同在 .text 段；
    # 运行时：Trap引导在高256G，Trap处理者在低256G；
    # 因此，不能通过call增加PC计数调用Trap处理者
    jr t1

__restore:
    # a0: 用户空间的Trap上下文地址，全体应用统一
    # a1: 用户空间的页表
    # 切换到用户空间
    csrw satp, a1
    sfence.vma
    csrw sscratch, a0
    mv sp, a0
    # 若是从 __switch 进来，sp已经设成将执行任务的内核栈了
    #
    # sp -> 分配好 Trap 环境的内核栈栈顶
    # 恢复 sstatus 与 sepc
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    csrw sstatus, t0
    csrw sepc, t1
    # 恢复除 sp 和 tp 以外的通用寄存器
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr
    # 回到用户栈
    ld sp, 2*8(sp)
    sret

    .align 2
__alltraps_k:
    addi sp, sp, -34*8
    sd x1, 1*8(sp)
    sd x3, 3*8(sp)
    .set n, 5
    .rept 27
        SAVE_GP %n
        .set n, n+1
    .endr
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 32*8(sp)
    sd t1, 33*8(sp)
    mv a0, sp
    csrr t2, sscratch
    jalr t2

__restore_k:
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    csrw sstatus, t0
    csrw sepc, t1
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr
    addi sp, sp, 34*8
    sret
