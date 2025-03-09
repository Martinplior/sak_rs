use std::sync::Arc;

use parking_lot::{Condvar, Mutex};

#[derive(Debug)]
pub struct OnceReceiver<T> {
    inner: Arc<OnceInner<T>>,
}

impl<T> OnceReceiver<T> {
    /// Receive the one-time value. This function blocks until the value is available.
    pub fn recv(self) -> T {
        let mut guard = self.inner.value.lock();
        self.inner
            .condvar
            .wait_while(&mut guard, |value| value.is_none());
        // Safety: `value` is guaranteed to be `Some` at this point.
        unsafe { guard.take().unwrap_unchecked() }
    }

    /// Try to receive the one-time value. This function returns `Ok(value)` if the value is
    /// available, or `Err(self)` if the value is not available yet.
    pub fn try_recv(self) -> Result<T, Self> {
        let value = self.inner.value.lock().take();
        value.ok_or_else(|| self)
    }
}

#[derive(Debug)]
pub struct OnceSender<T> {
    inner: Arc<OnceInner<T>>,
}

impl<T> OnceSender<T> {
    /// Send the one-time value.
    pub fn send(self, value: T) {
        *self.inner.value.lock() = Some(value);
        self.inner.condvar.notify_all();
    }
}

#[derive(Debug)]
struct OnceInner<T> {
    value: Mutex<Option<T>>,
    condvar: Condvar,
}

/// channel for one-time usage
pub fn once<T>() -> (OnceSender<T>, OnceReceiver<T>) {
    let inner = Arc::new(OnceInner {
        value: Mutex::new(None),
        condvar: Condvar::new(),
    });
    let sender = OnceSender {
        inner: inner.clone(),
    };
    let receiver = OnceReceiver { inner };
    (sender, receiver)
}
