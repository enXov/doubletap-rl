//! DoubleTap-RL - Auto-clicker for Rocket League double-tap aerials
//!
//! This program automatically sends a second right-click after detecting
//! the user's right-click, helping with double-tap aerial mechanics.

use doubletap_rl::{
    input_listener::{create_event_channel, InputListener},
    Config, DoubleTapError, InputSimulator,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

fn main() -> Result<(), DoubleTapError> {
    // Initialize logging
    let _subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .with_target(false)
        .compact()
        .init();

    info!("DoubleTap-RL starting...");

    // Load configuration
    let config = Config::default();
    info!(
        "Config: delay={}ms, target='{}'",
        config.click_delay_ms, config.target_window
    );

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
                info!("Right-click detected! Sending auto-click...");

                // Send the auto-click
                if let Err(e) = simulator.send_right_click(config.click_delay_ms) {
                    error!("Failed to send auto-click: {}", e);
                } else {
                    let elapsed = event.timestamp.elapsed();
                    info!("Auto-click sent (latency: {:?})", elapsed);
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

    // Note: listener thread will be terminated when main exits
    // In a more robust implementation, we'd have a proper shutdown mechanism

    Ok(())
}
