use std::{num::NonZero, thread::JoinHandle};

use crossbeam_channel::{Receiver as MpmcReceiver, Sender as MpmcSender};

use crate::sync::spsc::{self, OnceReceiver};

pub(super) enum Task {
    Task(Box<dyn FnOnce() + Send>),
    Exit,
}

pub(super) struct RawWorkerThread(JoinHandle<()>);

impl RawWorkerThread {
    pub(super) fn new(task_receiver: MpmcReceiver<Task>) -> Self {
        let join_handle = std::thread::spawn(|| Self::thread_main(task_receiver));
        Self(join_handle)
    }

    /// before [`join`](Self::join), ensure that this worker can receive a [`Task::Exit`].
    ///
    /// othewise `join` will block forever...
    pub(super) fn join(self) -> std::thread::Result<()> {
        self.0.join()
    }
}

impl RawWorkerThread {
    #[inline]
    fn thread_main(task_receiver: MpmcReceiver<Task>) {
        loop {
            let task = task_receiver.recv().expect("unreachable");
            match task {
                Task::Task(task) => task(),
                Task::Exit => return,
            }
        }
    }
}

pub struct WorkerThread {
    raw: Option<RawWorkerThread>,
    task_sender: MpmcSender<Task>,
}

impl Default for WorkerThread {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl WorkerThread {
    pub fn new() -> Self {
        let (task_sender, task_receiver) = crossbeam_channel::unbounded();
        let raw = Some(RawWorkerThread::new(task_receiver));
        Self { raw, task_sender }
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

    pub fn join(mut self) -> std::thread::Result<()> {
        unsafe { self.join_by_ref().unwrap_unchecked() }
    }
}

impl WorkerThread {
    fn send(&self, task: Task) {
        self.task_sender.send(task).expect("unreachable");
    }

    fn join_by_ref(&mut self) -> Option<std::thread::Result<()>> {
        self.raw.take().map(|j| {
            self.send(Task::Exit);
            j.join()
        })
    }
}

impl Drop for WorkerThread {
    fn drop(&mut self) {
        self.join_by_ref().map(|r| r.expect("WorkerThread panic"));
    }
}

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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn t1() {
        let worker = WorkerThread::new();
        (0..10).for_each(|i| worker.add_task(move || println!("hello! {i}")));
        println!("world!");
    }

    #[test]
    fn t2() {
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
}
