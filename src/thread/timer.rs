use std::{num::NonZero, thread::JoinHandle, time::Instant};

use crossbeam_channel::{Receiver as MpscReceiver, RecvTimeoutError, Sender as MpscSender};

use crate::sync::{TimerPool, timer::TimerTaskFn};

enum TimerTask {
    Task(Box<TimerTaskFn>),
    Exit,
}

pub struct TimerThread {
    join_handle: Option<JoinHandle<()>>,
    task_sender: MpscSender<(Instant, TimerTask)>,
}

impl Default for TimerThread {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl TimerThread {
    #[inline]
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let (task_sender, task_receiver) = crossbeam_channel::unbounded();
        let join_handle =
            std::thread::spawn(move || Self::thread_main(task_receiver, NonZero::new(capacity)));
        Self {
            join_handle: Some(join_handle),
            task_sender,
        }
    }

    #[inline]
    pub fn add_task(&self, deadline: Instant, task: impl FnOnce(&mut TimerPool) + Send + 'static) {
        self.add_task_boxed(deadline, Box::new(task));
    }

    #[inline]
    pub fn add_task_boxed(&self, deadline: Instant, task: Box<TimerTaskFn>) {
        self.send((deadline, TimerTask::Task(task)));
    }

    pub fn join(mut self) -> std::thread::Result<()> {
        unsafe { self.join_by_ref().unwrap_unchecked() }
    }
}

impl TimerThread {
    #[inline]
    fn thread_main(
        task_receiver: MpscReceiver<(Instant, TimerTask)>,
        capacity: Option<NonZero<usize>>,
    ) {
        let mut timer_pool = if let Some(capacity) = capacity {
            TimerPool::with_capacity(capacity.into())
        } else {
            TimerPool::new()
        };
        let mut need_exit = false;

        loop {
            // step 1: poll timers.
            while let Some(task) = timer_pool.poll() {
                task(&mut timer_pool);
            }

            // step 2: if has deadline, block until deadline, else until a new task arrived.
            let new_task = if let Some(deadline) = timer_pool.peek() {
                match task_receiver.recv_deadline(*deadline) {
                    Ok(task) => Some(task),
                    Err(RecvTimeoutError::Timeout) => None,
                    _ => unreachable!(),
                }
            } else if need_exit {
                // no deadline, then `time_pool` is empty, exit now.
                return;
            } else {
                Some(task_receiver.recv().expect("unreachable"))
            };

            // `Self::join` take `self`, so no `TimerTask::Task` after `TimerTask::Exit`.
            let mut f = |task| match task {
                (deadline, TimerTask::Task(task)) => timer_pool.add_task_boxed(deadline, task),
                (_, TimerTask::Exit) => need_exit = true,
            };

            new_task.map(&mut f);

            // step 3: receive remaining tasks.
            task_receiver.try_iter().for_each(&mut f);
        }
    }

    fn send(&self, timer: (Instant, TimerTask)) {
        self.task_sender.send(timer).expect("unreachable");
    }

    fn join_by_ref(&mut self) -> Option<std::thread::Result<()>> {
        self.join_handle.take().map(|j| {
            self.send((Instant::now(), TimerTask::Exit));
            j.join()
        })
    }
}

impl Drop for TimerThread {
    fn drop(&mut self) {
        self.join_by_ref().map(|r| r.expect("TimerThread panic"));
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn t1() {
        let timer_thread = TimerThread::new();

        let instant_now = Instant::now();

        for i in (0..=5).rev() {
            for id in 1..=4 {
                timer_thread.add_task(instant_now + Duration::from_secs(i), move |_| {
                    println!("[{id}]: {i}s passed")
                });
            }
        }
    }
}
