# EasyCue3 - Theatrical Lighting & Media Console

A simple, small-scale theatrical lighting console application combining ETC EOS-style lighting control with QLab-style media playback. Built in Rust with egui for maximum performance and cross-platform support.

## Features

**Core (MVP)**:
- ✅ DMX Universe management (1-2 universes, 512 channels each)
- ✅ Cue list with GO/BACK/STOP transport controls  
- ✅ Smooth crossfades between lighting states
- ✅ Virtual DMX backend for testing without hardware
- 🚧 USB DMX interfaces (FTDI-based, Enttec, etc.)
- 🚧 Art-Net DMX over Ethernet
- 🚧 Fixture profiles (dimmers, RGB, moving lights)
- 🚧 Live fixture control with sliders
- 🚧 EOS-style command line ("1 Thru 10 At 50")
- 🚧 Show file save/load (JSON format)

**Media Integration** (Phase 5):
- 📋 Audio playback (MP3, WAV, FLAC, AAC)
- 📋 Video playback (MP4, MOV, MKV, WebM)
- 📋 Image display (PNG, JPEG, SVG)
- 📋 Media cues linked to lighting cues

**Advanced** (Post-MVP):
- 📋 Groups/presets for fixture selections
- 📋 Magic sheets (custom visual layouts)
- 📋 Effects engine (chase, rainbow, etc.)
- 📋 Timecode sync (MIDI/LTC/OSC)

Legend: ✅ Complete | 🚧 In Progress | 📋 Planned

## Quick Start

### Prerequisites

#### Linux (Debian/Ubuntu)
```bash
sudo apt-get update
sudo apt-get install -y \
    build-essential \
    libx11-dev \
    libxi-dev \
    libxcursor-dev \
    libxrandr-dev \
    libxinerama-dev \
    libgl1-mesa-dev \
    libudev-dev \
    libasound2-dev \
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev
```

#### macOS
```bash
# Install Rust if not already installed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# GStreamer for video (optional)
brew install gstreamer gst-plugins-base
```

#### Windows
```powershell
# Install Rust from https://rustup.rs/
# System libraries are handled automatically via vcpkg
```

### Building

```bash
# Clone and build (minimal - no media)
git clone <repo-url>
cd easycue3
cargo build --release

# Build with all features (requires system libraries)
cargo build --release --features full

# Run
cargo run --release
```

### Feature Flags

Control which features to enable based on available system libraries:

- **Default**: Core lighting control, virtual DMX, no media
- `usb`: USB DMX interfaces (requires libudev on Linux)
- `audio`: Audio playback (requires ALSA on Linux)
- `video`: Video playback (requires GStreamer on Linux)
- `media`: Both audio and video
- `full`: All features

Examples:
```bash
# Just USB DMX support
cargo run --features usb

# Audio only
cargo run --features audio

# Everything
cargo run --features full
```

## Usage

### Basic Workflow

1. **Setup Fixtures**: Patch fixtures to DMX addresses
2. **Create Cues**: Set lighting levels and record cues
3. **Playback**: Use GO/BACK buttons or spacebar to play through your cue list
4. **Add Media**: Link sound/video cues to lighting cues (requires media features)

### Keyboard Shortcuts

- **Space**: GO (advance to next cue)
- **B**: BACK (return to previous cue)
- **S**: STOP (halt playback)
- **Ctrl+R**: Record new cue
- **Ctrl+S**: Save show
- **Ctrl+O**: Open show

### DMX Output

#### Virtual (Development)
No hardware required - outputs to console log

#### USB DMX
Connect a USB DMX interface (FTDI chip-based devices supported)

#### Art-Net
Configure in Settings > DMX Output:
- Enable Art-Net
- Set broadcast address (e.g., 2.255.255.255 for 2.x.x.x network)
- Set universe number

## Architecture

```
easycue3/
├── src/
│   ├── main.rs              # Application entry point
│   ├── app.rs               # Main application state
│   ├── dmx/                 # DMX universe & backends
│   │   ├── universe.rs      # 512-channel universe
│   │   └── backends/        # Virtual, USB, Art-Net
│   ├── cue/                 # Cue system
│   │   ├── types.rs         # Cue data structures
│   │   ├── list.rs          # Cue list management
│   │   └── playback.rs      # Playback engine with crossfades
│   ├── fixtures/            # Fixture profiles & patching
│   ├── media/               # Audio/video/image playback
│   ├── ui/                  # egui interface
│   └── show/                # Show file persistence
├── fixture_profiles/        # JSON fixture definitions
└── shows/                   # Example show files
```

## Development Status

**Phase 1**: ✅ Foundation & Project Setup  
**Phase 2**: 🚧 DMX Foundation (virtual backend complete)  
**Phase 3**: 🚧 Cue Engine Core (data structures complete, playback in progress)  
**Phase 4**: 📋 Live Control UI  
**Phase 5**: 📋 Media Integration  
**Phase 6**: 📋 Persistence & Show Files  
**Phase 7**: 📋 Polish & Usability  
**Phase 8**: 📋 Advanced Features  

## Contributing

Contributions welcome! Areas needing help:
- Fixture profile library (manufacturer-specific profiles)
- USB DMX driver testing on different hardware
- Platform testing (especially Windows)
- UI/UX improvements
- Documentation

## License

MIT OR Apache-2.0 (dual licensed)

## Credits

Built with:
- [egui](https://github.com/emilk/egui) - Immediate mode GUI
- [lumina-video](https://github.com/lumina-video/lumina-video) - Hardware-accelerated video
- [rodio](https://github.com/RustAudio/rodio) - Audio playback
- [artnet_protocol](https://github.com/mmmiles/artnet_protocol) - Art-Net DMX

Inspired by ETC EOS consoles and QLab media playback.
