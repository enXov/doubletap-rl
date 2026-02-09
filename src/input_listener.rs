//! Global input listening using rdev with macro recording

use rdev::{grab, Button, Event, EventType, Key};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Instant;
use tracing::{error, info};

/// Minimum time between auto-clicks in milliseconds
/// This prevents feedback loops from ydotool-generated events
const MIN_CLICK_INTERVAL_MS: u64 = 100;

/// Timestamp of last auto-click we triggered (in millis since program start)
static LAST_AUTO_CLICK_MS: AtomicU64 = AtomicU64::new(0);

/// Flag to block keys while waiting for auto-click
static BLOCKING_KEYS: AtomicBool = AtomicBool::new(false);

/// Program start time - initialized once, thread-safe
static PROGRAM_START: OnceLock<Instant> = OnceLock::new();

/// Recording start time (in millis since program start)
static RECORDING_START_MS: AtomicU64 = AtomicU64::new(0);

/// Buffer for recorded key events
static RECORDED_KEYS: OnceLock<Mutex<Vec<RecordedKeyEvent>>> = OnceLock::new();

/// Track which keys are currently held (pressed but not released)
static HELD_KEYS: OnceLock<Mutex<Vec<Key>>> = OnceLock::new();

/// A recorded key event with timing information
#[derive(Debug, Clone)]
pub struct RecordedKeyEvent {
    /// The key that was pressed/released
    pub key: Key,
    /// Whether this is a press (true) or release (false)
    pub is_press: bool,
    /// Time offset in milliseconds from recording start
    pub offset_ms: u64,
}

/// Get the recorded keys buffer
fn get_recorded_keys() -> &'static Mutex<Vec<RecordedKeyEvent>> {
    RECORDED_KEYS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Get the held keys buffer
fn get_held_keys() -> &'static Mutex<Vec<Key>> {
    HELD_KEYS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Get current time in millis since program start
fn now_ms() -> u64 {
    let start = PROGRAM_START.get_or_init(Instant::now);
    start.elapsed().as_millis() as u64
}

/// Mark that we just sent an auto-click
/// Call this AFTER sending the auto-click successfully
/// This also unblocks keys
pub fn mark_auto_click_sent() {
    let now = now_ms();
    LAST_AUTO_CLICK_MS.store(now, Ordering::SeqCst);
    // Unblock keys now that auto-click is complete
    BLOCKING_KEYS.store(false, Ordering::SeqCst);
    info!("Marked auto-click at {}ms, keys unblocked", now);
}

/// Start blocking keys and begin recording (called when right-click press is detected)
pub fn start_blocking_and_recording() {
    // If we're already blocking, don't restart - this prevents auto-click from clearing recording
    if BLOCKING_KEYS.load(Ordering::SeqCst) {
        return;
    }
    // Clear any previous recording
    if let Ok(mut keys) = get_recorded_keys().lock() {
        keys.clear();
    }
    // Clear held keys tracker
    if let Ok(mut held) = get_held_keys().lock() {
        held.clear();
    }
    // Mark recording start time
    RECORDING_START_MS.store(now_ms(), Ordering::SeqCst);
    // Enable blocking
    BLOCKING_KEYS.store(true, Ordering::SeqCst);
    info!("Keys blocked, recording started");
}

/// Check if keys should be blocked
pub fn is_blocking_keys() -> bool {
    BLOCKING_KEYS.load(Ordering::SeqCst)
}

/// Get the recorded events without stopping blocking yet
/// Call this BEFORE mark_auto_click_sent() to get the recording before it could be cleared
/// 
/// For keys still held when recording ends:
/// - WASD: Add synthetic release (so key doesn't stay pressed)
/// - Shift: REMOVE the press event (user is still holding, so don't replay it)
pub fn get_recording() -> Vec<RecordedKeyEvent> {
    let recording_start = RECORDING_START_MS.load(Ordering::SeqCst);
    let end_offset = now_ms().saturating_sub(recording_start);
    
    let mut events = if let Ok(mut keys) = get_recorded_keys().lock() {
        std::mem::take(&mut *keys)
    } else {
        Vec::new()
    };
    
    // Handle keys still held at end of recording
    if let Ok(mut held) = get_held_keys().lock() {
        for key in held.drain(..) {
            if key == Key::ShiftLeft {
                // For shift: REMOVE the press event - user is still holding physically
                // so we don't want to replay it (would conflict with physical hold)
                let before_len = events.len();
                events.retain(|e| !(e.key == Key::ShiftLeft && e.is_press));
                let removed = before_len - events.len();
                if removed > 0 {
                    info!("Removed {} ShiftLeft press events (user still holding)", removed);
                }
            } else {
                // For WASD: Add synthetic release so key doesn't stay pressed
                info!("Adding synthetic release for {:?} at +{}ms", key, end_offset);
                events.push(RecordedKeyEvent {
                    key,
                    is_press: false,
                    offset_ms: end_offset,
                });
            }
        }
    }
    
    info!("Got {} recorded events for playback", events.len());
    events
}

/// Stop blocking keys without returning recording (use when auto-click fails or window not focused)
pub fn stop_blocking_keys() {
    BLOCKING_KEYS.store(false, Ordering::SeqCst);
    // Clear recording since we won't use it
    if let Ok(mut keys) = get_recorded_keys().lock() {
        keys.clear();
    }
    // Clear held keys tracker
    if let Ok(mut held) = get_held_keys().lock() {
        held.clear();
    }
    info!("Keys unblocked, recording discarded");
}

/// Record a key event and track held state
fn record_key_event(key: Key, is_press: bool) {
    let recording_start = RECORDING_START_MS.load(Ordering::SeqCst);
    let offset_ms = now_ms().saturating_sub(recording_start);
    
    // Track held keys
    if let Ok(mut held) = get_held_keys().lock() {
        if is_press {
            // Add to held keys if not already there
            if !held.contains(&key) {
                held.push(key);
            }
        } else {
            // Remove from held keys
            held.retain(|k| *k != key);
        }
    }
    
    if let Ok(mut keys) = get_recorded_keys().lock() {
        keys.push(RecordedKeyEvent {
            key,
            is_press,
            offset_ms,
        });
        info!("Recorded {:?} {} at +{}ms", key, if is_press { "press" } else { "release" }, offset_ms);
    }
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
        info!("Ignoring event ({}ms since last auto-click, need {}ms)", elapsed, MIN_CLICK_INTERVAL_MS);
        return true;
    }
    
    false
}

/// Check if the key is a WASD key (these are BLOCKED during recording)
fn is_wasd_key(key: Key) -> bool {
    matches!(
        key,
        Key::KeyW | Key::KeyA | Key::KeyS | Key::KeyD
    )
}

/// Check if the key should be recorded (WASD + Left Shift)
/// Note: ShiftLeft is recorded but NOT blocked (passes through to game)
#[allow(dead_code)]
fn is_blocked_key(key: Key) -> bool {
    matches!(
        key,
        Key::KeyW | Key::KeyA | Key::KeyS | Key::KeyD | Key::ShiftLeft
    )
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
    /// Focus state to check if target window is focused
    focus_state: std::sync::Arc<crate::FocusState>,
}

impl InputListener {
    /// Create a new InputListener with the given channel sender and focus state
    pub fn new(sender: mpsc::Sender<RightClickEvent>, focus_state: std::sync::Arc<crate::FocusState>) -> Self {
        Self { sender, focus_state }
    }

    /// Start listening for input events in a background thread
    ///
    /// This function spawns a new thread that grabs global input events.
    /// When a right-click press is detected, keys are blocked and recording starts.
    /// When right-click release is detected, it sends an event through the channel.
    /// After auto-click is sent, keys are unblocked and recording is played back.
    ///
    /// Returns a JoinHandle for the listener thread.
    pub fn start(self) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            info!("Input listener started (with macro recording)");

            let sender = self.sender;
            let focus_state = self.focus_state;

            let callback = move |event: Event| -> Option<Event> {
                match event.event_type {
                    // When right-click is pressed, start blocking and recording (only if focused)
                    EventType::ButtonPress(Button::Right) => {
                        if !should_ignore_event() && focus_state.is_focused() {
                            start_blocking_and_recording();
                        }
                        Some(event) // Pass through the right-click
                    }
                    
                    // When right-click is released, send the event for auto-click (only if focused)
                    EventType::ButtonRelease(Button::Right) => {
                        if !should_ignore_event() && focus_state.is_focused() {
                            info!("Right-click release detected");

                            let click_event = RightClickEvent {
                                timestamp: std::time::Instant::now(),
                            };

                            if let Err(e) = sender.send(click_event) {
                                error!("Failed to send click event: {}", e);
                            }
                        }
                        Some(event) // Pass through the right-click release
                    }
                    
                    // ShiftLeft: Record but PASS THROUGH (only when blocking AND focused)
                    EventType::KeyPress(Key::ShiftLeft) if is_blocking_keys() && focus_state.is_focused() => {
                        record_key_event(Key::ShiftLeft, true);
                        Some(event) // Pass through - don't block shift!
                    }
                    
                    EventType::KeyRelease(Key::ShiftLeft) if is_blocking_keys() && focus_state.is_focused() => {
                        record_key_event(Key::ShiftLeft, false);
                        Some(event) // Pass through - don't block shift!
                    }
                    
                    // WASD: Block and record key PRESSES (only when blocking AND focused)
                    EventType::KeyPress(key) if is_wasd_key(key) && is_blocking_keys() && focus_state.is_focused() => {
                        record_key_event(key, true);
                        None // Block the key press event
                    }
                    
                    // WASD: Block and record key RELEASES (only when blocking AND focused)
                    EventType::KeyRelease(key) if is_wasd_key(key) && is_blocking_keys() && focus_state.is_focused() => {
                        record_key_event(key, false);
                        None // Block the key release event
                    }
                    
                    // Pass through all other events
                    _ => Some(event),
                }
            };

            if let Err(e) = grab(callback) {
                error!("Error in input listener: {:?}\nMake sure you have permission to read input devices (add user to 'input' group)", e);
            }
        })
    }
}

/// Create a channel for input events and return both ends
pub fn create_event_channel() -> (mpsc::Sender<RightClickEvent>, mpsc::Receiver<RightClickEvent>) {
    mpsc::channel()
}

