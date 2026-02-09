# DoubleTap-RL

**Auto-clicker for Rocket League double-tap aerials on Linux**

DoubleTap-RL automatically sends a second right-click immediately after you press right-click, eliminating the human refractory period limitation for perfect double-taps every time.

## How It Works

1. Creates a virtual input device via Linux's `uinput` subsystem
2. Listens for right-click events globally via `evdev`
3. Checks if Rocket League is the focused window (via X11/XWayland)
4. If focused, sends an automatic second right-click through the virtual device (~15ms latency)
5. If not focused, does nothing

### Virtual Input Device

The program creates its own virtual mouse device at startup (visible as `/dev/input/event*`). This device writes raw `input_event` structs directly to the kernel. Events are sent using raw `libc::write()` calls for maximum speed.

The virtual device is created **after** the input listener starts to prevent the listener from consuming the device's output events.

## About X11 and Wayland

If you examine the code, you will see things related to X11, but don't worry, this does not mean it won't work with Wayland. We are only checking to see if the currently focused window is Rocket League. Since the Rocket League game will start with any Proton, and Proton is still generally under X11 (XWayland), it will work flawlessly.

## About the Human Refractory Period

I've spent about 400 hours playing the game, and even I still can't right-click twice in a row reliably. The human refractory period is the minimum time required between two conscious motor responses - this tool eliminates that limitation.

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

## Troubleshooting

### Rocket League not detected

Make sure the game window title matches exactly: `Rocket League (64-bit, DX11, Cooked)`. The program uses X11 APIs which work for XWayland windows.

## License

MIT
