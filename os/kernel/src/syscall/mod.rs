mod fs;
mod graph;
mod input;
mod process;
mod sync;
mod thread;
mod time;

use self::{fs::*, graph::*, input::*, process::*, sync::*, thread::*, time::*};

const READ: usize = 0;
const WRITE: usize = 1;
const OPEN: usize = 2;
const CLOSE: usize = 3;
const FSTAT: usize = 5;
const PIPE: usize = 22;
const DUP: usize = 32;
const GETPID: usize = 39;
const FORK: usize = 57;
const EXIT: usize = 60;
const KILL: usize = 62;
const GETDENTS: usize = 78;
const GETCWD: usize = 79;
const CHDIR: usize = 80;
const RENAME: usize = 82;
const MKDIR: usize = 83;
const RMDIR: usize = 84;
const LINK: usize = 86;
const UNLINK: usize = 87;
const SLEEP: usize = 101;
const YIELD: usize = 124;
const SIGACTION: usize = 134;
const SIGPROCMASK: usize = 135;
const SIGRETURN: usize = 139;
const GET_TIME: usize = 169;
const GETTID: usize = 186;
const SBRK: usize = 214;
const MUNMAP: usize = 215;
const EXEC: usize = 221;
const MMAP: usize = 222;
const WAITPID: usize = 260;
const EVENTFD: usize = 290;
const SPAWN: usize = 400;
const SPAWN_THREAD: usize = 1000;
const WAITTID: usize = 1002;
const MUTEX_CREATE: usize = 1010;
const MUTEX_LOCK: usize = 1011;
const MUTEX_UNLOCK: usize = 1012;
const SEMAPHORE_CREATE: usize = 1020;
const SEMAPHORE_UP: usize = 1021;
const SEMAPHORE_DOWN: usize = 1022;
const CONDVAR_CREATE: usize = 1030;
const CONDVAR_SIGNAL: usize = 1031;
const CONDVAR_WAIT: usize = 1032;
const FRAMEBUFFER: usize = 2000;
const FRAMEBUFFER_FLUSH: usize = 2001;
const GET_EVENT: usize = 3000;
const KEY_PRESSED: usize = 3001;

pub fn syscall(id: usize, args: [usize; 3]) -> isize {
    match id {
        READ => sys_read(args[0], args[1] as _, args[2]),
        WRITE => sys_write(args[0], args[1] as _, args[2]),
        OPEN => sys_open(args[0] as _, args[1] as u32),
        CLOSE => sys_close(args[0]),
        FSTAT => sys_fstat(args[0], args[1] as _),
        PIPE => sys_pipe(args[0] as _),
        DUP => sys_dup(args[0]),
        GETPID => sys_getpid(),
        FORK => sys_fork(),
        EXIT => sys_exit(args[0] as i32),
        KILL => sys_kill(args[0], args[1] as u32),
        GETDENTS => sys_getdents(args[0], args[1] as _, args[2]),
        GETCWD => sys_getcwd(args[0] as _, args[1]),
        CHDIR => sys_chdir(args[0] as _),
        RENAME => sys_rename(args[0] as _, args[1] as _),
        MKDIR => sys_mkdir(args[0] as _),
        RMDIR => sys_rmdir(args[0] as _),
        LINK => sys_link(args[0] as _, args[1] as _),
        UNLINK => sys_unlink(args[0] as _),
        SLEEP => sys_sleep(args[0]),
        YIELD => sys_yield(),
        SIGACTION => sys_sigaction(args[0] as u32, args[1] as _, args[2] as _),
        SIGPROCMASK => sys_sigprocmask(args[0] as u32),
        SIGRETURN => sys_sigreturn(),
        GET_TIME => sys_get_time(),
        GETTID => sys_gettid(),
        SBRK => sys_sbrk(args[0] as i32),
        MUNMAP => sys_munmap(args[0], args[1]),
        EXEC => sys_exec(args[0] as _, args[1] as _),
        MMAP => sys_mmap(args[0], args[1], args[2] as u8),
        WAITPID => sys_waitpid(args[0] as isize, args[1] as _),
        SPAWN => sys_spawn(args[0] as _),
        SPAWN_THREAD => sys_spawn_thread(args[0], args[1]),
        WAITTID => sys_waittid(args[0]),
        EVENTFD => sys_eventfd(args[0] as u64, args[1] as u32),
        MUTEX_CREATE => sys_mutex_create(args[0] == 1),
        MUTEX_LOCK => sys_mutex_lock(args[0]),
        MUTEX_UNLOCK => sys_mutex_unlock(args[0]),
        SEMAPHORE_CREATE => sys_semaphore_create(args[0]),
        SEMAPHORE_UP => sys_semaphore_up(args[0]),
        SEMAPHORE_DOWN => sys_semaphore_down(args[0]),
        CONDVAR_CREATE => sys_condvar_create(),
        CONDVAR_SIGNAL => sys_condvar_signal(args[0]),
        CONDVAR_WAIT => sys_condvar_wait(args[0], args[1]),
        FRAMEBUFFER => sys_framebuffer(),
        FRAMEBUFFER_FLUSH => sys_framebuffer_flush(),
        GET_EVENT => sys_get_event(),
        KEY_PRESSED => sys_key_pressed(),
        _ => panic!("Unsupported syscall ID: {id}"),
    }
}
