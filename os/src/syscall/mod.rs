mod fs;
mod process;

use easy_fs::Stat;

use self::fs::*;
use self::process::*;

const UNLINKAT: usize = 35;
const LINKAT: usize = 37;
const OPEN: usize = 56;
const CLOSE: usize = 57;
const READ: usize = 63;
const WRITE: usize = 64;
const FSTAT: usize = 80;
const EXIT: usize = 93;
const YIELD: usize = 124;
const TIME: usize = 169;
const GETPID: usize = 172;
const SBRK: usize = 214;
const MUNMAP: usize = 215;
const FORK: usize = 220;
const EXEC: usize = 221;
const MMAP: usize = 222;
const WAITPID: usize = 260;
const SPAWN: usize = 400;

pub fn syscall(id: usize, args: [usize; 3]) -> isize {
    match id {
        UNLINKAT => sys_unlinkat(args[0] as *const u8),
        LINKAT => sys_linkat(args[0] as *const u8, args[1] as *const u8),
        OPEN => sys_open(args[0] as *const u8, args[1] as u32),
        CLOSE => sys_close(args[0]),
        READ => sys_read(args[0], args[1] as *const u8, args[2]),
        WRITE => sys_write(args[0], args[1] as *const u8, args[2]),
        FSTAT => sys_fstat(args[0], args[1] as *mut Stat),
        EXIT => sys_exit(args[0] as i32),
        YIELD => sys_yield(),
        TIME => sys_get_time(),
        GETPID => sys_getpid(),
        SBRK => sys_sbrk(args[0] as i32),
        MUNMAP => sys_munmap(args[0], args[1]),
        FORK => sys_fork(),
        EXEC => sys_exec(args[0] as *const u8),
        MMAP => sys_mmap(args[0], args[1], args[2] as u8),
        WAITPID => sys_waitpid(args[0] as isize, args[1] as *mut i32),
        SPAWN => sys_spawn(args[0] as *const u8),
        _ => panic!("Unsupported syscall_id: {}", id),
    }
}
