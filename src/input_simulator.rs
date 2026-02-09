//! Input simulation using ydotool
//!
//! Uses ydotool to send mouse and keyboard events via uinput at the kernel level.
//! Works on Wayland by bypassing the display server entirely.
//! Requires ydotoold daemon to be running: sudo systemctl enable --now ydotoold

use rdev::Key;
use std::process::Command;
use std::thread;
use std::time::Duration;
use tracing::{debug, error, info};

use crate::input_listener::RecordedKeyEvent;
use crate::DoubleTapError;

/// Get the ydotool socket path
fn get_socket_path() -> String {
    let uid = unsafe { libc::getuid() };
    format!("/run/user/{}/.ydotool_socket", uid)
}

/// Convert rdev Key to ydotool key code
/// These are Linux evdev key codes
fn key_to_code(key: Key) -> Option<u32> {
    match key {
        Key::KeyW => Some(17),        // KEY_W
        Key::KeyA => Some(30),        // KEY_A
        Key::KeyS => Some(31),        // KEY_S
        Key::KeyD => Some(32),        // KEY_D
        Key::ShiftLeft => Some(42),   // KEY_LEFTSHIFT
        _ => None,
    }
}

/// Input simulator that sends synthetic inputs via ydotool
pub struct InputSimulator {
    socket_path: String,
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
        Ok(Self {
            socket_path: get_socket_path(),
        })
    }

    /// Run a ydotool command with the socket path set
    fn run_ydotool(&self, args: &[&str]) -> Result<(), DoubleTapError> {
        let args_str = args.join(" ");
        let cmd = format!("YDOTOOL_SOCKET={} ydotool {}", self.socket_path, args_str);
        
        let output = Command::new("sh")
            .args(["-c", &cmd])
            .output()
            .map_err(|e| DoubleTapError::SendEvent(format!("Failed to run ydotool: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DoubleTapError::SendEvent(format!("ydotool failed: {}", stderr)));
        }

        Ok(())
    }

    /// Send a right-click event (press and release)
    pub fn send_right_click(&mut self) -> Result<(), DoubleTapError> {
        debug!("Sending right-click via ydotool");
        self.run_ydotool(&["click", "0xC1"])?;
        debug!("Right-click sent successfully");
        Ok(())
    }

    /// Send just a right-click press (no release)
    pub fn send_right_press(&mut self) -> Result<(), DoubleTapError> {
        debug!("Sending right-click press via ydotool");
        self.run_ydotool(&["click", "-D", "0xC1"])
    }

    /// Send just a right-click release
    pub fn send_right_release(&mut self) -> Result<(), DoubleTapError> {
        debug!("Sending right-click release via ydotool");
        self.run_ydotool(&["click", "-U", "0xC1"])
    }

    /// Send a key press event
    pub fn send_key_press(&mut self, key: Key) -> Result<(), DoubleTapError> {
        if let Some(code) = key_to_code(key) {
            debug!("Sending {:?} key press via ydotool (code {})", key, code);
            // ydotool key format: keycode:down
            let key_arg = format!("{}:1", code);
            self.run_ydotool(&["key", &key_arg])
        } else {
            debug!("Unknown key {:?}, skipping", key);
            Ok(())
        }
    }

    /// Send a key release event
    pub fn send_key_release(&mut self, key: Key) -> Result<(), DoubleTapError> {
        if let Some(code) = key_to_code(key) {
            debug!("Sending {:?} key release via ydotool (code {})", key, code);
            // ydotool key format: keycode:up
            let key_arg = format!("{}:0", code);
            self.run_ydotool(&["key", &key_arg])
        } else {
            debug!("Unknown key {:?}, skipping", key);
            Ok(())
        }
    }

    /// Replay recorded key events with proper timing
    /// This spawns a background thread to handle the timing
    pub fn replay_recorded_keys(&mut self, events: Vec<RecordedKeyEvent>) {
        if events.is_empty() {
            return;
        }

        info!("Starting playback of {} recorded events", events.len());
        
        let socket_path = self.socket_path.clone();
        
        thread::spawn(move || {
            let mut last_offset = 0u64;
            
            for event in events {
                // Wait for the correct timing
                if event.offset_ms > last_offset {
                    let wait_ms = event.offset_ms - last_offset;
                    thread::sleep(Duration::from_millis(wait_ms));
                }
                last_offset = event.offset_ms;
                
                // Send the key event
                if let Some(code) = key_to_code(event.key) {
                    let key_arg = if event.is_press {
                        format!("{}:1", code)
                    } else {
                        format!("{}:0", code)
                    };
                    
                    let cmd = format!("YDOTOOL_SOCKET={} ydotool key {}", socket_path, key_arg);
                    
                    if let Err(e) = Command::new("sh")
                        .args(["-c", &cmd])
                        .output()
                    {
                        error!("Failed to replay key event: {}", e);
                    } else {
                        debug!("Replayed {:?} {} at +{}ms", 
                            event.key, 
                            if event.is_press { "press" } else { "release" },
                            event.offset_ms
                        );
                    }
                }
            }
            
            info!("Playback complete");
        });
    }
}

