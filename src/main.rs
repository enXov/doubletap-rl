//! DoubleTap-RL - Auto-clicker for Rocket League double-tap aerials
//!
//! This program automatically sends a second right-click after detecting
//! the user's right-click, helping with double-tap aerial mechanics.

use doubletap_rl::{
    create_focus_detector,
    input_listener::{create_event_channel, InputListener},
    start_focus_poller, DoubleTapError, FocusState, InputSimulator,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

/// Target window name - hardcoded for Rocket League
const TARGET_WINDOW: &str = "Rocket League (64-bit, DX11, Cooked)";

fn main() -> Result<(), DoubleTapError> {
    // Initialize logging
    let _subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .compact()
        .init();

    info!("DoubleTap-RL starting...");
    info!("Target window: '{}'", TARGET_WINDOW);

    // Set up Ctrl+C handler for graceful shutdown
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    ctrlc::set_handler(move || {
        info!("Shutdown signal received");
        running_clone.store(false, Ordering::SeqCst);
    })
    .expect("Failed to set Ctrl+C handler");

    // Create input simulator
    let mut simulator = match InputSimulator::new() {
        Ok(sim) => sim,
        Err(DoubleTapError::PermissionDenied) => {
            error!("Permission denied. Please add your user to the 'input' group:");
            error!("  sudo usermod -aG input $USER");
            error!("Then logout and login again.");
            return Err(DoubleTapError::PermissionDenied);
        }
        Err(e) => return Err(e),
    };

    info!("Input simulator ready");

    // Create focus detector
    let focus_detector = create_focus_detector(TARGET_WINDOW)?;
    let focus_state = Arc::new(FocusState::new());
    let _focus_handle = start_focus_poller(focus_detector, focus_state.clone(), running.clone());

    // Create channel for input events
    let (sender, receiver) = create_event_channel();

    // Start input listener in background thread
    let listener = InputListener::new(sender);
    let _listener_handle = listener.start();

    info!("Input listener ready - listening for right-clicks");
    info!("Press Ctrl+C to exit");

    // Main event loop
    while running.load(Ordering::SeqCst) {
        // Try to receive events with a timeout
        match receiver.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(event) => {
                // Check if target window is focused
                if focus_state.is_focused() {
                    info!("Right-click detected! Target focused - sending auto-click...");

                    // Send the auto-click
                    if let Err(e) = simulator.send_right_click() {
                        error!("Failed to send auto-click: {}", e);
                    } else {
                        let elapsed = event.timestamp.elapsed();
                        info!("Auto-click sent (latency: {:?})", elapsed);
                    }
                } else {
                    info!("Right-click detected, but target not focused - ignoring");
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // No event, continue loop
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                warn!("Input listener disconnected");
                break;
            }
        }
    }

    info!("DoubleTap-RL shutting down...");

    // Note: listener and focus threads will be terminated when main exits
    // In a more robust implementation, we'd have a proper shutdown mechanism

    Ok(())
}
