use std::mem::ManuallyDrop;
use std::mem::MaybeUninit;
use std::thread::JoinHandle;
use std::time::Instant;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, HWND_MESSAGE, MSG,
    PostMessageW, RegisterClassExW, WM_CLOSE, WNDCLASSEXW,
};
use windows::core::PCWSTR;

use crate::os::windows::panic_from_win32;

#[derive(Debug, Clone, PartialEq)]
pub struct WinMsg {
    pub msg: MSG,
    pub instant: Instant,
}

pub struct GlobalListener {
    msg_hwnd: HWND,
    thread: ManuallyDrop<JoinHandle<()>>,
}

impl GlobalListener {
    /// `msg_hook`: return true if you dont't want msg to be dispatched.
    /// `register_raw_input_hook`: register your raw input.
    pub fn new(
        msg_hook: impl FnMut(&WinMsg) -> bool + Send + 'static,
        register_raw_input_hook: impl FnOnce(&HWND) + Send + 'static,
    ) -> Self {
        Self::init_window_class();
        let (hwnd_sender, hwnd_receiver) = crate::sync::spsc::once();
        let thread = std::thread::spawn(|| {
            Self::thread_main(msg_hook, register_raw_input_hook, hwnd_sender)
        });
        let msg_hwnd = hwnd_receiver.recv();
        let msg_hwnd = HWND(msg_hwnd as _);
        Self {
            msg_hwnd,
            thread: ManuallyDrop::new(thread),
        }
    }

    const fn window_class_name() -> PCWSTR {
        windows::core::w!("global_listener_window_class")
    }

    fn init_window_class() {
        unsafe extern "system" fn wnd_proc(
            hwnd: HWND,
            msg: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(|| {
            let window_class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as _,
                lpfnWndProc: Some(wnd_proc),
                hInstance: unsafe { GetModuleHandleW(None) }
                    .expect("unreachable")
                    .into(),
                lpszClassName: Self::window_class_name(),
                ..Default::default()
            };
            if unsafe { RegisterClassExW(&window_class) } == 0 {
                panic_from_win32();
            }
        });
    }

    fn thread_main(
        mut msg_hook: impl FnMut(&WinMsg) -> bool,
        register_raw_input_hook: impl FnOnce(&HWND),
        hwnd_sender: crate::sync::spsc::OnceSender<usize>,
    ) {
        let hwnd = unsafe {
            CreateWindowExW(
                Default::default(),
                Self::window_class_name(),
                None,
                Default::default(),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )
        }
        .expect("unreachable");

        hwnd_sender.send(hwnd.0 as _);

        register_raw_input_hook(&hwnd);

        loop {
            let mut win_msg = MaybeUninit::<WinMsg>::uninit();
            let r =
                unsafe { GetMessageW(&raw mut (*win_msg.as_mut_ptr()).msg, Some(hwnd), 0, 0) }.0;
            let instant = Instant::now();
            if matches!(r, 0 | -1) {
                break;
            }
            let win_msg = unsafe {
                (*win_msg.as_mut_ptr()).instant = instant;
                win_msg.assume_init()
            };
            if msg_hook(&win_msg) {
                continue;
            }
            unsafe { DispatchMessageW(&win_msg.msg) };
        }
    }
}

impl Drop for GlobalListener {
    fn drop(&mut self) {
        let _ = unsafe { PostMessageW(Some(self.msg_hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        unsafe { ManuallyDrop::take(&mut self.thread) }
            .join()
            .expect("global listener thread panicked");
    }
}
