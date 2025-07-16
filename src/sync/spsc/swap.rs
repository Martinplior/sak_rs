use std::sync::Arc;

use parking_lot::{Condvar, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
pub enum RecvError {
    #[error("Swap channel is disconnected")]
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
pub enum TryRecvError {
    #[error("Swap channel is disconnected")]
    Disconnected,
    #[error("Swap channel is empty")]
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
pub enum SendError<T> {
    #[error("Swap channel is disconnected")]
    Disconnected(T),
}

struct SwapInner<T> {
    buf: Mutex<Vec<T>>,
    condvar: Condvar,
}

pub struct SwapReceiver<T> {
    inner: Arc<SwapInner<T>>,
    buf: Vec<T>,
}

#[inline]
fn swap_disconnected<T>(inner: &Arc<SwapInner<T>>) -> bool {
    Arc::strong_count(inner) == 1
}

impl<T> SwapReceiver<T> {
    pub fn recv(&mut self) -> Result<T, RecvError> {
        if let Some(x) = self.buf.pop() {
            return Ok(x);
        }
        self.swap_buf()?;
        Ok(unsafe { self.buf.pop().unwrap_unchecked() })
    }

    pub fn try_recv(&mut self) -> Result<T, TryRecvError> {
        if let Some(x) = self.buf.pop() {
            return Ok(x);
        }
        self.try_swap_buf()?;
        Ok(unsafe { self.buf.pop().unwrap_unchecked() })
    }

    pub fn try_iter<'a>(&'a mut self) -> impl Iterator<Item = T> + 'a {
        struct TryIter<'a, T> {
            receiver: &'a mut SwapReceiver<T>,
        }
        impl<'a, T> Iterator for TryIter<'a, T> {
            type Item = T;

            #[inline]
            fn next(&mut self) -> Option<Self::Item> {
                self.receiver.try_recv().ok()
            }
        }
        TryIter { receiver: self }
    }

    #[inline]
    pub fn disconnected(&self) -> bool {
        swap_disconnected(&self.inner)
    }

    /// local buffer.
    #[inline]
    pub fn buf(&self) -> &Vec<T> {
        &self.buf
    }
}

impl<T> SwapReceiver<T> {
    /// block if empty.
    ///
    /// if returns `Ok`, `self.buf` is not empty.
    fn swap_buf(&mut self) -> Result<(), RecvError> {
        if self.disconnected() {
            return Err(RecvError::Disconnected);
        }
        {
            let mut buf = self.inner.buf.lock();
            self.inner
                .condvar
                .wait_while(&mut buf, |buf| buf.is_empty());
            core::mem::swap(&mut self.buf, &mut *buf);
        }
        Ok(())
    }

    /// if returns `Ok`, `self.buf` is not empty.
    fn try_swap_buf(&mut self) -> Result<(), TryRecvError> {
        if self.disconnected() {
            return Err(TryRecvError::Disconnected);
        }
        {
            let mut buf = self.inner.buf.lock();
            if buf.is_empty() {
                return Err(TryRecvError::Empty);
            }
            core::mem::swap(&mut self.buf, &mut *buf);
        }
        Ok(())
    }
}

pub struct SwapSender<T> {
    inner: Arc<SwapInner<T>>,
}

impl<T> SwapSender<T> {
    pub fn send(&self, value: T) -> Result<(), SendError<T>> {
        if self.disconnected() {
            return Err(SendError::Disconnected(value));
        }
        {
            let mut buf = self.inner.buf.lock();
            buf.push(value);
            self.inner.condvar.notify_one();
        }
        Ok(())
    }

    #[inline]
    pub fn disconnected(&self) -> bool {
        swap_disconnected(&self.inner)
    }
}

pub fn swap<T: Send>(capacity: usize) -> (SwapSender<T>, SwapReceiver<T>) {
    let inner = Arc::new(SwapInner {
        buf: Mutex::new(Vec::with_capacity(capacity)),
        condvar: Condvar::new(),
    });
    let sender = SwapSender {
        inner: inner.clone(),
    };
    let receiver = SwapReceiver {
        inner,
        buf: Vec::with_capacity(capacity),
    };
    (sender, receiver)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swap() {
        let (sender, mut receiver) = swap(10);
        assert_eq!(sender.disconnected(), false);
        assert_eq!(receiver.disconnected(), false);
        assert_eq!(sender.send(10), Ok(()));
        assert_eq!(receiver.recv(), Ok(10));
        assert_eq!(sender.send(20), Ok(()));
        assert_eq!(receiver.try_recv(), Ok(20));
        assert_eq!(receiver.try_recv(), Err(TryRecvError::Empty));
        drop(sender);
        assert_eq!(receiver.recv(), Err(RecvError::Disconnected));
        assert_eq!(receiver.try_recv(), Err(TryRecvError::Disconnected));
        assert_eq!(receiver.disconnected(), true);
    }
}
