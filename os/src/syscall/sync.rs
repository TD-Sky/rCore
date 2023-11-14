use alloc::sync::Arc;

use crate::sync::{BlockMutex, Condvar, Mutex, Semaphore, SpinMutex};
use crate::task::processor;

pub fn sys_mutex_create(block: bool) -> isize {
    let mutex: Arc<dyn Mutex> = if block {
        Arc::new(BlockMutex::new())
    } else {
        Arc::new(SpinMutex::new())
    };
    let process = processor::current_process();
    let mut process = process.inner().exclusive_access();
    process.mutex_list.insert(mutex) as isize
}

pub fn sys_mutex_lock(id: usize) -> isize {
    let process = processor::current_process();
    let mutex = process.inner().exclusive_access().mutex_list.get(id);
    drop(process);
    mutex.lock();
    0
}

pub fn sys_mutex_unlock(id: usize) -> isize {
    let process = processor::current_process();
    let mutex = process.inner().exclusive_access().mutex_list.get(id);
    drop(process);
    mutex.unlock();
    0
}

pub fn sys_semaphore_create(permits: usize) -> isize {
    let process = processor::current_process();
    let id = process
        .inner()
        .exclusive_access()
        .semaphore_list
        .insert(Arc::new(Semaphore::new(permits)));
    id as isize
}

pub fn sys_semaphore_up(id: usize) -> isize {
    let process = processor::current_process();
    let semaphore = process.inner().exclusive_access().semaphore_list.get(id);
    drop(process);
    semaphore.up();
    0
}

pub fn sys_semaphore_down(id: usize) -> isize {
    let process = processor::current_process();
    let semaphore = process.inner().exclusive_access().semaphore_list.get(id);
    drop(process);
    semaphore.down();
    0
}

pub fn sys_condvar_create() -> isize {
    let process = processor::current_process();
    let id = process
        .inner()
        .exclusive_access()
        .condvar_list
        .insert(Arc::new(Condvar::new()));
    id as isize
}

pub fn sys_condvar_signal(id: usize) -> isize {
    let process = processor::current_process();
    let condvar = process.inner().exclusive_access().condvar_list.get(id);
    drop(process);
    condvar.signal();
    0
}

pub fn sys_condvar_wait(id: usize, mutex_id: usize) -> isize {
    let process = processor::current_process();
    let condvar = process.inner().exclusive_access().condvar_list.get(id);
    let mutex = process.inner().exclusive_access().mutex_list.get(mutex_id);
    drop(process);
    condvar.wait_with_mutex(mutex);
    0
}
