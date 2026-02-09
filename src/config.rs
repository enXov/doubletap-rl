//! Configuration management for DoubleTap-RL

/// Configuration for the auto-clicker
#[derive(Debug, Clone)]
pub struct Config {
    /// Delay in milliseconds between original click and auto-click
    pub click_delay_ms: u64,

    /// Target window name to match (for focus gating)
    pub target_window: String,

    /// Enable verbose logging
    pub verbose: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            click_delay_ms: 0,
            target_window: String::from("Rocket League"),
            verbose: false,
        }
    }
}

impl Config {
    /// Create a new Config with custom click delay
    pub fn with_delay(mut self, delay_ms: u64) -> Self {
        self.click_delay_ms = delay_ms;
        self
    }

    /// Create a new Config with custom target window
    pub fn with_target_window(mut self, window: impl Into<String>) -> Self {
        self.target_window = window.into();
        self
    }

    /// Enable verbose logging
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }
}
