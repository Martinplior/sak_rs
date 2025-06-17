use std::{
    cell::{Cell, UnsafeCell},
    ops::{Deref, DerefMut},
};

pub struct MutexCellGuard<'a, T: ?Sized + 'a> {
    lock: &'a MutexCell<T>,
}

impl<T: ?Sized> Deref for MutexCellGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.value.get() }
    }
}

impl<T: ?Sized> DerefMut for MutexCellGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.value.get() }
    }
}

impl<T: ?Sized> Drop for MutexCellGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        self.lock.is_locked.set(false);
    }
}

impl<T: ?Sized + std::fmt::Debug> std::fmt::Debug for MutexCellGuard<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + std::fmt::Display> std::fmt::Display for MutexCellGuard<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        (**self).fmt(f)
    }
}

/// A replacement of `RefCell` that has mutex restriction and possible smaller size
pub struct MutexCell<T: ?Sized> {
    is_locked: Cell<bool>,
    value: UnsafeCell<T>,
}

impl<T> MutexCell<T> {
    #[inline]
    pub const fn new(value: T) -> Self {
        Self {
            is_locked: Cell::new(false),
            value: UnsafeCell::new(value),
        }
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<T: ?Sized> MutexCell<T> {
    #[inline]
    pub fn lock(&self) -> Option<MutexCellGuard<'_, T>> {
        (!self.is_locked.get()).then(|| {
            self.is_locked.set(true);
            MutexCellGuard { lock: self }
        })
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.value.get_mut()
    }

    #[inline]
    pub fn is_locked(&self) -> bool {
        self.is_locked.get()
    }
}

impl<T> From<T> for MutexCell<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T: Default> Default for MutexCell<T> {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T: ?Sized + std::fmt::Debug> std::fmt::Debug for MutexCell<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("MutexCell");
        d.field("is_locked", &self.is_locked.get());
        match self.lock() {
            Some(guard) => d.field("data", &&*guard),
            None => d.field("data", &format_args!("<locked>")),
        };
        d.finish()
    }
}

#[cfg(test)]
mod test {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn t1() {
        let a = MutexCell::new(42);
        println!("{:?}", a);
        a.lock().map(|mut guard| {
            println!("{:?}", a);
            *guard = 4242;
        });
        println!("{:?}", a);
    }
}
