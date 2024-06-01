use crate::syscall::*;

pub fn spin_mutex() -> usize {
    sys_mutex_create(false) as usize
}

pub fn block_mutex() -> usize {
    sys_mutex_create(true) as usize
}

pub fn mutex_lock(id: usize) -> Option<()> {
    sys_mutex_lock(id).some()
}

pub fn mutex_unlock(id: usize) -> Option<()> {
    sys_mutex_unlock(id).some()
}

pub fn semaphore_create(permits: usize) -> usize {
    sys_semaphore_create(permits) as usize
}

pub fn semaphore_up(id: usize) -> Option<()> {
    sys_semaphore_up(id).some()
}

pub fn semaphore_down(id: usize) -> Option<()> {
    sys_semaphore_down(id).some()
}

pub fn condvar_create() -> usize {
    sys_condvar_create() as usize
}

pub fn condvar_signal(id: usize) -> Option<()> {
    sys_condvar_signal(id).some()
}

pub fn condvar_wait(id: usize, mutex_id: usize) -> Option<()> {
    sys_condvar_wait(id, mutex_id).some()
}
