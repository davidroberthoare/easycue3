# Installation Guide

## System Requirements

- **Operating System**: Linux, macOS, or Windows
- **Rust**: 1.70 or later
- **Memory**: 512 MB RAM minimum, 1 GB recommended
- **Display**: Any resolution, 1280x720 or higher recommended

## Platform-Specific Installation

### Linux (Debian/Ubuntu)

1. **Install System Dependencies**:
```bash
sudo apt-get update
sudo apt-get install -y \
    build-essential \
    pkg-config \
    libx11-dev \
    libxi-dev \
    libxcursor-dev \
    libxrandr-dev \
    libxinerama-dev \
    libgl1-mesa-dev
```

2. **Optional: For USB DMX and Media Support**:
```bash
sudo apt-get install -y \
    libudev-dev \
    libasound2-dev \
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev
```

3. **Install Rust** (if not already installed):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

4. **Build EasyCue3**:
```bash
git clone <repository-url>
cd easycue3
cargo build --release

# Or with all features:
cargo build --release --features full
```

5. **Run**:
```bash
cargo run --release
```

### macOS

1. **Install Rust**:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

2. **Install Homebrew** (if not already installed):
```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

3. **Optional: For Video Support**:
```bash
brew install gstreamer gst-plugins-base
```

4. **Build and Run**:
```bash
git clone <repository-url>
cd easycue3
cargo build --release
cargo run --release
```

### Windows

1. **Install Rust**:
   - Download and run [rustup-init.exe](https://rustup.rs/)
   - Follow the installer instructions

2. **Install Visual Studio Build Tools**:
   - Download [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/)
   - Select "Desktop development with C++"

3. **Build and Run**:
```powershell
git clone <repository-url>
cd easycue3
cargo build --release
cargo run --release
```

## Feature Flags

EasyCue3 uses optional features to avoid requiring system libraries you don't need:

- **Default**: Core lighting control with virtual DMX (no hardware)
- `usb`: USB DMX interface support (requires libudev on Linux)
- `audio`: Audio playback (requires ALSA on Linux)
- `video`: Video playback (requires GStreamer on Linux)
- `media`: Both audio and video
- `full`: All features

**Examples**:
```bash
# Just USB DMX support
cargo run --release --features usb

# Audio and video
cargo run --release --features media

# Everything
cargo run --release --features full
```

## Troubleshooting

### Linux: Missing library errors

If you see errors about missing libraries (`libudev`, `libasound2`, `gstreamer`), either:
1. Install the missing system libraries (see step 2 above)
2. Build without those features:
   ```bash
   cargo build --release --no-default-features
   ```

### macOS: SDK not found

If you get SDK errors during build:
```bash
export SDKROOT=$(xcrun --sdk macosx --show-sdk-path)
cargo clean
cargo build --release
```

### Windows: Link errors

Make sure Visual Studio Build Tools are installed with C++ support:
```powershell
# Verify installation
where cl
```

### Display/GUI issues

If the application doesn't start or crashes on startup:
- Make sure your display server is running (X11 or Wayland on Linux)
- Try setting the display explicitly: `DISPLAY=:0 cargo run --release`
- Check for graphics driver issues

### Permission errors for USB DMX

On Linux, you may need to add your user to the `dialout` group for USB device access:
```bash
sudo usermod -a -G dialout $USER
# Log out and back in for changes to take effect
```

## Verifying Installation

After building, test the application:

1. **Start the application**:
   ```bash
   cargo run --release
   ```

2. **Check the console for startup messages**:
   ```
   [INFO] Starting EasyCue3...
   [INFO] EasyCue3 application initialized
   [INFO] Virtual DMX backend initialized (verbose: true)
   [INFO] DMX Backend: Virtual DMX (logging)
   ```

3. **Verify the UI loads**:
   - Window should open with title "EasyCue3 - Theatrical Lighting Console"
   - You should see menu bar, transport controls, and cue list panel

## Next Steps

- Read [README.md](README.md) for feature overview
- Check out example show files in `shows/`
- Review keyboard shortcuts
- Start creating your first show!

## Getting Help

If you encounter issues:
1. Check this guide for common solutions
2. Review error messages carefully
3. Search existing issues on GitHub
4. Create a new issue with:
   - Your OS and version
   - Rust version (`rustc --version`)
   - Full error message
   - Build command used
