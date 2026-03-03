#[repr(transparent)]
pub struct VolatileCell<T: Copy>(core::cell::UnsafeCell<T>);

impl<T: Copy> VolatileCell<T> {
    #[inline(always)]
    pub const fn new(value: T) -> Self {
        Self(core::cell::UnsafeCell::new(value))
    }

    #[inline(always)]
    pub fn read(&self) -> T {
        unsafe { core::ptr::read_volatile(self.0.get()) }
    }

    #[inline(always)]
    pub fn write(&self, value: T) {
        unsafe { core::ptr::write_volatile(self.0.get(), value) }
    }

    #[inline(always)]
    pub fn as_ptr(&self) -> *mut T {
        self.0.get()
    }

    #[inline(always)]
    pub fn into_inner(self) -> T {
        self.0.into_inner()
    }
}
