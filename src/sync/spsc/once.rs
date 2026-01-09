use std::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    ptr::NonNull,
    sync::atomic::{self, AtomicU8},
    thread::Thread,
    time::Instant,
};

use parking_lot::{RawMutex, lock_api::RawMutex as _};

pub struct OnceInner<T> {
    lock: RawMutex,
    value: UnsafeCell<Option<T>>,
    thread: UnsafeCell<Option<Thread>>,

    state: AtomicU8,
}

impl<T> OnceInner<T> {
    const CONNECTION_BIT: u8 = 0b1;
    const INPLACE_BIT: u8 = 0b10;

    fn new(inplace: bool) -> Self {
        Self {
            lock: RawMutex::INIT,
            value: UnsafeCell::new(None),
            thread: UnsafeCell::new(None),
            state: AtomicU8::new(if inplace { Self::INPLACE_BIT } else { 0 }),
        }
    }

    /// similar to [`std::sync::Arc::drop`]
    #[inline]
    unsafe fn drop(inner: NonNull<Self>) {
        let old_state = unsafe { inner.as_ref() }
            .state
            .fetch_or(Self::CONNECTION_BIT, atomic::Ordering::Release);
        let only_one_connection = (old_state & Self::CONNECTION_BIT) != 0;
        if !only_one_connection {
            return;
        }
        atomic::fence(atomic::Ordering::Acquire);
        let inplace = (old_state & Self::INPLACE_BIT) != 0;
        if inplace {
            unsafe { inner.drop_in_place() };
        } else {
            let _ = unsafe { Box::from_raw(inner.as_ptr()) };
        }
    }
}

#[derive(Debug)]
pub struct OnceReceiver<T> {
    inner: NonNull<OnceInner<T>>,
}

impl<T> OnceReceiver<T> {
    /// Receive the one-time value. This function blocks until the value is available.
    pub fn recv(self) -> T {
        loop {
            let inner = unsafe { self.inner.as_ref() };
            inner.lock.lock();
            let value = unsafe { &mut *inner.value.get() }.take();
            if let Some(value) = value {
                return value;
            }
            unsafe {
                *inner.thread.get() = Some(std::thread::current());
                inner.lock.unlock();
            }
            std::thread::park();
        }
    }

    /// Try to receive the one-time value. This function returns `Ok(value)` if the value is
    /// available, or `Err(self)` if the value is not available yet.
    pub fn try_recv(self) -> Result<T, Self> {
        let inner = unsafe { self.inner.as_ref() };
        if !inner.lock.try_lock() {
            return Err(self);
        }
        let value = unsafe { &mut *inner.value.get() }.take();
        if let Some(value) = value {
            Ok(value)
        } else {
            unsafe { inner.lock.unlock() };
            Err(self)
        }
    }

    /// Try to receive the one-time value with timeout. This function returns `Ok(value)` if the
    /// value is available, or `Err(self)` if the value is not available within the specified
    /// `timeout`.
    pub fn try_recv_timeout(self, timeout: std::time::Duration) -> Result<T, Self> {
        let begin_instant = Instant::now();
        loop {
            let inner = unsafe { self.inner.as_ref() };
            if inner.lock.try_lock() {
                let value = unsafe { &mut *inner.value.get() }.take();
                if let Some(value) = value {
                    return Ok(value);
                }
                unsafe {
                    *inner.thread.get() = Some(std::thread::current());
                    inner.lock.unlock();
                }
            }
            let elapsed = begin_instant.elapsed();
            if elapsed >= timeout {
                return Err(self);
            }
            let remaining = timeout - elapsed;
            std::thread::park_timeout(remaining);
        }
    }

    /// Receive the one-time value inplace.
    pub fn try_recv_inplace(this: &mut Option<Self>) -> Option<T> {
        let Some(receiver) = this.take() else {
            return None;
        };
        match receiver.try_recv() {
            Ok(r) => Some(r),
            Err(err) => {
                *this = Some(err);
                None
            }
        }
    }

    /// Receive the one-time value inplace with timeout.
    pub fn try_recv_timeont_inplace(
        this: &mut Option<Self>,
        timeout: std::time::Duration,
    ) -> Option<T> {
        let Some(receiver) = this.take() else {
            return None;
        };
        match receiver.try_recv_timeout(timeout) {
            Ok(r) => Some(r),
            Err(err) => {
                *this = Some(err);
                None
            }
        }
    }
}

impl<T> Drop for OnceReceiver<T> {
    fn drop(&mut self) {
        unsafe { OnceInner::drop(self.inner) };
    }
}

unsafe impl<T: Send> Send for OnceReceiver<T> {}

#[derive(Debug)]
pub struct OnceSender<T> {
    inner: NonNull<OnceInner<T>>,
}

impl<T> OnceSender<T> {
    /// Send the one-time value.
    pub fn send(self, value: T) {
        let inner = unsafe { self.inner.as_ref() };
        inner.lock.lock();
        unsafe { *inner.value.get() = Some(value) };
        let thread = unsafe { &mut *inner.thread.get() }.take();
        unsafe { inner.lock.unlock() };
        thread.map(|thread| thread.unpark());
    }
}

impl<T> Drop for OnceSender<T> {
    fn drop(&mut self) {
        unsafe { OnceInner::drop(self.inner) };
    }
}

unsafe impl<T: Send> Send for OnceSender<T> {}

/// channel for one-time usage
pub fn once<T: Send>() -> (OnceSender<T>, OnceReceiver<T>) {
    let inner = NonNull::from(Box::leak(Box::new(OnceInner::new(false))));
    (OnceSender { inner }, OnceReceiver { inner })
}

/// channel for one-time usage with inplace allocation.
///
/// caller should ensure that `*inner` is uninitialized before calling this function.
pub fn once_inplace<T: Send>(
    inner: &'static mut MaybeUninit<OnceInner<T>>,
) -> (OnceSender<T>, OnceReceiver<T>) {
    let inner = NonNull::from(inner.write(OnceInner::new(true)));
    (OnceSender { inner }, OnceReceiver { inner })
}

/// channel for one-time usage with inplace allocation.
///
/// caller should ensure that `*inner` is uninitialized before calling this function.
///
/// # Safety
///
/// caller **must** ensure that `*inner` lives longer than the returned `OnceSender` and
/// `OnceReceiver`.
///
/// see also: [`once_inplace`]
pub unsafe fn once_inplace_unchecked<T: Send + 'static>(
    inner: &mut MaybeUninit<OnceInner<T>>,
) -> (OnceSender<T>, OnceReceiver<T>) {
    unsafe {
        once_inplace(core::mem::transmute::<
            &mut MaybeUninit<OnceInner<T>>,
            &'static mut MaybeUninit<OnceInner<T>>,
        >(inner))
    }
}

#[cfg(test)]
mod tests {
    use std::{marker::PhantomData, time::Duration};

    use super::*;

    #[test]
    fn t1() {
        let (sender, receiver) = once::<String>();
        println!("hello");
        let th = std::thread::spawn(move || {
            println!("sleep");
            std::thread::sleep(Duration::from_secs(3));
            println!("send");
            sender.send("world".into());
        });
        let value = receiver.recv();
        println!("{}", value);
        th.join().unwrap();
    }

    #[test]
    fn t2() {
        struct Bar(PhantomData<*const ()>);
        impl Bar {
            fn new() -> Self {
                Self(PhantomData)
            }
        }
        unsafe impl Send for Bar {}
        let (sender, receiver) = once::<Bar>();
        std::thread::spawn(move || sender.send(Bar::new()));
        receiver.recv();
    }

    #[test]
    fn timeout() {
        let (sender, receiver) = once::<String>();
        let receiver = receiver
            .try_recv_timeout(Duration::from_millis(1000))
            .unwrap_err();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(1000));
            sender.send("world".into());
        });
        receiver.try_recv_timeout(Duration::from_secs(10)).unwrap();
    }

    #[test]
    fn inplace() {
        let mut inner = MaybeUninit::uninit();
        let (sender, receiver) = unsafe { once_inplace_unchecked(&mut inner) };
        sender.send(10);
        assert_eq!(receiver.recv(), 10);
    }
}
