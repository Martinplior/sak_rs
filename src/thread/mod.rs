#[cfg(feature = "thread_async")]
pub mod async_;

pub mod timer;
pub mod worker;

#[cfg(feature = "thread_async")]
pub use async_::{AsyncThread, AsyncThreadPool};

pub use timer::TimerThread;
pub use worker::{ThreadPool, WorkerThread};

use std::time::{Duration, Instant};

#[inline(always)]
pub fn precise_sleep(duration: Duration) {
    const SPIN_THRESHOLD: Duration = Duration::from_millis(1);
    precise_sleep_with_spin_threshold(duration, SPIN_THRESHOLD);
}

#[inline]
pub fn precise_sleep_with_spin_threshold(duration: Duration, spin_threshold: Duration) {
    let deadline = Instant::now() + duration;
    duration
        .checked_sub(spin_threshold)
        .map(|d| std::thread::sleep(d));
    spin_sleep_until(deadline);
}

#[inline(always)]
pub fn spin_sleep(duration: Duration) {
    spin_sleep_until(Instant::now() + duration);
}

#[inline(always)]
pub fn spin_sleep_until(deadline: Instant) {
    while Instant::now() < deadline {
        std::thread::yield_now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t1() {
        let begin = Instant::now();
        // std::thread::sleep(Duration::from_millis(5));
        precise_sleep(Duration::from_millis(5));
        let elapsed = begin.elapsed();
        println!("elapsed: {:#?}", elapsed);
    }
}
