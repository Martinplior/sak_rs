#[cfg(feature = "os")]
pub mod os;

#[cfg(feature = "sync")]
pub mod sync;

#[cfg(feature = "cell")]
pub mod cell;

#[cfg(feature = "collections")]
pub mod collections;

#[cfg(feature = "async")]
pub mod async_;

#[cfg(feature = "thread")]
pub mod thread;

#[cfg(feature = "message_dialog")]
pub mod message_dialog;

#[cfg(feature = "graphics")]
pub mod graphics;

#[cfg(feature = "font")]
pub mod font;

#[cfg(feature = "math")]
pub mod math;

pub mod assert;
pub mod slice;

#[cfg(feature = "graceful_run")]
pub fn graceful_run<R>(
    f: impl FnOnce() -> R + std::panic::UnwindSafe,
) -> Result<R, Box<dyn std::any::Any + Send + 'static>> {
    std::panic::catch_unwind(f).inspect_err(|err| {
        let message = if let Some(s) = err.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = err.downcast_ref::<String>() {
            s.clone()
        } else {
            format!("{err:?}")
        };
        message_dialog::error(message).show();
    })
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    #[cfg(feature = "graceful_run")]
    fn t1() {
        let _ = graceful_run(|| Result::<(), _>::Err("test1").unwrap())
            .inspect_err(|err| assert!(err.is::<String>()));
        let _ = graceful_run(|| panic!("test2")).inspect_err(|err| assert!(err.is::<&str>()));
        let _ = graceful_run(|| std::panic::resume_unwind(Box::new("test3")));
        let _ = graceful_run(|| std::panic::resume_unwind(Box::new("test4".to_string())));
        let _ = graceful_run(|| std::panic::resume_unwind(Box::new(42)));
    }
}
