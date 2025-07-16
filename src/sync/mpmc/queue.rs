use std::sync::Arc;

use crossbeam_queue::{ArrayQueue, SegQueue};

#[derive(Clone)]
#[repr(transparent)]
pub struct BoundedReceiver<T> {
    queue: Arc<ArrayQueue<T>>,
}

impl<T> BoundedReceiver<T> {
    #[inline]
    pub fn try_recv(&self) -> Option<T> {
        self.queue.pop()
    }

    pub fn try_iter<'a>(&'a self) -> impl Iterator<Item = T> + 'a {
        struct TryIter<'a, T> {
            queue: &'a ArrayQueue<T>,
        }
        impl<'a, T> Iterator for TryIter<'a, T> {
            type Item = T;

            #[inline]
            fn next(&mut self) -> Option<T> {
                self.queue.pop()
            }
        }
        TryIter { queue: &self.queue }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.queue.capacity()
    }
}

#[derive(Clone)]
#[repr(transparent)]
pub struct BoundedSender<T> {
    queue: Arc<ArrayQueue<T>>,
}

impl<T> BoundedSender<T> {
    #[inline]
    pub fn send(&self, value: T) -> Result<(), T> {
        self.queue.push(value)
    }

    /// if the queue is full, the oldest element is replaced and returned.
    #[inline]
    pub fn force_send(&self, value: T) -> Option<T> {
        self.queue.force_push(value)
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.queue.capacity()
    }
}

#[derive(Clone)]
#[repr(transparent)]
pub struct UnboundedReceiver<T> {
    queue: Arc<SegQueue<T>>,
}

impl<T> UnboundedReceiver<T> {
    #[inline]
    pub fn try_recv(&self) -> Option<T> {
        self.queue.pop()
    }

    pub fn try_iter<'a>(&'a self) -> impl Iterator<Item = T> + 'a {
        struct TryIter<'a, T> {
            queue: &'a SegQueue<T>,
        }
        impl<'a, T> Iterator for TryIter<'a, T> {
            type Item = T;

            #[inline]
            fn next(&mut self) -> Option<T> {
                self.queue.pop()
            }
        }
        TryIter { queue: &self.queue }
    }
}

#[derive(Clone)]
#[repr(transparent)]
pub struct UnboundedSender<T> {
    queue: Arc<SegQueue<T>>,
}

impl<T> UnboundedSender<T> {
    #[inline]
    pub fn send(&self, value: T) {
        self.queue.push(value)
    }
}

/// concurrent queue channel with bounded capacity.
pub fn bounded<T: Send>(capacity: usize) -> (BoundedSender<T>, BoundedReceiver<T>) {
    let queue = Arc::new(ArrayQueue::new(capacity));
    let sender = BoundedSender {
        queue: queue.clone(),
    };
    let receiver = BoundedReceiver { queue };
    (sender, receiver)
}

/// concurrent queue channel with unbounded capacity.
pub fn unbounded<T: Send>() -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    let queue = Arc::new(SegQueue::new());
    let sender = UnboundedSender {
        queue: queue.clone(),
    };
    let receiver = UnboundedReceiver { queue };
    (sender, receiver)
}
