use std::{
    num::NonZero,
    pin::Pin,
    task::{Poll, Waker},
    thread::JoinHandle,
};

use crossbeam_channel::{Receiver as MpmcReceiver, Sender as MpmcSender};

use crate::sync::spsc::{self, OnceReceiver, OnceSender};

pub(super) enum Task {
    Task(Pin<Box<dyn Future<Output = ()> + Send>>),
    Exit,
}

pub(super) struct RawAsyncThread(JoinHandle<()>);

impl RawAsyncThread {
    pub(super) fn with_capacity(
        task_receiver: MpmcReceiver<Task>,
        capacity: usize,
    ) -> (Self, Waker) {
        let (waker_sender, waker_receiver) = spsc::once();

        let join_handle = std::thread::spawn(move || {
            Self::thread_main(task_receiver, waker_sender, NonZero::new(capacity))
        });

        let waker = waker_receiver.recv();
        (Self(join_handle), waker)
    }

    /// before [`join`](Self::join), ensure that this worker can receive a [`Task::Exit`].
    ///
    /// othewise `join` will block forever...
    pub(super) fn join(self) -> std::thread::Result<()> {
        self.0.join()
    }
}

impl RawAsyncThread {
    #[inline]
    fn thread_main(
        task_receiver: MpmcReceiver<Task>,
        waker_sender: OnceSender<Waker>,
        capacity: Option<NonZero<usize>>,
    ) {
        let mut tasks = if let Some(capacity) = capacity {
            Vec::with_capacity(capacity.into())
        } else {
            Vec::new()
        };
        let mut need_exit = false;
        let mut waker_sender = Some(waker_sender);

        let f = std::future::poll_fn(|cx| {
            waker_sender.take().map(|s| s.send(cx.waker().clone()));

            // step 1: receive all tasks if don't need exit.
            if !need_exit {
                // when there are multiple receivers on the same channel, there may be multiple
                // `Task::Exit` in the channel.
                let _ = task_receiver.try_iter().try_for_each(|task| match task {
                    Task::Task(task) => {
                        tasks.push(task);
                        Ok(())
                    }
                    Task::Exit => {
                        need_exit = true;
                        Err(())
                    }
                });
            }

            // step 2: poll futures, retain pending ones.
            tasks.retain_mut(|f| f.as_mut().poll(cx).is_pending());

            if need_exit && tasks.is_empty() {
                // ready means exit.
                Poll::Ready(())
            } else {
                // sender should send a task and wake this thread up.
                Poll::Pending
            }
        });

        crate::async_::block_on(f);
    }
}

/// similar to a thread pool, [`AsyncThread`] can execute async tasks on an independent thread.
pub struct AsyncThread {
    raw: Option<RawAsyncThread>,
    task_sender: MpmcSender<Task>,
    waker: Waker,
}

impl Default for AsyncThread {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl AsyncThread {
    #[inline]
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let (task_sender, task_receiver) = crossbeam_channel::unbounded();
        let (raw, waker) = RawAsyncThread::with_capacity(task_receiver, capacity);
        Self {
            raw: Some(raw),
            task_sender,
            waker,
        }
    }

    #[inline]
    pub fn add_task(&self, task: impl Future<Output = ()> + Send + 'static) {
        self.add_task_boxed(Box::new(task));
    }

    pub fn add_task_sync<R: Send + 'static>(
        &self,
        task: impl Future<Output = R> + Send + 'static,
    ) -> OnceReceiver<R> {
        let (r_sender, r_receiver) = spsc::once();
        let task = Box::new(async { r_sender.send(task.await) });
        self.add_task_boxed(task);
        r_receiver
    }

    #[inline]
    pub fn add_task_boxed(&self, task: Box<dyn Future<Output = ()> + Send>) {
        self.send_and_wake(Task::Task(Box::into_pin(task)));
    }

    /// this function will wake the thread only once, so it might be slightly more efficient.
    pub fn add_tasks(
        &self,
        task_iter: impl IntoIterator<Item = impl Future<Output = ()> + Send + 'static>,
    ) {
        task_iter.into_iter().for_each(|task| {
            self.task_sender
                .send(Task::Task(Box::pin(task)))
                .expect("unreachable");
        });
        self.waker.wake_by_ref();
    }

    pub fn join(mut self) -> std::thread::Result<()> {
        unsafe { self.join_by_ref().unwrap_unchecked() }
    }
}

impl AsyncThread {
    fn send_and_wake(&self, task: Task) {
        self.task_sender.send(task).expect("unreachable");
        self.waker.wake_by_ref();
    }

    fn join_by_ref(&mut self) -> Option<std::thread::Result<()>> {
        self.raw.take().map(|j| {
            self.send_and_wake(Task::Exit);
            j.join()
        })
    }
}

impl Drop for AsyncThread {
    fn drop(&mut self) {
        self.join_by_ref().map(|r| r.expect("AsyncThread panic"));
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::async_;

    use super::*;

    #[test]
    fn t1() {
        async fn foo(i: usize) {
            async_::sleep(Duration::from_secs(3)).await;
            println!("foo({i}) end");
        }
        let worker = AsyncThread::new();
        (0..500).for_each(|i| worker.add_task(foo(i)));
        println!("hello!");
        std::thread::sleep(Duration::from_secs(2));
        println!("world!");
        (500..1_000).for_each(|i| worker.add_task(foo(i)));
    }
}
