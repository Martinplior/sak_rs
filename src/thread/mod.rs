#[cfg(feature = "thread_async")]
pub mod async_;

pub mod timer;
pub mod worker;

#[cfg(feature = "thread_async")]
pub use async_::{AsyncThread, AsyncThreadPool};

pub use timer::TimerThread;
pub use worker::{ThreadPool, WorkerThread};
