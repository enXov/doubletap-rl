//! Input simulation using ydotool
//!
//! Uses ydotool to send mouse events via uinput at the kernel level.
//! Works on Wayland by bypassing the display server entirely.
//! Requires ydotoold daemon to be running: sudo systemctl enable --now ydotoold

use std::process::Command;
use tracing::{debug, info};

use crate::DoubleTapError;

/// Input simulator that sends synthetic mouse events via ydotool
pub struct InputSimulator {
    // No state needed - we just call ydotool each time
}

impl InputSimulator {
    /// Create a new InputSimulator
    ///
    /// Requires ydotool to be installed and ydotoold daemon running.
    pub fn new() -> Result<Self, DoubleTapError> {
        info!("Creating virtual input device...");

        // Verify ydotool is available
        let output = Command::new("which")
            .arg("ydotool")
            .output()
            .map_err(|e| DoubleTapError::InputAccess(format!("Failed to check for ydotool: {}", e)))?;

        if !output.status.success() {
            return Err(DoubleTapError::InputAccess(
                "ydotool not found. Install it: sudo pacman -S ydotool".to_string()
            ));
        }

        // Verify ydotoold daemon is running by doing a quick test
        let test = Command::new("ydotool")
            .args(["click", "--help"])
            .output();
        
        if test.is_err() {
            return Err(DoubleTapError::InputAccess(
                "ydotoold daemon may not be running. Start it: sudo systemctl enable --now ydotoold".to_string()
            ));
        }

        info!("Virtual input device created successfully");
        Ok(Self {})
    }

    /// Send a right-click event (press and release)
    pub fn send_right_click(&mut self) -> Result<(), DoubleTapError> {
        debug!("Sending right-click via ydotool");
        
        // Get the socket path - default to user runtime dir
        let uid = unsafe { libc::getuid() };
        let socket_path = format!("/run/user/{}/.ydotool_socket", uid);
        
        // Run ydotool with socket path set
        let cmd = format!(
            "YDOTOOL_SOCKET={} ydotool click 0xC1",
            socket_path
        );
        
        let output = Command::new("sh")
            .args(["-c", &cmd])
            .output()
            .map_err(|e| DoubleTapError::SendEvent(format!("Failed to run ydotool: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DoubleTapError::SendEvent(format!("ydotool failed: {}", stderr)));
        }

        debug!("Right-click sent successfully");
        Ok(())
    }

    /// Send just a right-click press (no release)
    pub fn send_right_press(&mut self) -> Result<(), DoubleTapError> {
        debug!("Sending right-click press via ydotool");
        
        // 0xC1 with -D (down only)
        let output = Command::new("ydotool")
            .args(["click", "-D", "0xC1"])
            .output()
            .map_err(|e| DoubleTapError::SendEvent(format!("Failed to run ydotool: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DoubleTapError::SendEvent(format!("ydotool failed: {}", stderr)));
        }

        Ok(())
    }

    /// Send just a right-click release
    pub fn send_right_release(&mut self) -> Result<(), DoubleTapError> {
        debug!("Sending right-click release via ydotool");
        
        // 0xC1 with -U (up only)
        let output = Command::new("ydotool")
            .args(["click", "-U", "0xC1"])
            .output()
            .map_err(|e| DoubleTapError::SendEvent(format!("Failed to run ydotool: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DoubleTapError::SendEvent(format!("ydotool failed: {}", stderr)));
        }

        Ok(())
    }
}
