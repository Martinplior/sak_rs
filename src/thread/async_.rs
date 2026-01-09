use std::{
    cell::Cell,
    collections::LinkedList,
    num::NonZero,
    pin::Pin,
    task::{Poll, Waker},
    thread::JoinHandle,
};

use crate::sync::mpmc::queue::{UnboundedReceiver as MpmcReceiver, UnboundedSender as MpmcSender};
use crate::sync::spsc::{self, OnceReceiver, OnceSender};

enum Task {
    Task(Pin<Box<dyn Future<Output = ()> + Send>>),
    Exit,
}

struct RawAsyncThread(JoinHandle<()>);

impl RawAsyncThread {
    fn new(task_receiver: MpmcReceiver<Task>) -> (Self, Waker) {
        let (waker_sender, waker_receiver) = spsc::once();

        let join_handle =
            std::thread::spawn(move || Self::thread_main(task_receiver, waker_sender));

        let waker = waker_receiver.recv();
        (Self(join_handle), waker)
    }

    /// before [`join`](Self::join), ensure that this worker can receive a [`Task::Exit`].
    ///
    /// othewise `join` will block forever...
    fn join(self) -> std::thread::Result<()> {
        self.0.join()
    }
}

impl RawAsyncThread {
    #[inline]
    fn thread_main(task_receiver: MpmcReceiver<Task>, waker_sender: OnceSender<Waker>) {
        let mut tasks = LinkedList::new();
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
                        tasks.push_back(task);
                        Ok(())
                    }
                    Task::Exit => {
                        need_exit = true;
                        Err(())
                    }
                });
            }

            // step 2: poll futures, retain pending ones.
            tasks
                .extract_if(|f| f.as_mut().poll(cx).is_ready())
                .for_each(drop);

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
    pub fn new() -> Self {
        let (task_sender, task_receiver) = crate::sync::mpmc::queue::unbounded();
        let (raw, waker) = RawAsyncThread::new(task_receiver);
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
            self.task_sender.send(Task::Task(Box::pin(task)));
        });
        self.waker.wake_by_ref();
    }

    pub fn join(mut self) -> std::thread::Result<()> {
        unsafe { self.join_by_ref().unwrap_unchecked() }
    }
}

impl AsyncThread {
    fn send_and_wake(&self, task: Task) {
        self.task_sender.send(task);
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

/// async version of [`ThreadPool`].
///
/// each thread can run async tasks.
pub struct AsyncThreadPool {
    workers: Option<Box<[RawAsyncThread]>>,
    wakers: Box<[Waker]>,
    task_sender: MpmcSender<Task>,
    index_to_wake: Cell<usize>,
}

impl AsyncThreadPool {
    pub fn new(num_workers: NonZero<usize>) -> Self {
        let num_workers = num_workers.get();
        let (task_sender, task_receiver) = crate::sync::mpmc::queue::unbounded();
        let (workers, wakers): (Vec<_>, Vec<_>) = (0..num_workers)
            .map(|_| RawAsyncThread::new(task_receiver.clone()))
            .collect();
        let workers = workers.into_boxed_slice();
        let wakers = wakers.into_boxed_slice();
        Self {
            workers: Some(workers),
            wakers,
            task_sender,
            index_to_wake: Cell::new(0),
        }
    }

    #[inline]
    pub fn add_task(&self, task: impl Future<Output = ()> + Send + 'static) {
        self.add_task_boxed(Box::new(task));
    }

    #[inline]
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

    #[inline]
    pub fn num_workers(&self) -> usize {
        unsafe { self.workers.as_ref().unwrap_unchecked() }.len()
    }

    pub fn join(mut self) -> std::thread::Result<()> {
        unsafe { self.join_by_ref().unwrap_unchecked() }
    }
}

impl AsyncThreadPool {
    fn next_index_to_wake(&self, index_to_wake: usize) -> usize {
        (index_to_wake + 1) % self.num_workers()
    }

    fn send_and_wake(&self, task: Task) {
        self.task_sender.send(task);
        let index = self.index_to_wake.get();
        unsafe { self.wakers.get_unchecked(index) }.wake_by_ref();
        self.index_to_wake.set(self.next_index_to_wake(index));
    }

    fn join_by_ref(&mut self) -> Option<std::thread::Result<()>> {
        self.workers.take().map(|workers| {
            workers
                .iter()
                .for_each(|_| self.task_sender.send(Task::Exit));
            self.wakers.iter().for_each(|w| w.wake_by_ref());
            workers.into_iter().try_for_each(|w| w.join())
        })
    }
}

impl Drop for AsyncThreadPool {
    fn drop(&mut self) {
        self.join_by_ref()
            .map(|r| r.expect("AsyncThreadPool panic"));
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

    #[test]
    fn t2() {
        let num_workers = std::thread::available_parallelism()
            .map(|n| n.into())
            .unwrap_or(2);
        let thread_pool = AsyncThreadPool::new(num_workers.try_into().unwrap());
        (0..(num_workers * 5)).for_each(|i| {
            thread_pool.add_task(async move {
                async_::sleep(Duration::from_secs(3)).await;
                println!("f({i}) end");
            });
        });
        println!("hello!");
    }
}
