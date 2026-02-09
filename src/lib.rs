//! DoubleTap-RL - Auto-clicker for Rocket League double-tap aerials
//!
//! This library provides components for:
//! - Global input listening (right-click detection)
//! - Input simulation (sending synthetic clicks)
//! - Focus detection (window/process-based)

pub mod focus_detector;
pub mod input_listener;
pub mod input_simulator;

pub use focus_detector::{create_focus_detector, FocusDetector, FocusState, start_focus_poller};
pub use input_listener::InputListener;
pub use input_simulator::InputSimulator;

use thiserror::Error;

/// Main error type for DoubleTap-RL
#[derive(Error, Debug)]
pub enum DoubleTapError {
    #[error("Failed to access input devices: {0}")]
    InputAccess(String),

    #[error("Failed to create virtual device: {0}")]
    VirtualDevice(String),

    #[error("Failed to send input event: {0}")]
    SendEvent(String),

    #[error("Failed to detect window focus: {0}")]
    FocusDetection(String),

    #[error("Permission denied - add user to 'input' group")]
    PermissionDenied,

    #[error("Channel error: {0}")]
    Channel(String),
}
