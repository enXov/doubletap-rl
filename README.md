# DoubleTap-RL

**Auto-clicker for Rocket League double-tap aerials on Linux**

DoubleTap-RL automatically sends a second right-click immediately after you press right-click, eliminating the human refractory period limitation for perfect double-taps every time.

## Installation

```bash
# Add yourself to the input group (logout/login required)
sudo usermod -aG input $USER

# Clone and build
git clone https://github.com/enXov/doubletap-rl.git
cd doubletap-rl/doubletap-rl
cargo build --release

# Run
./target/release/doubletap-rl
```

## How It Works

1. Listens for right-click events globally via `evdev`
2. Checks if Rocket League is the focused window
3. If focused, automatically sends a second right-click via `uinput`
4. If not focused, passes through normally

## License

MIT
