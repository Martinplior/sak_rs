use std::{
    future::Future,
    sync::{
        Arc, OnceLock,
        atomic::{self, AtomicBool},
    },
    task::{Context, Poll, RawWaker, RawWakerVTable, Wake, Waker},
    thread::Thread,
    time::{Duration, Instant},
};

pub fn block_on<R>(f: impl Future<Output = R>) -> R {
    struct ThreadWaker {
        thread: Thread,
        unparked: AtomicBool,
    }

    impl ThreadWaker {
        fn new() -> Self {
            Self {
                thread: std::thread::current(),
                unparked: false.into(),
            }
        }
    }

    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.wake_by_ref();
        }

        fn wake_by_ref(self: &Arc<Self>) {
            let unparked = self.unparked.swap(true, atomic::Ordering::Release);
            if !unparked {
                self.thread.unpark();
            }
        }
    }

    let mut f = std::pin::pin!(f);
    let thread_waker = Arc::new(ThreadWaker::new());
    let waker = thread_waker.clone().into();
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
    const VTABLE: RawWakerVTable = RawWakerVTable::new(|_| WAKER, |_| {}, |_| {}, |_| {});
    const WAKER: RawWaker = RawWaker::new(&(), &VTABLE);

    let mut f = std::pin::pin!(f);
    let waker = unsafe { Waker::from_raw(WAKER) };
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(r) = f.as_mut().poll(&mut cx) {
            return r;
        }
        YIELD.then(|| std::thread::yield_now());
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

pub async fn sleep(duration: Duration) {
    struct Shared {
        time_out: AtomicBool,
        waker: OnceLock<Waker>,
    }
    let dead_line = Instant::now() + duration;
    let shared = Arc::new(Shared {
        time_out: false.into(),
        waker: Default::default(),
    });
    let shared_1 = shared.clone();
    // can you come up with a better implementation?
    std::thread::spawn(move || {
        let instant_now = Instant::now();
        if instant_now < dead_line {
            std::thread::sleep(dead_line - instant_now);
        }
        shared_1.time_out.store(true, atomic::Ordering::Relaxed);
        shared_1.waker.get().map(|waker| waker.wake_by_ref());
    });
    std::future::poll_fn(move |cx| {
        shared.waker.get_or_init(|| cx.waker().clone());
        let time_out = shared.time_out.load(atomic::Ordering::Relaxed);
        if time_out {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    })
    .await
}

pub async fn yield_now() {
    let mut yielded = false;
    std::future::poll_fn(move |cx| {
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
            sleep(Duration::from_secs(1)).await;
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
