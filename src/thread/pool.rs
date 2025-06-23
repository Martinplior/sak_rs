use std::{cell::Cell, num::NonZero, task::Waker};

use crate::{
    sync::spsc::{self, OnceReceiver},
    thread::{
        async_::RawAsyncThread,
        worker::{RawWorkerThread, Task},
    },
};

use super::async_::Task as AsyncTask;

use crossbeam_channel::Sender as MpmcSender;
use tinyrand::{RandRange, StdRand};

pub struct ThreadPool {
    workers: Option<Box<[RawWorkerThread]>>,
    task_sender: MpmcSender<Task>,
}

impl ThreadPool {
    pub fn new(num_workers: NonZero<usize>) -> Self {
        let num_workers = num_workers.get();
        let (task_sender, task_receiver) = crossbeam_channel::unbounded();
        let workers: Box<_> = (0..num_workers)
            .map(|_| RawWorkerThread::new(task_receiver.clone()))
            .collect();
        Self {
            workers: Some(workers),
            task_sender,
        }
    }

    #[inline]
    pub fn add_task(&self, task: impl FnOnce() + Send + 'static) {
        self.add_task_boxed(Box::new(task));
    }

    pub fn add_task_sync<R: Send + 'static>(
        &self,
        task: impl FnOnce() -> R + Send + 'static,
    ) -> OnceReceiver<R> {
        let (r_sender, r_receiver) = spsc::once();
        let task = Box::new(|| r_sender.send(task()));
        self.add_task_boxed(task);
        r_receiver
    }

    #[inline]
    pub fn add_task_boxed(&self, task: Box<dyn FnOnce() + Send>) {
        self.send(Task::Task(task));
    }

    #[inline]
    pub fn num_workers(&self) -> usize {
        unsafe { self.workers.as_ref().unwrap_unchecked() }.len()
    }

    pub fn join(mut self) -> std::thread::Result<()> {
        unsafe { self.join_by_ref().unwrap_unchecked() }
    }
}

impl ThreadPool {
    fn send(&self, task: Task) {
        self.task_sender.send(task).expect("unreachable");
    }

    fn join_by_ref(&mut self) -> Option<std::thread::Result<()>> {
        self.workers.take().map(|workers| {
            workers.iter().for_each(|_| self.send(Task::Exit));
            workers.into_iter().try_for_each(|w| w.join())
        })
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.join_by_ref().map(|r| r.expect("ThreadPool panic"));
    }
}

/// async version of [`ThreadPool`].
///
/// each thread can run async tasks.
pub struct AsyncThreadPool {
    workers: Option<Box<[RawAsyncThread]>>,
    wakers: Box<[Waker]>,
    task_sender: MpmcSender<AsyncTask>,
    rnd: Cell<StdRand>,
}

impl AsyncThreadPool {
    #[inline]
    pub fn new(num_workers: NonZero<usize>) -> Self {
        Self::with_capacity(num_workers, 0)
    }

    pub fn with_capacity(num_workers: NonZero<usize>, capacity: usize) -> Self {
        let num_workers = num_workers.get();
        let (task_sender, task_receiver) = crossbeam_channel::unbounded();
        let (workers, wakers): (Vec<_>, Vec<_>) = (0..num_workers)
            .map(|_| RawAsyncThread::with_capacity(task_receiver.clone(), capacity))
            .collect();
        let workers = workers.into_boxed_slice();
        let wakers = wakers.into_boxed_slice();
        // fixed seed? it doesn't really matter imo...
        let rnd = Cell::default();
        Self {
            workers: Some(workers),
            wakers,
            task_sender,
            rnd,
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
        self.send_and_wake(AsyncTask::Task(Box::into_pin(task)));
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
    fn send_and_wake(&self, task: AsyncTask) {
        self.task_sender.send(task).expect("unreachable");
        // just wake a worker randomly...
        let index = self.rnd_scope(|r| r.next_range(0..self.wakers.len()));
        unsafe { self.wakers.get_unchecked(index) }.wake_by_ref();
    }

    #[inline]
    fn rnd_scope<R>(&self, f: impl FnOnce(&mut StdRand) -> R) -> R {
        let mut rnd = self.rnd.take();
        let r = f(&mut rnd);
        self.rnd.set(rnd);
        r
    }

    fn join_by_ref(&mut self) -> Option<std::thread::Result<()>> {
        self.workers.take().map(|workers| {
            workers
                .iter()
                .for_each(|_| self.task_sender.send(AsyncTask::Exit).expect("unreachable"));
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
        let num_workers = std::thread::available_parallelism()
            .map(|n| n.into())
            .unwrap_or(2);
        let thread_pool = ThreadPool::new(num_workers.try_into().unwrap());
        (0..(num_workers * 5)).for_each(|i| {
            thread_pool.add_task(move || {
                std::thread::sleep(Duration::from_secs(1));
                println!("f({i}) end");
            });
        });
        println!("hello!");
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
