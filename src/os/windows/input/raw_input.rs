use windows::Win32::UI::{
    Input::{
        GetRawInputData, HRAWINPUT, KeyboardAndMouse::VIRTUAL_KEY, MOUSE_ATTRIBUTES_CHANGED,
        MOUSE_MOVE_NOCOALESCE, MOUSE_VIRTUAL_DESKTOP, RAWHID, RAWINPUT, RAWINPUT_0, RAWINPUTHEADER,
        RAWKEYBOARD, RAWMOUSE, RAWMOUSE_0_0, RID_DEVICE_INFO_TYPE, RID_INPUT, RIM_TYPEHID,
        RIM_TYPEKEYBOARD, RIM_TYPEMOUSE,
    },
    WindowsAndMessaging::{
        MSG, RI_KEY_E0, RI_KEY_E1, RI_MOUSE_BUTTON_4_DOWN, RI_MOUSE_BUTTON_4_UP,
        RI_MOUSE_BUTTON_5_DOWN, RI_MOUSE_BUTTON_5_UP, RI_MOUSE_HWHEEL, RI_MOUSE_LEFT_BUTTON_DOWN,
        RI_MOUSE_LEFT_BUTTON_UP, RI_MOUSE_MIDDLE_BUTTON_DOWN, RI_MOUSE_MIDDLE_BUTTON_UP,
        RI_MOUSE_RIGHT_BUTTON_DOWN, RI_MOUSE_RIGHT_BUTTON_UP, RI_MOUSE_WHEEL, WM_INPUT,
    },
};

use crate::os::windows::panic_from_win32;

/// [see also](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-rawkeyboard)
pub struct Keyboard<'a> {
    pub raw: &'a RAWKEYBOARD,
}

impl<'a> Keyboard<'a> {
    #[inline]
    pub fn make_code(&self) -> u16 {
        self.raw.MakeCode
    }

    #[inline]
    pub fn virtual_key(&self) -> VIRTUAL_KEY {
        VIRTUAL_KEY(self.raw.VKey)
    }

    #[inline]
    pub fn message(&self) -> u32 {
        self.raw.Message
    }

    #[inline]
    pub fn extra_information(&self) -> u32 {
        self.raw.ExtraInformation
    }

    #[inline]
    pub fn key_is_down(&self) -> bool {
        self.raw.Flags & 1 == 0
    }

    #[inline]
    pub fn key_is_up(&self) -> bool {
        !self.key_is_down()
    }

    #[inline]
    pub fn has_e0(&self) -> bool {
        self.raw.Flags & (RI_KEY_E0 as u16) != 0
    }

    #[inline]
    pub fn has_e1(&self) -> bool {
        self.raw.Flags & (RI_KEY_E1 as u16) != 0
    }
}

/// [see also](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-rawmouse)
pub struct Mouse<'a> {
    pub raw: &'a RAWMOUSE,
}

impl<'a> Mouse<'a> {
    #[inline]
    pub fn is_move_relative(&self) -> bool {
        self.raw.usFlags.0 & 1 == 0
    }

    #[inline]
    pub fn is_move_absolute(&self) -> bool {
        !self.is_move_relative()
    }

    #[inline]
    pub fn is_virtual_desktop(&self) -> bool {
        self.raw.usFlags.0 & MOUSE_VIRTUAL_DESKTOP.0 == 0
    }

    #[inline]
    pub fn is_attributes_changed(&self) -> bool {
        self.raw.usFlags.0 & MOUSE_ATTRIBUTES_CHANGED.0 == 0
    }

    #[inline]
    pub fn is_move_nocoalesce(&self) -> bool {
        self.raw.usFlags.0 & MOUSE_MOVE_NOCOALESCE.0 == 0
    }

    #[inline]
    pub fn last_x(&self) -> i32 {
        self.raw.lLastX
    }

    #[inline]
    pub fn last_y(&self) -> i32 {
        self.raw.lLastY
    }

    #[inline]
    pub fn extra_information(&self) -> u32 {
        self.raw.ulExtraInformation
    }

    #[inline]
    pub fn flags_and_data(&self) -> &RAWMOUSE_0_0 {
        unsafe { &self.raw.Anonymous.Anonymous }
    }

    #[inline]
    pub fn button_data(&self) -> u16 {
        self.flags_and_data().usButtonData
    }

    #[inline]
    pub fn is_left_button_down(&self) -> bool {
        self.flags_and_data().usButtonFlags & (RI_MOUSE_LEFT_BUTTON_DOWN as u16) != 0
    }

    #[inline]
    pub fn is_left_button_up(&self) -> bool {
        self.flags_and_data().usButtonFlags & (RI_MOUSE_LEFT_BUTTON_UP as u16) != 0
    }

    #[inline]
    pub fn is_right_button_down(&self) -> bool {
        self.flags_and_data().usButtonFlags & (RI_MOUSE_RIGHT_BUTTON_DOWN as u16) != 0
    }

    #[inline]
    pub fn is_right_button_up(&self) -> bool {
        self.flags_and_data().usButtonFlags & (RI_MOUSE_RIGHT_BUTTON_UP as u16) != 0
    }

    #[inline]
    pub fn is_middle_button_down(&self) -> bool {
        self.flags_and_data().usButtonFlags & (RI_MOUSE_MIDDLE_BUTTON_DOWN as u16) != 0
    }

    #[inline]
    pub fn is_middle_button_up(&self) -> bool {
        self.flags_and_data().usButtonFlags & (RI_MOUSE_MIDDLE_BUTTON_UP as u16) != 0
    }

    #[inline]
    pub fn is_ext1_button_down(&self) -> bool {
        self.flags_and_data().usButtonFlags & (RI_MOUSE_BUTTON_4_DOWN as u16) != 0
    }

    #[inline]
    pub fn is_ext1_button_up(&self) -> bool {
        self.flags_and_data().usButtonFlags & (RI_MOUSE_BUTTON_4_UP as u16) != 0
    }

    #[inline]
    pub fn is_ext2_button_down(&self) -> bool {
        self.flags_and_data().usButtonFlags & (RI_MOUSE_BUTTON_5_DOWN as u16) != 0
    }

    #[inline]
    pub fn is_ext2_button_up(&self) -> bool {
        self.flags_and_data().usButtonFlags & (RI_MOUSE_BUTTON_5_UP as u16) != 0
    }

    /// The wheel delta is stored in [Self::button_data].
    /// A positive value indicates that the wheel was rotated forward, away from the user;
    /// a negative value indicates that the wheel was rotated backward, toward the user.
    #[inline]
    pub fn is_wheel(&self) -> bool {
        self.flags_and_data().usButtonFlags & (RI_MOUSE_WHEEL as u16) != 0
    }

    /// The wheel delta is stored in [Self::button_data].
    /// A positive value indicates that the wheel was rotated to the right, away from the user;
    /// a negative value indicates that the wheel was rotated to the left, toward the user.
    #[inline]
    pub fn is_horizontal_wheel(&self) -> bool {
        self.flags_and_data().usButtonFlags & (RI_MOUSE_HWHEEL as u16) != 0
    }
}

/// [see also](https://docs.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-rawhid)
pub struct HID<'a> {
    pub raw: &'a [u8],
}

impl<'a> HID<'a> {
    #[inline]
    pub fn per_input_size(&self) -> u32 {
        self.as_raw_hid().dwSizeHid
    }

    #[inline]
    pub fn count(&self) -> u32 {
        self.as_raw_hid().dwCount
    }

    #[inline]
    pub fn data(&self) -> &[u8] {
        let offset = size_of::<u32>() * 2;
        unsafe { self.raw.get(offset..).unwrap_unchecked() }
    }
}

impl<'a> HID<'a> {
    fn as_raw_hid(&self) -> &RAWHID {
        unsafe { &*(self.raw.as_ptr() as *const RAWHID) }
    }
}

/// [see also](https://docs.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-rawinput)
pub enum RawData<'a> {
    Keyboard(Keyboard<'a>),
    Mouse(Mouse<'a>),
    HID(HID<'a>),
}

/// [see also](https://docs.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-rawinput)
#[derive(Debug)]
pub struct RawInputBufReader {
    buf: Box<[u8]>,
}

impl RawInputBufReader {
    #[inline]
    pub fn new() -> Self {
        Self {
            buf: vec![0; size_of::<RAWINPUT>().next_power_of_two()].into_boxed_slice(),
        }
    }

    /// `msg` should comes from `GetMessage` or `PeekMessage`.
    ///
    /// returns `None` if `msg.message` is not `WM_INPUT`
    pub fn read_from_msg(&mut self, msg: &MSG) -> Option<RawData<'_>> {
        if msg.message != WM_INPUT {
            return None;
        }
        let mut size = size_of::<RAWINPUT>() as _;
        let header_size = size_of::<RAWINPUTHEADER>() as _;
        let r = unsafe {
            GetRawInputData(
                HRAWINPUT(msg.lParam.0 as _),
                RID_INPUT,
                Some(self.buf.as_mut_ptr() as _),
                &mut size,
                header_size,
            )
        };
        if r == u32::MAX {
            panic_from_win32();
        }
        let data = match RID_DEVICE_INFO_TYPE(self.header().dwType) {
            RIM_TYPEKEYBOARD => RawData::Keyboard(Keyboard {
                raw: unsafe { &self.raw_data().keyboard },
            }),
            RIM_TYPEMOUSE => RawData::Mouse(Mouse {
                raw: unsafe { &self.raw_data().mouse },
            }),
            RIM_TYPEHID => {
                let mut size = self.header().dwSize;
                if self.buf.len() < size as usize {
                    self.buf = vec![0; (size as usize).next_power_of_two()].into_boxed_slice();
                }
                let r = unsafe {
                    GetRawInputData(
                        HRAWINPUT(msg.lParam.0 as _),
                        RID_INPUT,
                        Some(self.buf.as_mut_ptr() as _),
                        &mut size,
                        header_size,
                    )
                };
                if r == u32::MAX {
                    panic_from_win32();
                }
                let offset = std::mem::offset_of!(RAWINPUT, data);
                let raw = unsafe { self.buf.get(offset..).unwrap_unchecked() };
                RawData::HID(HID { raw })
            }
            _ => unsafe { std::hint::unreachable_unchecked() },
        };
        Some(data)
    }
}

impl RawInputBufReader {
    #[inline(always)]
    fn header(&self) -> &RAWINPUTHEADER {
        &self.as_raw_input().header
    }

    #[inline(always)]
    fn raw_data(&self) -> &RAWINPUT_0 {
        &self.as_raw_input().data
    }

    #[inline(always)]
    fn as_raw_input(&self) -> &RAWINPUT {
        unsafe { &*(self.buf.as_ptr() as *const RAWINPUT) }
    }
}

pub mod device {
    use windows::Win32::{
        Devices::HumanInterfaceDevice::{
            HID_USAGE_GENERIC_KEYBOARD, HID_USAGE_GENERIC_MOUSE, HID_USAGE_PAGE_GENERIC,
        },
        Foundation::HWND,
        UI::Input::{
            RAWINPUTDEVICE, RAWINPUTDEVICE_FLAGS, RIDEV_INPUTSINK, RIDEV_NOLEGACY, RIDEV_REMOVE,
            RegisterRawInputDevices,
        },
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DeviceType {
        Keyboard,
        Mouse,
    }

    type Flags = RAWINPUTDEVICE_FLAGS;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum OptionType {
        Remove,
        Flags(HWND, Flags),
    }

    impl OptionType {
        pub fn inputsink(hwnd: HWND) -> Self {
            Self::Flags(hwnd, RIDEV_INPUTSINK)
        }

        pub fn inputsink_with_no_legacy(hwnd: HWND) -> Self {
            Self::Flags(hwnd, RIDEV_INPUTSINK | RIDEV_NOLEGACY)
        }
    }

    pub fn register(device_type: DeviceType, option_type: OptionType) {
        let device = match device_type {
            DeviceType::Keyboard => HID_USAGE_GENERIC_KEYBOARD,
            DeviceType::Mouse => HID_USAGE_GENERIC_MOUSE,
        };
        let (hwnd, flags) = match option_type {
            OptionType::Remove => (HWND(std::ptr::null_mut()), RIDEV_REMOVE),
            OptionType::Flags(hwnd, flags) => (hwnd, flags),
        };
        let raw_input_device = RAWINPUTDEVICE {
            usUsagePage: HID_USAGE_PAGE_GENERIC,
            usUsage: device,
            dwFlags: flags,
            hwndTarget: hwnd,
        };
        unsafe {
            RegisterRawInputDevices(
                &[raw_input_device],
                std::mem::size_of::<RAWINPUTDEVICE>() as _,
            )
        }
        .expect("unreachable");
    }
}
