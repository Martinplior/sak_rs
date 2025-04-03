use std::mem::MaybeUninit;

use windows::Win32::UI::{
    Input::{
        GetRawInputData, HRAWINPUT, KeyboardAndMouse::VIRTUAL_KEY, MOUSE_ATTRIBUTES_CHANGED,
        MOUSE_MOVE_NOCOALESCE, MOUSE_VIRTUAL_DESKTOP, RAWHID, RAWINPUT, RAWINPUTHEADER,
        RAWKEYBOARD, RAWMOUSE, RAWMOUSE_0_0, RID_DEVICE_INFO_TYPE, RID_INPUT, RIM_TYPEHID,
        RIM_TYPEKEYBOARD, RIM_TYPEMOUSE,
    },
    WindowsAndMessaging::{
        MSG, RI_KEY_E0, RI_KEY_E1, RI_MOUSE_BUTTON_4_DOWN, RI_MOUSE_BUTTON_4_UP,
        RI_MOUSE_BUTTON_5_DOWN, RI_MOUSE_BUTTON_5_UP, RI_MOUSE_HWHEEL, RI_MOUSE_LEFT_BUTTON_DOWN,
        RI_MOUSE_LEFT_BUTTON_UP, RI_MOUSE_MIDDLE_BUTTON_DOWN, RI_MOUSE_MIDDLE_BUTTON_UP,
        RI_MOUSE_RIGHT_BUTTON_DOWN, RI_MOUSE_RIGHT_BUTTON_UP, RI_MOUSE_WHEEL,
    },
};

use crate::os::windows::panic_from_win32;

/// [see also](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-rawkeyboard)
#[repr(C)]
#[derive(Clone)]
pub struct Keyboard {
    pub header: RAWINPUTHEADER,
    pub data: RAWKEYBOARD,
}

impl Keyboard {
    #[inline]
    pub fn make_code(&self) -> u16 {
        self.data.MakeCode
    }

    #[inline]
    pub fn virtual_key(&self) -> VIRTUAL_KEY {
        VIRTUAL_KEY(self.data.VKey)
    }

    #[inline]
    pub fn message(&self) -> u32 {
        self.data.Message
    }

    #[inline]
    pub fn extra_information(&self) -> u32 {
        self.data.ExtraInformation
    }

    #[inline]
    pub fn key_is_down(&self) -> bool {
        self.data.Flags & 1 == 0
    }

    #[inline]
    pub fn key_is_up(&self) -> bool {
        !self.key_is_down()
    }

    #[inline]
    pub fn has_e0(&self) -> bool {
        self.data.Flags & (RI_KEY_E0 as u16) != 0
    }

    #[inline]
    pub fn has_e1(&self) -> bool {
        self.data.Flags & (RI_KEY_E1 as u16) != 0
    }
}

/// [see also](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-rawmouse)
#[repr(C)]
#[derive(Clone)]
pub struct Mouse {
    pub header: RAWINPUTHEADER,
    pub data: RAWMOUSE,
}

impl Mouse {
    #[inline]
    pub fn is_move_relative(&self) -> bool {
        self.data.usFlags.0 & 1 == 0
    }

    #[inline]
    pub fn is_move_absolute(&self) -> bool {
        !self.is_move_relative()
    }

    #[inline]
    pub fn is_virtual_desktop(&self) -> bool {
        self.data.usFlags.0 & MOUSE_VIRTUAL_DESKTOP.0 == 0
    }

    #[inline]
    pub fn is_attributes_changed(&self) -> bool {
        self.data.usFlags.0 & MOUSE_ATTRIBUTES_CHANGED.0 == 0
    }

    #[inline]
    pub fn is_move_nocoalesce(&self) -> bool {
        self.data.usFlags.0 & MOUSE_MOVE_NOCOALESCE.0 == 0
    }

    #[inline]
    pub fn last_x(&self) -> i32 {
        self.data.lLastX
    }

    #[inline]
    pub fn last_y(&self) -> i32 {
        self.data.lLastY
    }

    #[inline]
    pub fn extra_information(&self) -> u32 {
        self.data.ulExtraInformation
    }

    #[inline]
    pub fn flags_and_data(&self) -> &RAWMOUSE_0_0 {
        unsafe { &self.data.Anonymous.Anonymous }
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

    #[inline]
    pub fn is_wheel(&self) -> bool {
        self.flags_and_data().usButtonFlags & (RI_MOUSE_WHEEL as u16) != 0
    }

    #[inline]
    pub fn is_horizontal_wheel(&self) -> bool {
        self.flags_and_data().usButtonFlags & (RI_MOUSE_HWHEEL as u16) != 0
    }
}

/// [see also](https://docs.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-rawhid)
#[repr(C)]
#[derive(Clone)]
pub struct HID {
    pub header: RAWINPUTHEADER,
    pub data: RAWHID,
}

impl HID {
    #[inline]
    pub fn per_input_size(&self) -> u32 {
        self.data.dwSizeHid
    }

    #[inline]
    pub fn count(&self) -> u32 {
        self.data.dwCount
    }
}

/// [see also](https://docs.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-rawinput)
#[derive(Clone)]
pub enum RawInput {
    Keyboard(Keyboard),
    Mouse(Mouse),
    HID(HID),
}

impl RawInput {
    pub fn from_msg(msg: &MSG) -> Self {
        let RAWINPUT { header, data } = {
            let mut raw_input = MaybeUninit::<RAWINPUT>::uninit();
            let mut size = std::mem::size_of::<RAWINPUT>() as _;
            let header_size = std::mem::size_of::<RAWINPUTHEADER>() as _;
            let r = unsafe {
                GetRawInputData(
                    HRAWINPUT(msg.lParam.0 as _),
                    RID_INPUT,
                    Some(raw_input.as_mut_ptr() as _),
                    &mut size,
                    header_size,
                )
            };
            if r == u32::MAX {
                panic_from_win32();
            }
            unsafe { raw_input.assume_init() }
        };
        match RID_DEVICE_INFO_TYPE(header.dwType) {
            RIM_TYPEKEYBOARD => Self::Keyboard(Keyboard {
                header,
                data: unsafe { data.keyboard },
            }),
            RIM_TYPEMOUSE => Self::Mouse(Mouse {
                header,
                data: unsafe { data.mouse },
            }),
            RIM_TYPEHID => Self::HID(HID {
                header,
                data: unsafe { data.hid },
            }),
            _ => unsafe { std::hint::unreachable_unchecked() },
        }
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
