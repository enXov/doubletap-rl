//! Input simulation using direct uinput via raw writes
//!
//! Creates a virtual input device, then writes events using raw write() calls
//! to the file descriptor — exactly like ydotool does.

use evdev::{
    uinput::VirtualDeviceBuilder, AttributeSet, BusType, InputId, Key, RelativeAxisType,
};
use std::os::unix::io::AsRawFd;
use tracing::{debug, info};

use crate::DoubleTapError;

/// Raw input_event struct matching the kernel's struct input_event
#[repr(C)]
struct RawInputEvent {
    tv_sec: libc::time_t,
    tv_usec: libc::suseconds_t,
    r#type: u16,
    code: u16,
    value: i32,
}

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const SYN_REPORT: u16 = 0x00;
const BTN_RIGHT: u16 = 0x111;

/// All keys registered by ydotool — required for the compositor to
/// properly recognize and forward events from this device.
const DEVICE_KEYS: &[Key] = &[
    // Keyboard keys (needed for kbd handler registration)
    Key::KEY_ESC, Key::KEY_1, Key::KEY_2, Key::KEY_3, Key::KEY_4, Key::KEY_5,
    Key::KEY_6, Key::KEY_7, Key::KEY_8, Key::KEY_9, Key::KEY_0, Key::KEY_MINUS,
    Key::KEY_EQUAL, Key::KEY_BACKSPACE, Key::KEY_TAB, Key::KEY_Q, Key::KEY_W,
    Key::KEY_E, Key::KEY_R, Key::KEY_T, Key::KEY_Y, Key::KEY_U, Key::KEY_I,
    Key::KEY_O, Key::KEY_P, Key::KEY_LEFTBRACE, Key::KEY_RIGHTBRACE, Key::KEY_ENTER,
    Key::KEY_LEFTCTRL, Key::KEY_A, Key::KEY_S, Key::KEY_D, Key::KEY_F, Key::KEY_G,
    Key::KEY_H, Key::KEY_J, Key::KEY_K, Key::KEY_L, Key::KEY_SEMICOLON,
    Key::KEY_APOSTROPHE, Key::KEY_GRAVE, Key::KEY_LEFTSHIFT, Key::KEY_BACKSLASH,
    Key::KEY_Z, Key::KEY_X, Key::KEY_C, Key::KEY_V, Key::KEY_B, Key::KEY_N,
    Key::KEY_M, Key::KEY_COMMA, Key::KEY_DOT, Key::KEY_SLASH, Key::KEY_RIGHTSHIFT,
    Key::KEY_KPASTERISK, Key::KEY_LEFTALT, Key::KEY_SPACE, Key::KEY_CAPSLOCK,
    Key::KEY_F1, Key::KEY_F2, Key::KEY_F3, Key::KEY_F4, Key::KEY_F5, Key::KEY_F6,
    Key::KEY_F7, Key::KEY_F8, Key::KEY_F9, Key::KEY_F10, Key::KEY_NUMLOCK,
    Key::KEY_SCROLLLOCK, Key::KEY_KP7, Key::KEY_KP8, Key::KEY_KP9, Key::KEY_KPMINUS,
    Key::KEY_KP4, Key::KEY_KP5, Key::KEY_KP6, Key::KEY_KPPLUS, Key::KEY_KP1,
    Key::KEY_KP2, Key::KEY_KP3, Key::KEY_KP0, Key::KEY_KPDOT,
    Key::KEY_F11, Key::KEY_F12,
    Key::KEY_KPENTER, Key::KEY_RIGHTCTRL, Key::KEY_KPSLASH, Key::KEY_SYSRQ,
    Key::KEY_RIGHTALT, Key::KEY_HOME, Key::KEY_UP, Key::KEY_PAGEUP, Key::KEY_LEFT,
    Key::KEY_RIGHT, Key::KEY_END, Key::KEY_DOWN, Key::KEY_PAGEDOWN, Key::KEY_INSERT,
    Key::KEY_DELETE, Key::KEY_MUTE, Key::KEY_VOLUMEDOWN, Key::KEY_VOLUMEUP,
    Key::KEY_POWER, Key::KEY_PAUSE, Key::KEY_LEFTMETA, Key::KEY_RIGHTMETA,
    Key::KEY_COMPOSE, Key::KEY_STOP, Key::KEY_MENU, Key::KEY_CALC, Key::KEY_SLEEP,
    Key::KEY_WAKEUP, Key::KEY_MAIL, Key::KEY_BOOKMARKS, Key::KEY_BACK,
    Key::KEY_FORWARD, Key::KEY_NEXTSONG, Key::KEY_PLAYPAUSE, Key::KEY_PREVIOUSSONG,
    Key::KEY_STOPCD, Key::KEY_HOMEPAGE, Key::KEY_REFRESH, Key::KEY_SEARCH, Key::KEY_FN,
    // Mouse buttons
    Key::BTN_LEFT, Key::BTN_RIGHT, Key::BTN_MIDDLE, Key::BTN_SIDE, Key::BTN_EXTRA,
    Key::BTN_FORWARD, Key::BTN_BACK, Key::BTN_TASK,
];

/// Input simulator using raw writes to uinput fd
pub struct InputSimulator {
    fd: std::os::unix::io::RawFd,
    _device: evdev::uinput::VirtualDevice,
}

impl InputSimulator {
    pub fn new() -> Result<Self, DoubleTapError> {
        let mut keys = AttributeSet::<Key>::new();
        for key in DEVICE_KEYS {
            keys.insert(*key);
        }

        let mut rel_axes = AttributeSet::<RelativeAxisType>::new();
        rel_axes.insert(RelativeAxisType::REL_X);
        rel_axes.insert(RelativeAxisType::REL_Y);
        rel_axes.insert(RelativeAxisType::REL_Z);
        rel_axes.insert(RelativeAxisType::REL_WHEEL);
        rel_axes.insert(RelativeAxisType::REL_HWHEEL);

        let id = InputId::new(BusType::BUS_VIRTUAL, 0x2333, 0x6666, 1);

        let mut device = VirtualDeviceBuilder::new()
            .map_err(|e| DoubleTapError::VirtualDevice(format!("Builder: {e}")))?
            .name("DoubleTap-RL Virtual Mouse")
            .input_id(id)
            .with_keys(&keys)
            .map_err(|e| DoubleTapError::VirtualDevice(format!("Keys: {e}")))?
            .with_relative_axes(&rel_axes)
            .map_err(|e| DoubleTapError::VirtualDevice(format!("Axes: {e}")))?
            .build()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    DoubleTapError::PermissionDenied
                } else {
                    DoubleTapError::VirtualDevice(format!("Build: {e}"))
                }
            })?;

        let fd = device.as_raw_fd();

        if let Some(Ok(path)) = device
            .enumerate_dev_nodes_blocking()
            .ok()
            .and_then(|mut iter| iter.next())
        {
            info!("Virtual device: {:?}", path);
        }

        debug!("Waiting for device registration...");
        std::thread::sleep(std::time::Duration::from_secs(1));
        info!("Virtual input device ready");

        Ok(Self { fd, _device: device })
    }

    fn write_event(&self, event_type: u16, code: u16, value: i32) -> Result<(), DoubleTapError> {
        let event = RawInputEvent {
            tv_sec: 0,
            tv_usec: 0,
            r#type: event_type,
            code,
            value,
        };
        let ptr = &event as *const RawInputEvent as *const libc::c_void;
        let size = std::mem::size_of::<RawInputEvent>();
        let ret = unsafe { libc::write(self.fd, ptr, size) };
        if ret < 0 {
            return Err(DoubleTapError::SendEvent(
                std::io::Error::last_os_error().to_string(),
            ));
        }
        Ok(())
    }

    /// Send a right-click (press + sync + release + sync)
    pub fn send_right_click(&mut self) -> Result<(), DoubleTapError> {
        self.write_event(EV_KEY, BTN_RIGHT, 1)?;
        self.write_event(EV_SYN, SYN_REPORT, 0)?;
        self.write_event(EV_KEY, BTN_RIGHT, 0)?;
        self.write_event(EV_SYN, SYN_REPORT, 0)?;
        Ok(())
    }
}
