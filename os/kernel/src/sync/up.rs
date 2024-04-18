use core::cell::UnsafeCell;
use core::cell::{RefCell, RefMut};
use core::ops::{Deref, DerefMut};

use riscv::register::sstatus;

static INTERRUPT_GUARD: SafeCell<InterruptGuard> = SafeCell::new(InterruptGuard::new());

#[derive(Debug)]
pub struct UpCell<T> {
    inner: RefCell<T>,
}
unsafe impl<T> Sync for UpCell<T> {}

// `Option`是为了在释放时可以提前销毁`RefMut`，不受启用中断的影响
pub struct UpRefMut<'a, T>(Option<RefMut<'a, T>>);

impl<T> UpCell<T> {
    pub const fn new(value: T) -> Self {
        Self {
            inner: RefCell::new(value),
        }
    }

    /// Panic if the data has been borrowed.
    pub fn exclusive_access(&self) -> UpRefMut<'_, T> {
        INTERRUPT_GUARD.get_mut().enter();
        UpRefMut(Some(self.inner.borrow_mut()))
    }

    pub fn exclusive_session<F, V>(&self, f: F) -> V
    where
        F: FnOnce(&mut T) -> V,
    {
        let mut inner = self.exclusive_access();
        f(&mut inner)
    }
}

impl<'a, T> Drop for UpRefMut<'a, T> {
    fn drop(&mut self) {
        self.0 = None;
        INTERRUPT_GUARD.get_mut().exit();
    }
}

impl<'a, T> Deref for UpRefMut<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.as_deref().unwrap()
    }
}

impl<'a, T> DerefMut for UpRefMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_deref_mut().unwrap()
    }
}

/// 中断守卫，负责中断的屏蔽与启用
struct InterruptGuard {
    nested_level: usize,
    /// 屏蔽之前的中断使能
    sie_before_shield: bool,
}

impl InterruptGuard {
    const fn new() -> Self {
        Self {
            nested_level: 0,
            sie_before_shield: false,
        }
    }

    pub fn enter(&mut self) {
        let sie = sstatus::read().sie();
        unsafe {
            sstatus::clear_sie();
        }
        if self.nested_level == 0 {
            self.sie_before_shield = sie;
        }
        self.nested_level += 1;
    }

    pub fn exit(&mut self) {
        self.nested_level -= 1;
        if self.nested_level == 0 && self.sie_before_shield {
            unsafe {
                sstatus::set_sie();
            }
        }
    }
}

struct SafeCell<T>(UnsafeCell<T>);
unsafe impl<T> Sync for SafeCell<T> {}
impl<T> SafeCell<T> {
    const fn new(value: T) -> Self {
        Self(UnsafeCell::new(value))
    }
}
impl<T> SafeCell<T> {
    #[allow(clippy::mut_from_ref)]
    fn get_mut(&self) -> &mut T {
        unsafe { &mut *self.0.get() }
    }
}
