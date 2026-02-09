//! Global input listening using rdev

use rdev::{listen, Button, Event, EventType};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::OnceLock;
use std::thread;
use std::time::Instant;
use tracing::error;

/// Minimum time between auto-clicks in milliseconds
/// This prevents feedback loops from our own simulated events
const MIN_CLICK_INTERVAL_MS: u64 = 100;

/// Timestamp of last auto-click we triggered (in millis since program start)
static LAST_AUTO_CLICK_MS: AtomicU64 = AtomicU64::new(0);

/// Program start time - initialized once, thread-safe
static PROGRAM_START: OnceLock<Instant> = OnceLock::new();

/// Get current time in millis since program start
fn now_ms() -> u64 {
    let start = PROGRAM_START.get_or_init(Instant::now);
    start.elapsed().as_millis() as u64
}

/// Mark that we just sent an auto-click
/// Call this AFTER sending the auto-click successfully
pub fn mark_auto_click_sent() {
    let now = now_ms();
    LAST_AUTO_CLICK_MS.store(now, Ordering::SeqCst);
}

/// Check if enough time has passed since last auto-click
/// Returns true if this event should be ignored (too soon after our auto-click)
fn should_ignore_event() -> bool {
    let last_click = LAST_AUTO_CLICK_MS.load(Ordering::SeqCst);
    
    // If we've never sent an auto-click, don't ignore
    if last_click == 0 {
        return false;
    }
    
    let now = now_ms();
    let elapsed = now.saturating_sub(last_click);
    
    if elapsed < MIN_CLICK_INTERVAL_MS {
        return true;
    }
    
    false
}

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
            let sender = self.sender;

            let callback = move |event: Event| {
                // Trigger on button RELEASE - this ensures user's button is up
                // before we send our simulated click (avoids button state conflict)
                if let EventType::ButtonRelease(Button::Right) = event.event_type {
                    // Check if this might be our own auto-click event
                    if should_ignore_event() {
                        return;
                    }
                    let click_event = RightClickEvent {
                        timestamp: std::time::Instant::now(),
                    };

                    if let Err(e) = sender.send(click_event) {
                        error!("Failed to send click event: {}", e);
                    }
                }
            };

            if let Err(e) = listen(callback) {
                error!("Error in input listener: {:?}\nMake sure you have permission to read input devices (add user to 'input' group)", e);
            }
        })
    }
}

/// Create a channel for input events and return both ends
pub fn create_event_channel() -> (mpsc::Sender<RightClickEvent>, mpsc::Receiver<RightClickEvent>) {
    mpsc::channel()
}
