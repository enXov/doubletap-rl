//! Global input listening using rdev

use rdev::{listen, Button, Event, EventType};
use std::sync::mpsc;
use std::thread;
use tracing::{debug, error, info};


/// Event sent when right-click is detected
#[derive(Debug, Clone)]
pub struct RightClickEvent {
    /// Timestamp when the click was detected
    pub timestamp: std::time::Instant,
}

/// Input listener that captures global mouse events
pub struct InputListener {
    /// Sender for click events
    sender: mpsc::Sender<RightClickEvent>,
}

impl InputListener {
    /// Create a new InputListener with the given channel sender
    pub fn new(sender: mpsc::Sender<RightClickEvent>) -> Self {
        Self { sender }
    }

    /// Start listening for input events in a background thread
    ///
    /// This function spawns a new thread that listens for global mouse events.
    /// When a right-click press is detected, it sends an event through the channel.
    ///
    /// Returns a JoinHandle for the listener thread.
    pub fn start(self) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            info!("Input listener started");

            let sender = self.sender;

            let callback = move |event: Event| {
                if let EventType::ButtonPress(Button::Right) = event.event_type {
                    debug!("Right click detected!");

                    let click_event = RightClickEvent {
                        timestamp: std::time::Instant::now(),
                    };

                    if let Err(e) = sender.send(click_event) {
                        error!("Failed to send click event: {}", e);
                    }
                }
            };

            if let Err(e) = listen(callback) {
                error!("Error in input listener: {:?}", e);
            }
        })
    }
}

/// Create a channel for input events and return both ends
pub fn create_event_channel() -> (mpsc::Sender<RightClickEvent>, mpsc::Receiver<RightClickEvent>) {
    mpsc::channel()
}
