pub mod global_listener;
pub mod raw_input;

pub use global_listener::GlobalListener;

#[cfg(test)]
mod tests {
    use std::{io::Write, time::Duration};

    use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::WM_INPUT};

    use super::{global_listener::WinMsg, *};

    #[test]
    fn test_mouse_input() {
        let msg_hook = |win_msg: &WinMsg| {
            if win_msg.msg.message != WM_INPUT {
                return false;
            }
            let raw_input::RawInput::Mouse(mouse) = raw_input::RawInput::from_msg(&win_msg.msg)
            else {
                return false;
            };
            print!("\r{}", " ".repeat(80));
            print!("\rflags: {:#b}", mouse.flags_and_data().usButtonFlags);
            std::io::stdout().flush().unwrap();
            false
        };
        let raw_input_hook = |hwnd: &HWND| {
            raw_input::device::register(
                raw_input::device::DeviceType::Mouse,
                raw_input::device::OptionType::inputsink(*hwnd),
            );
        };
        let _listener = GlobalListener::new(msg_hook, raw_input_hook);
        std::thread::sleep(Duration::from_secs(60));
    }
}
