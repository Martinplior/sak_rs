use std::{
    ptr::NonNull,
    sync::atomic::{self, AtomicBool},
    thread::Thread,
};

use parking_lot::Mutex;

#[derive(Debug)]
struct OnceInner<T> {
    value_and_thread: Mutex<(Option<T>, Option<Thread>)>,
    only_one_connection: AtomicBool,
}

/// similar to [`std::sync::Arc::drop`]
#[inline]
unsafe fn once_drop<T>(inner: NonNull<OnceInner<T>>) {
    let only_one_connection = unsafe { inner.as_ref() }
        .only_one_connection
        .swap(true, atomic::Ordering::Release);
    if only_one_connection {
        atomic::fence(atomic::Ordering::Acquire);
        let _ = unsafe { Box::from_raw(inner.as_ptr()) };
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
            let mut guard = unsafe { self.inner.as_ref() }.value_and_thread.lock();
            let (value, thread) = &mut *guard;
            if let Some(value) = value.take() {
                return value;
            }
            thread
                .is_none()
                .then(|| *thread = Some(std::thread::current()));
            drop(guard);
            std::thread::park();
        }
    }

    /// Try to receive the one-time value. This function returns `Ok(value)` if the value is
    /// available, or `Err(self)` if the value is not available yet.
    pub fn try_recv(self) -> Result<T, Self> {
        let value = unsafe { self.inner.as_ref() }
            .value_and_thread
            .lock()
            .0
            .take();
        value.ok_or(self)
    }
}

impl<T> Drop for OnceReceiver<T> {
    fn drop(&mut self) {
        unsafe { once_drop(self.inner) };
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
        let mut guard = unsafe { self.inner.as_ref() }.value_and_thread.lock();
        let (inner_value, thread) = &mut *guard;
        *inner_value = Some(value);
        let thread = thread.take();
        drop(guard);
        thread.map(|thread| thread.unpark());
    }
}

impl<T> Drop for OnceSender<T> {
    fn drop(&mut self) {
        unsafe { once_drop(self.inner) };
    }
}

unsafe impl<T: Send> Send for OnceSender<T> {}

/// channel for one-time usage
pub fn once<T: Send>() -> (OnceSender<T>, OnceReceiver<T>) {
    let inner = NonNull::from(Box::leak(Box::new(OnceInner {
        value_and_thread: Mutex::new((None, None)),
        only_one_connection: false.into(),
    })));
    let sender = OnceSender { inner };
    let receiver = OnceReceiver { inner };
    (sender, receiver)
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
}
