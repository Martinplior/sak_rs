use core::mem::ManuallyDrop;

/// A cell that holds a value with an extra drop function.
#[derive(Debug, Clone)]
pub struct DropCell<T, F: FnOnce(&mut T)> {
    value: ManuallyDrop<T>,
    drop_fn: ManuallyDrop<F>,
}

impl<T, F: FnOnce(&mut T)> DropCell<T, F> {
    #[inline]
    pub fn new(value: T, drop_fn: F) -> Self {
        Self {
            value: ManuallyDrop::new(value),
            drop_fn: ManuallyDrop::new(drop_fn),
        }
    }

    #[inline]
    pub fn value(&self) -> &T {
        &self.value
    }

    #[inline]
    pub fn value_mut(&mut self) -> &mut T {
        &mut self.value
    }

    #[inline]
    pub fn drop_fn(&self) -> &F {
        &self.drop_fn
    }

    #[inline]
    pub fn drop_fn_mut(&mut self) -> &mut F {
        &mut self.drop_fn
    }
}

impl<T, F: FnOnce(&mut T)> core::ops::Deref for DropCell<T, F> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T, F: FnOnce(&mut T)> core::ops::DerefMut for DropCell<T, F> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T, F: FnOnce(&mut T)> Drop for DropCell<T, F> {
    fn drop(&mut self) {
        let mut value = unsafe { ManuallyDrop::take(&mut self.value) };
        let drop_fn = unsafe { ManuallyDrop::take(&mut self.drop_fn) };
        drop_fn(&mut value);
    }
}
