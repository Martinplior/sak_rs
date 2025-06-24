use std::{
    future::Future,
    sync::atomic::{self, AtomicBool, AtomicUsize},
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    thread::Thread,
    time::{Duration, Instant},
};

use crate::thread::TimerThread;

struct ThreadWaker {
    thread: Thread,
    unparked: AtomicBool,

    timer_thread: TimerThread,

    strong_count: AtomicUsize,
}

impl ThreadWaker {
    const VTABLE: RawWakerVTable = RawWakerVTable::new(
        ThreadWaker::clone,
        ThreadWaker::wake_by_ref,
        ThreadWaker::wake_by_ref,
        ThreadWaker::drop,
    );

    fn clone(raw_this: *const ()) -> RawWaker {
        let this = unsafe { &*(raw_this as *const Self) };
        this.strong_count.fetch_add(1, atomic::Ordering::Relaxed);
        RawWaker::new(raw_this, &Self::VTABLE)
    }

    fn wake_by_ref(raw_this: *const ()) {
        let this = unsafe { &*(raw_this as *const Self) };
        let unparked = this.unparked.swap(true, atomic::Ordering::Release);
        (!unparked).then(|| this.thread.unpark());
    }

    fn drop(raw_this: *const ()) {
        let old_strong_count = {
            // restrict `&Self` in the scope.
            let this = unsafe { &*(raw_this as *const Self) };
            this.strong_count.fetch_sub(1, atomic::Ordering::Release)
        };
        if old_strong_count == 1 {
            atomic::fence(atomic::Ordering::Acquire);
            let _ = unsafe { Box::from_raw(raw_this as *mut Self) };
        }
    }

    /// returns `None` if `waker` is not [`ThreadWaker`].
    fn ref_from_waker(waker: &Waker) -> Option<&Self> {
        if waker.vtable() != &Self::VTABLE {
            return None;
        }
        Some(unsafe { &*(waker.data() as *const Self) })
    }
}

pub fn block_on<R>(f: impl Future<Output = R>) -> R {
    let mut f = std::pin::pin!(f);
    let thread_waker = &*Box::leak(Box::new(ThreadWaker {
        thread: std::thread::current(),
        unparked: false.into(),
        timer_thread: TimerThread::with_capacity(8),
        strong_count: 1.into(),
    }));
    let waker = unsafe {
        Waker::from_raw(RawWaker::new(
            thread_waker as *const ThreadWaker as _,
            &ThreadWaker::VTABLE,
        ))
    };
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(r) = f.as_mut().poll(&mut cx) {
            return r;
        }
        while !thread_waker.unparked.swap(false, atomic::Ordering::Acquire) {
            std::thread::park();
        }
    }
}

pub fn block_on_spin<const YIELD: bool, R>(f: impl Future<Output = R>) -> R {
    let mut f = std::pin::pin!(f);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(r) = f.as_mut().poll(&mut cx) {
            return r;
        }
        YIELD.then(std::thread::yield_now);
    }
}

pub async fn join<R1, R2>(f1: impl Future<Output = R1>, f2: impl Future<Output = R2>) -> (R1, R2) {
    let mut f1 = std::pin::pin!(f1);
    let mut f2 = std::pin::pin!(f2);
    let mut r1 = None;
    let mut r2 = None;
    std::future::poll_fn(move |cx| {
        if r1.is_none() {
            if let Poll::Ready(r) = f1.as_mut().poll(cx) {
                r1 = Some(r);
            }
        }
        if r2.is_none() {
            if let Poll::Ready(r) = f2.as_mut().poll(cx) {
                r2 = Some(r);
            }
        }
        if r1.is_some() && r2.is_some() {
            let r = unsafe { (r1.take().unwrap_unchecked(), r2.take().unwrap_unchecked()) };
            Poll::Ready(r)
        } else {
            Poll::Pending
        }
    })
    .await
}

/// only for [`crate::async_::block_on`].
///
/// otherwise fallback to busy spin.
pub async fn sleep(duration: Duration) {
    let deadline = Instant::now() + duration;
    let mut once = Some(());
    std::future::poll_fn(|cx| {
        if let Some(thread_waker) = ThreadWaker::ref_from_waker(cx.waker()) {
            once.take().map(|_| {
                let waker = cx.waker().clone();
                thread_waker
                    .timer_thread
                    .add_task(deadline, move |_| waker.wake_by_ref())
            });

            let time_out = Instant::now() >= deadline;
            if time_out {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        } else {
            // fallback to busy spin
            let time_out = Instant::now() >= deadline;
            if time_out {
                Poll::Ready(())
            } else {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    })
    .await
}

pub async fn yield_now() {
    let mut yielded = false;
    std::future::poll_fn(|cx| {
        if yielded {
            Poll::Ready(())
        } else {
            yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t1() {
        let f1 = async {
            println!("f1 start sleep");
            sleep(Duration::from_secs(3)).await;
            println!("f1 end sleep");
        };
        let f2 = async {
            println!("f2 start yield");
            yield_now().await;
            println!("f2 end yield");
        };
        println!("start block");
        block_on(join(f1, f2));
        println!("end");
    }
}
