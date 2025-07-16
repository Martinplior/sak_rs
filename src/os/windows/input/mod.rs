pub mod global_listener;
pub mod raw_input;

pub use global_listener::GlobalListener;

#[cfg(test)]
mod tests {
    use std::{
        io::Write,
        time::{Duration, Instant},
    };

    use windows::Win32::{
        Foundation::HWND,
        UI::{
            Input::KeyboardAndMouse::{
                INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_A,
                VK_BACK,
            },
            WindowsAndMessaging::WM_INPUT,
        },
    };

    use crate::sync::mpmc;

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

    #[test]
    fn test_latency() {
        let (sender, reciver) = mpmc::queue::unbounded();
        let msg_hook = move |win_msg: &WinMsg| {
            if win_msg.msg.message != WM_INPUT {
                return false;
            }
            sender.send(win_msg.instant);
            true
        };
        let raw_input_hook = |&hwnd: &HWND| {
            raw_input::device::register(
                raw_input::device::DeviceType::Keyboard,
                raw_input::device::OptionType::inputsink(hwnd),
            );
        };
        let _listener = GlobalListener::new(msg_hook, raw_input_hook);
        let count = 4 * 10;
        let begin_instants: Vec<_> = (0..count)
            .map(|i| {
                let input = INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: match i % 4 {
                                0 | 1 => VK_A,
                                2 | 3 => VK_BACK,
                                _ => unreachable!(),
                            },
                            dwFlags: if i % 2 == 0 {
                                Default::default()
                            } else {
                                KEYEVENTF_KEYUP
                            },
                            ..Default::default()
                        },
                    },
                };
                let before_send_instant = Instant::now();
                unsafe { SendInput(&[input], size_of::<INPUT>() as i32) };
                let after_send_instant = Instant::now();
                std::thread::sleep(Duration::from_millis(100));
                (before_send_instant, after_send_instant)
            })
            .collect();
        let end_instants: Vec<_> = reciver.try_iter().collect();
        let latencies: Vec<_> = end_instants
            .iter()
            .zip(begin_instants.iter())
            .map(|(end, &(before, after))| (end.duration_since(before), end.duration_since(after)))
            .collect();
        let (before_sum, after_sum) = latencies
            .iter()
            .fold((Duration::ZERO, Duration::ZERO), |acc, x| {
                (acc.0 + x.0, acc.1 + x.1)
            });
        let latency_min = (
            latencies.iter().map(|x| x.0).min().unwrap(),
            latencies.iter().map(|x| x.1).min().unwrap(),
        );
        let latency_max = (
            latencies.iter().map(|x| x.0).max().unwrap(),
            latencies.iter().map(|x| x.1).max().unwrap(),
        );
        let latency_avg = (before_sum / count, after_sum / count);
        println!("latencies: {:?}", latencies);
        println!("min latency: {:?}", latency_min);
        println!("max latency: {:?}", latency_max);
        println!("avg latency: {:?}", latency_avg);
    }
}
