//! Focus detection for window gating
//!
//! Uses X11 APIs to detect the active window. Works for both native X11
//! and XWayland windows (like Proton/Wine games).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tracing::info;

use crate::DoubleTapError;

/// Trait for focus detection implementations
pub trait FocusDetector: Send + Sync {
    /// Check if the target window is currently focused
    fn is_target_focused(&self) -> bool;
}

/// X11-based focus detector using _NET_ACTIVE_WINDOW
pub struct X11FocusDetector {
    target_name: String,
}

impl X11FocusDetector {
    /// Create a new X11 focus detector
    pub fn new(target_name: impl Into<String>) -> Result<Self, DoubleTapError> {
        let target_name = target_name.into();
        
        // Test connection to X11
        match x11rb::connect(None) {
            Ok(_) => Ok(Self { target_name }),
            Err(e) => {
                Err(DoubleTapError::FocusDetection(format!(
                    "Failed to connect to X11: {}",
                    e
                )))
            }
        }
    }
    
    /// Get the active window title from X11
    fn get_active_window_title(&self) -> Option<String> {
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, Window};
        
        let (conn, screen_num) = x11rb::connect(None).ok()?;
        let screen = &conn.setup().roots[screen_num];
        let root = screen.root;
        
        // Intern atoms we need
        let net_active_window = conn
            .intern_atom(false, b"_NET_ACTIVE_WINDOW")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let net_wm_name = conn
            .intern_atom(false, b"_NET_WM_NAME")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let utf8_string = conn
            .intern_atom(false, b"UTF8_STRING")
            .ok()?
            .reply()
            .ok()?
            .atom;
        
        // Get active window
        let active_window_reply = conn
            .get_property(false, root, net_active_window, AtomEnum::WINDOW, 0, 1)
            .ok()?
            .reply()
            .ok()?;
        
        if active_window_reply.value.len() < 4 {
            return None;
        }
        
        let active_window = u32::from_ne_bytes([
            active_window_reply.value[0],
            active_window_reply.value[1],
            active_window_reply.value[2],
            active_window_reply.value[3],
        ]) as Window;
        
        if active_window == 0 {
            return None;
        }
        
        // Try _NET_WM_NAME first (UTF-8)
        let name_reply = conn
            .get_property(false, active_window, net_wm_name, utf8_string, 0, 256)
            .ok()?
            .reply()
            .ok()?;
        
        if !name_reply.value.is_empty() {
            return String::from_utf8(name_reply.value).ok();
        }
        
        // Fall back to WM_NAME (legacy)
        let wm_name_reply = conn
            .get_property(
                false,
                active_window,
                AtomEnum::WM_NAME,
                AtomEnum::STRING,
                0,
                256,
            )
            .ok()?
            .reply()
            .ok()?;
        
        if !wm_name_reply.value.is_empty() {
            return String::from_utf8(wm_name_reply.value).ok();
        }
        
        None
    }
}

impl FocusDetector for X11FocusDetector {
    fn is_target_focused(&self) -> bool {
        if let Some(title) = self.get_active_window_title() {
            title == self.target_name
        } else {
            false
        }
    }
}

/// Cached focus detector that wraps another detector
/// Caches the focus state and only re-queries after the cache expires
pub struct CachedFocusDetector<D: FocusDetector> {
    inner: D,
    cached_state: AtomicBool,
    last_check: std::sync::Mutex<Instant>,
    cache_duration: Duration,
}

impl<D: FocusDetector> CachedFocusDetector<D> {
    /// Create a new cached focus detector
    pub fn new(inner: D, cache_duration: Duration) -> Self {
        Self {
            inner,
            cached_state: AtomicBool::new(false),
            last_check: std::sync::Mutex::new(Instant::now() - cache_duration),
            cache_duration,
        }
    }
}

impl<D: FocusDetector> FocusDetector for CachedFocusDetector<D> {
    fn is_target_focused(&self) -> bool {
        let now = Instant::now();
        let mut last_check = self.last_check.lock().unwrap();
        
        if now.duration_since(*last_check) >= self.cache_duration {
            let focused = self.inner.is_target_focused();
            self.cached_state.store(focused, Ordering::SeqCst);
            *last_check = now;
            focused
        } else {
            self.cached_state.load(Ordering::SeqCst)
        }
    }
}

/// Create the focus detector
pub fn create_focus_detector(
    target_window: &str,
) -> Result<Box<dyn FocusDetector>, DoubleTapError> {
    let detector = X11FocusDetector::new(target_window)?;
    Ok(Box::new(CachedFocusDetector::new(
        detector,
        Duration::from_millis(100),
    )))
}

/// Shared focus state that can be polled from another thread
pub struct FocusState {
    is_focused: AtomicBool,
}

impl FocusState {
    pub fn new() -> Self {
        Self {
            is_focused: AtomicBool::new(false),
        }
    }
    
    pub fn is_focused(&self) -> bool {
        self.is_focused.load(Ordering::SeqCst)
    }
    
    fn set_focused(&self, focused: bool) {
        self.is_focused.store(focused, Ordering::SeqCst);
    }
}

impl Default for FocusState {
    fn default() -> Self {
        Self::new()
    }
}

/// Start a background thread that polls focus state
pub fn start_focus_poller(
    detector: Box<dyn FocusDetector>,
    state: Arc<FocusState>,
    running: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        info!("Focus poller started");
        
        while running.load(Ordering::SeqCst) {
            let focused = detector.is_target_focused();
            state.set_focused(focused);
            thread::sleep(Duration::from_millis(100));
        }
        
        info!("Focus poller stopped");
    })
}
