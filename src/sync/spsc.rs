use std::{sync::Arc, thread::Thread};

use parking_lot::Mutex;

#[derive(Debug)]
pub struct OnceReceiver<T> {
    inner: Arc<OnceInner<T>>,
}

impl<T> OnceReceiver<T> {
    /// Receive the one-time value. This function blocks until the value is available.
    pub fn recv(self) -> T {
        loop {
            let mut guard = self.inner.value_and_thread.lock();
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
        let value = self.inner.value_and_thread.lock().0.take();
        value.ok_or(self)
    }
}

#[derive(Debug)]
pub struct OnceSender<T> {
    inner: Arc<OnceInner<T>>,
}

impl<T> OnceSender<T> {
    /// Send the one-time value.
    pub fn send(self, value: T) {
        let mut guard = self.inner.value_and_thread.lock();
        let (inner_value, thread) = &mut *guard;
        *inner_value = Some(value);
        let thread = thread.take();
        drop(guard);
        thread.map(|thread| thread.unpark());
    }
}

#[derive(Debug)]
struct OnceInner<T> {
    value_and_thread: Mutex<(Option<T>, Option<Thread>)>,
}

/// channel for one-time usage
pub fn once<T>() -> (OnceSender<T>, OnceReceiver<T>) {
    let inner = Arc::new(OnceInner {
        value_and_thread: Mutex::new((None, None)),
    });
    let sender = OnceSender {
        inner: inner.clone(),
    };
    let receiver = OnceReceiver { inner };
    (sender, receiver)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn t1() {
        let (sender, receiver) = once::<String>();
        println!("hello");
        let th = std::thread::spawn(move || {
            println!("sleep");
            std::thread::sleep(Duration::from_secs(5));
            println!("send");
            sender.send("world".into());
        });
        let value = receiver.recv();
        println!("{}", value);
        th.join().unwrap();
    }
}
