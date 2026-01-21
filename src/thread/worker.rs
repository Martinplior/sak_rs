use std::{num::NonZero, thread::JoinHandle};

use crossbeam_channel::{Receiver as MpmcReceiver, Sender as MpmcSender};

use crate::sync::spsc::{self, OnceReceiver};

enum Task {
    Task(Box<dyn FnOnce() + Send>),
    Exit,
}

struct RawWorkerThread(JoinHandle<()>);

impl RawWorkerThread {
    fn new(
        task_receiver: MpmcReceiver<Task>,
        builder: std::thread::Builder,
    ) -> std::io::Result<Self> {
        let join_handle = builder.spawn(|| Self::thread_main(task_receiver))?;
        Ok(Self(join_handle))
    }

    /// before [`join`](Self::join), ensure that this worker can receive a [`Task::Exit`].
    ///
    /// othewise `join` will block forever...
    fn join(self) -> std::thread::Result<()> {
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
    #[inline]
    pub fn new() -> Self {
        Self::with_builder(std::thread::Builder::new()).expect("failed to spawn thread")
    }

    pub fn with_builder(builder: std::thread::Builder) -> std::io::Result<Self> {
        let (task_sender, task_receiver) = crossbeam_channel::unbounded();
        let raw = Some(RawWorkerThread::new(task_receiver, builder)?);
        Ok(Self { raw, task_sender })
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
    pub fn join(mut self) -> std::thread::Result<()> {
        unsafe { self.join_by_ref().unwrap_unchecked() }
    }
}

impl WorkerThread {
    #[inline]
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
    #[inline]
    pub fn new(num_workers: NonZero<usize>) -> Self {
        Self::with_builder(num_workers, |_| std::thread::Builder::new())
            .expect("failed to create thread")
    }

    pub fn with_builder(
        num_workers: NonZero<usize>,
        mut builder: impl FnMut(usize) -> std::thread::Builder,
    ) -> std::io::Result<Self> {
        let num_workers = num_workers.get();
        let (task_sender, task_receiver) = crossbeam_channel::unbounded();
        let mut workers = Vec::with_capacity(num_workers);
        for index in 0..num_workers {
            workers.push(RawWorkerThread::new(task_receiver.clone(), builder(index))?);
        }
        Ok(Self {
            workers: Some(workers.into_boxed_slice()),
            task_sender,
        })
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

    #[inline]
    pub fn join(mut self) -> std::thread::Result<()> {
        unsafe { self.join_by_ref().unwrap_unchecked() }
    }
}

impl ThreadPool {
    #[inline]
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
