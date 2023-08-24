use core::arch::asm;

// Stack
//                    .
//                    .
//       +->          .
//       |   +-----------------+   |
//       |   | return address  |   |
//       |   |   previous fp ------+
//       |   | saved registers |
//       |   | local variables |
//       |   |       ...       | <-+
//       |   +-----------------+   |
//       |   | return address  |   |
//       +------ previous fp   |   |
//           | saved registers |   |
//           | local variables |   |
//       +-> |       ...       |   |
//       |   +-----------------+   |
//       |   | return address  |   |
//       |   |   previous fp ------+
//       |   | saved registers |
//       |   | local variables |
//       |   |       ...       | <-+
//       |   +-----------------+   |
//       |   | return address  |   |
//       +------ previous fp   |   |
//           | saved registers |   |
//           | local variables |   |
//   $fp --> |       ...       |   |
//           +-----------------+   |
//           | return address  |   |
//           |   previous fp ------+
//           | saved registers |
//   $sp --> | local variables |
//           +-----------------+
#[allow(dead_code)]
pub unsafe fn print_stack_trace() {
    let mut fp: *const usize;
    asm!("mv {}, fp", out(reg) fp);

    println!("== Begin stack trace ==");
    while !fp.is_null() {
        // RISC-V 调用函数是通过 jalr 指令，
        // ra 即 jalr 的下一条指令之地址
        let saved_ra = *fp.sub(1); // 往下获取保存的 ra
        let pre_fp = *fp.sub(2); // 往下获取上上次调用前最后一帧之地址

        println!("0x{:016x}, fp = 0x{:016x}", saved_ra, pre_fp);

        fp = pre_fp as *const usize;
    }
    println!("== End stack trace ==");
}
