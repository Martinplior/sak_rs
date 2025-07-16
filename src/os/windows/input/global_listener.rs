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
use crate::sync::spsc::{self, OnceSender};

#[derive(Debug, Clone, PartialEq)]
pub struct WinMsg {
    pub msg: MSG,
    pub instant: Instant,
}

pub struct GlobalListener {
    msg_hwnd: HWND,
    thread: Option<JoinHandle<()>>,
}

impl GlobalListener {
    /// `msg_hook`: return true if you don't want msg to be dispatched.
    /// `register_raw_input_hook`: register your raw input.
    pub fn new(
        msg_hook: impl FnMut(&WinMsg) -> bool + Send + 'static,
        register_raw_input_hook: impl FnOnce(&HWND) + Send + 'static,
    ) -> Self {
        Self::init_window_class();
        let mut once_buf = MaybeUninit::uninit();
        let (hwnd_sender, hwnd_receiver) = unsafe { spsc::once_inplace_unchecked(&mut once_buf) };
        let thread = std::thread::spawn(|| {
            Self::thread_main(msg_hook, register_raw_input_hook, hwnd_sender)
        });
        let msg_hwnd = hwnd_receiver.recv();
        let msg_hwnd = HWND(msg_hwnd as _);
        Self {
            msg_hwnd,
            thread: Some(thread),
        }
    }

    pub fn join(mut self) -> std::thread::Result<()> {
        unsafe { self.join_by_ref().unwrap_unchecked() }
    }
}

impl GlobalListener {
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

    fn join_by_ref(&mut self) -> Option<std::thread::Result<()>> {
        self.thread.take().map(|j| {
            let _ = unsafe { PostMessageW(Some(self.msg_hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
            j.join()
        })
    }

    fn thread_main(
        mut msg_hook: impl FnMut(&WinMsg) -> bool,
        register_raw_input_hook: impl FnOnce(&HWND),
        hwnd_sender: OnceSender<usize>,
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

        let mut win_msg = WinMsg {
            msg: Default::default(),
            instant: Instant::now(),
        };
        loop {
            let r = unsafe { GetMessageW(&mut win_msg.msg, Some(hwnd), 0, 0) }.0;
            win_msg.instant = Instant::now();
            if matches!(r, 0 | -1) {
                break;
            }
            if msg_hook(&win_msg) {
                continue;
            }
            unsafe { DispatchMessageW(&win_msg.msg) };
        }
    }
}

impl Drop for GlobalListener {
    fn drop(&mut self) {
        self.join_by_ref().map(|r| r.expect("GlobalListener panic"));
    }
}
