//! Input simulation using mouse-keyboard-input (uinput)

use mouse_keyboard_input::VirtualDevice;
use std::thread;
use std::time::Duration;
use tracing::{debug, info};

use crate::DoubleTapError;

/// Button code for right mouse button (from linux/input-event-codes.h)
const BTN_RIGHT: u16 = 0x111;

/// Input simulator that sends synthetic mouse events via uinput
pub struct InputSimulator {
    device: VirtualDevice,
}

impl InputSimulator {
    /// Create a new InputSimulator with a virtual device
    ///
    /// Requires user to be in the 'input' group or running as root.
    pub fn new() -> Result<Self, DoubleTapError> {
        info!("Creating virtual input device...");

        // VirtualDevice::new() panics on failure, so we can't easily catch errors
        // In a production app, we'd want to check /dev/uinput access first
        let device = VirtualDevice::new();

        info!("Virtual input device created successfully");
        Ok(Self { device })
    }

    /// Send a right-click event (press and release)
    ///
    /// Optionally waits for the specified delay before sending.
    pub fn send_right_click(&mut self, delay_ms: u64) -> Result<(), DoubleTapError> {
        if delay_ms > 0 {
            debug!("Waiting {}ms before sending click", delay_ms);
            thread::sleep(Duration::from_millis(delay_ms));
        }

        debug!("Sending right-click press");
        self.device
            .click(BTN_RIGHT)
            .map_err(|e| DoubleTapError::SendEvent(e.to_string()))?;

        debug!("Right-click sent successfully");
        Ok(())
    }

    /// Send just a right-click press (no release)
    pub fn send_right_press(&mut self) -> Result<(), DoubleTapError> {
        debug!("Sending right-click press");
        self.device
            .press(BTN_RIGHT)
            .map_err(|e| DoubleTapError::SendEvent(e.to_string()))?;
        Ok(())
    }

    /// Send just a right-click release
    pub fn send_right_release(&mut self) -> Result<(), DoubleTapError> {
        debug!("Sending right-click release");
        self.device
            .release(BTN_RIGHT)
            .map_err(|e| DoubleTapError::SendEvent(e.to_string()))?;
        Ok(())
    }
}
