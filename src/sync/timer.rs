use std::{collections::BinaryHeap as Heap, time::Instant};

pub type TimerTaskFn = dyn FnOnce(&mut TimerPool) + Send + 'static;

struct Timer {
    deadline: Instant,
    task: Box<TimerTaskFn>,
}

impl PartialEq for Timer {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline
    }
}

impl Eq for Timer {}

impl PartialOrd for Timer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Timer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.deadline.cmp(&self.deadline)
    }
}

pub struct TimerPool {
    heap: Heap<Timer>,
}

impl Default for TimerPool {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl TimerPool {
    #[inline]
    pub fn new() -> Self {
        Self { heap: Heap::new() }
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            heap: Heap::with_capacity(capacity),
        }
    }

    #[inline]
    pub fn add_task(&mut self, deadline: Instant, task: impl FnOnce(&mut Self) + Send + 'static) {
        self.add_task_boxed(deadline, Box::new(task));
    }

    #[inline]
    pub fn add_task_boxed(&mut self, deadline: Instant, task: Box<TimerTaskFn>) {
        let timer = Timer { deadline, task };
        self.heap.push(timer);
    }

    /// peek the closest dead line
    #[inline]
    pub fn peek(&self) -> Option<&Instant> {
        self.heap.peek().map(|timer| &timer.deadline)
    }

    /// poll the closest timer, returns `Some(task)` if deadline is reached.
    #[must_use]
    pub fn poll(&mut self) -> Option<Box<TimerTaskFn>> {
        let deadline = self.peek()?;
        let instant_now = Instant::now();
        if &instant_now < deadline {
            return None;
        }
        // Safety: self.peek() is `Some`
        let task = unsafe { self.heap.pop().unwrap_unchecked().task };
        Some(task)
    }

    /// if no timer arrived, call [`std::thread::sleep`] until the first timer arrived.
    #[inline]
    pub fn sleep_until_available(&self) {
        self.peek().map(|deadline| {
            let instant_now = Instant::now();
            if &instant_now < deadline {
                let duration = *deadline - instant_now;
                std::thread::sleep(duration);
            }
        });
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn t1() {
        let mut timer_pool = TimerPool::new();

        let instant_now = Instant::now();

        for i in 0..=5 {
            timer_pool.add_task(instant_now + Duration::from_secs(i), move |_| {
                println!("{i}s passed")
            });
        }
        while !timer_pool.is_empty() {
            timer_pool.sleep_until_available();
            while let Some(task) = timer_pool.poll() {
                task(&mut timer_pool);
            }
        }
    }
}
