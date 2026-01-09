#[cfg(feature = "os_windows_input")]
pub mod input;

pub fn panic_from_win32() -> ! {
    let message = windows::core::Error::from_thread().message();
    panic!("{}", message)
}
