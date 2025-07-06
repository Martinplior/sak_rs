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
            format!("{:?}", err)
        };
        message_dialog::error(message).show();
    })
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn t1() {}
}
