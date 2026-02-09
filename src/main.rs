//! DoubleTap-RL - Auto-clicker for Rocket League double-tap aerials
//!
//! Automatically sends a second right-click after detecting the user's
//! right-click, helping with double-tap aerial mechanics.

use doubletap_rl::{
    create_focus_detector,
    input_listener::{create_event_channel, mark_auto_click_sent, InputListener},
    start_focus_poller, DoubleTapError, FocusState, InputSimulator,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

const TARGET_WINDOW: &str = "Rocket League (64-bit, DX11, Cooked)";

/// Delay (ms) before auto-click — the compositor needs a brief window to
/// process the physical release before our click arrives. 10ms is reliable
/// and still under 1 game frame (16.6ms at 60fps), so it's imperceptible.
const AUTO_CLICK_DELAY_MS: u64 = 15;

fn main() -> Result<(), DoubleTapError> {
    let _subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .compact()
        .init();

    info!("DoubleTap-RL starting...");
    info!("Target window: '{}'", TARGET_WINDOW);

    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();
    ctrlc::set_handler(move || {
        info!("Shutdown signal received");
        running_clone.store(false, Ordering::SeqCst);
    })
    .expect("Failed to set Ctrl+C handler");

    // Focus detection
    let focus_detector = create_focus_detector(TARGET_WINDOW)?;
    let focus_state = Arc::new(FocusState::new());
    let _focus_handle = start_focus_poller(focus_detector, focus_state.clone(), running.clone());

    // Start input listener FIRST — rdev scans /dev/input/event* on startup.
    // Creating our virtual device AFTER ensures rdev won't read from it.
    let (sender, receiver) = create_event_channel();
    let listener = InputListener::new(sender);
    let _listener_handle = listener.start();
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Now create virtual device (rdev won't know about it)
    let mut simulator = match InputSimulator::new() {
        Ok(sim) => sim,
        Err(DoubleTapError::PermissionDenied) => {
            error!("Permission denied. Add your user to the 'input' group:");
            error!("  sudo usermod -aG input $USER");
            error!("Then logout and login again.");
            return Err(DoubleTapError::PermissionDenied);
        }
        Err(e) => return Err(e),
    };

    info!("Press Ctrl+C to exit");

    while running.load(Ordering::SeqCst) {
        match receiver.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(_event) => {
                if focus_state.is_focused() {
                    // Brief delay for compositor to process physical release
                    std::thread::sleep(std::time::Duration::from_millis(AUTO_CLICK_DELAY_MS));

                    if let Err(e) = simulator.send_right_click() {
                        error!("Auto-click failed: {}", e);
                    } else {
                        mark_auto_click_sent();
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                warn!("Input listener disconnected");
                break;
            }
        }
    }

    info!("DoubleTap-RL shutting down...");
    Ok(())
}
