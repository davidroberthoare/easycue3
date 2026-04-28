# EasyCue3 - GitHub Copilot Instructions

## Project Overview

EasyCue3 is a **theatrical lighting and media console** built in Rust with egui. It combines ETC EOS-style lighting control with QLab-style media playback for small-to-medium scale productions, educational theatre, and hobbyists.

**Core Philosophy:**
- Simplicity first, beginner-friendly
- Cross-platform (macOS, Linux, Windows)
- Hardware flexible (Virtual DMX, USB DMX, Art-Net, sACN)
- Real-time performance critical (60 FPS UI, 40 Hz DMX output)

---

## Architecture

**UI Framework:** egui 0.31 (immediate-mode GUI)
- Updates every frame at 60 FPS
- All UI is rendered in `update()` method
- State lives in `EasyCueApp` struct in `src/app.rs`

**DMX Output:** Multi-backend system
- `Universe` struct: 512 channels (1-indexed), values 0-255 (u8)
- `DmxBackend` trait for pluggable outputs (Virtual, USB, Art-Net, sACN)
- Separate thread for DMX output to prevent UI blocking

**Threading Model:**
```
Main Thread (egui UI 60 FPS)
    ↓ reads/writes
Arc<Mutex<Universe>> (shared state)
    ↑ writes
Playback Thread (fade calculations)
    ↓ sends
DMX Thread (40 Hz output)
```

---

## Module Structure

```
src/
├── main.rs              # Entry point, eframe setup
├── app.rs               # EasyCueApp (main application state)
├── dmx/
│   ├── mod.rs           # DMX module exports
│   ├── universe.rs      # Universe struct, DmxBackend trait
│   └── backends/        # VirtualBackend, USB, Art-Net, sACN
├── cue/
│   ├── types.rs         # Cue struct
│   ├── list.rs          # CueList (sorted Vec<Cue>)
│   └── playback.rs      # PlaybackEngine, PlaybackState
├── fixtures/            # Fixture profiles (planned Phase 3)
├── media/               # Audio/video playback (planned Phase 5)
├── show/                # Show file save/load (JSON)
└── ui/                  # UI panel render functions
```

---

## Key Types

### Core Data Structures

```rust
// DMX channel: 1-indexed, value 0-255
type ChannelNumber = u16;  // 1-512
type ChannelValue = u8;     // 0-255 (0=off, 255=full)

// Cue: A preset lighting/sound state
pub struct Cue {
    pub number: f32,                        // 1.0, 1.5, 2.0, etc.
    pub label: String,                      // "House Lights"
    pub fade_up: f32,                       // Seconds
    pub fade_down: f32,                     // Seconds
    pub channel_values: HashMap<u16, u8>,   // DMX values
    pub notes: String,
}

// Universe: 512 DMX channels
pub struct Universe {
    channels: [u8; 512],  // 0-indexed internally, 1-indexed API
}

// Playback states
pub enum PlaybackState {
    Idle,
    FadingUp { from_cue: usize, to_cue: usize, elapsed: f32 },
    FadingDown { from_cue: usize, to_cue: usize, elapsed: f32 },
    Holding { cue_index: usize },
}
```

### Key Patterns

**Error Handling:**
```rust
use anyhow::Result;  // Use for all fallible operations

pub fn load_show(path: &str) -> Result<Show> {
    let contents = std::fs::read_to_string(path)?;  // Propagate errors
    let show: Show = serde_json::from_str(&contents)?;
    Ok(show)
}

// In app code, handle gracefully:
match load_show(path) {
    Ok(show) => { /* success */ },
    Err(e) => log::error!("Failed to load: {}", e),
}
```

**Logging:**
```rust
log::info!("Starting playback");
log::debug!("Fade progress: {:.2}", progress);
log::warn!("Channel {} out of range", channel);
log::error!("Failed to open file: {}", e);
```

---

## Coding Conventions

### Naming

- **snake_case:** functions, variables, modules: `fade_up`, `get_channel`, `channel_values`
- **CamelCase:** types, structs, enums, traits: `Cue`, `CueList`, `PlaybackEngine`, `DmxBackend`
- **SCREAMING_SNAKE_CASE:** constants: `MAX_CHANNELS`, `DEFAULT_FADE_TIME`, `DMX_UNIVERSE_SIZE`

### Comments

```rust
// Use inline comments for complex logic
// Explain WHY, not WHAT (code should be self-documenting)

/// Use doc comments for public APIs
/// 
/// # Arguments
/// 
/// * `channel` - DMX channel number (1-512)
/// * `value` - DMX value (0-255)
/// 
/// # Example
/// 
/// ```
/// cue.set_channel(1, 255);
/// ```
pub fn set_channel(&mut self, channel: u16, value: u8) { }
```

### Ownership Patterns

```rust
// Prefer borrowing over ownership transfer
fn process_cue(cue: &Cue) { }  // ✅ Borrow
fn process_cue(cue: Cue) { }   // ❌ Takes ownership

// Use &mut for modification
fn update_channel(universe: &mut Universe, ch: u16, val: u8) {
    universe.set_channel(ch, val);
}

// Arc<Mutex<T>> for shared mutable state across threads
let universe = Arc::new(Mutex::new(Universe::new()));
```

---

## Theatre Terminology (appears in code)

- **DMX:** Protocol for stage lighting (512 channels per universe)
- **Universe:** Set of 512 DMX channels
- **Channel:** Single controllable parameter (1-512, maps to lights)
- **Fixture:** Physical lighting device (LED, dimmer, moving light)
- **Cue:** Preset lighting state with fade times
- **Fade:** Smooth transition between cues (in seconds)
- **GO:** Advance to next cue
- **BACK:** Return to previous cue
- **Patch:** Assign fixtures to DMX channels
- **Intensity:** Brightness (0-255 in DMX, 0-100% to users)
- **Group:** Saved fixture selection
- **Preset/Palette:** Saved attribute (color, position, etc.)
- **Art-Net:** DMX over Ethernet protocol
- **sACN:** Streaming ACN, another DMX-over-Ethernet protocol

---

## Critical Technical Patterns

### Smooth Crossfades

```rust
// Calculate fade progress (0.0 to 1.0)
let progress = elapsed_time / fade_time;
let progress = progress.clamp(0.0, 1.0);

// Interpolate between previous and next channel values
let prev = prev_cue.get_channel(ch).unwrap_or(0) as f32;
let next = next_cue.get_channel(ch).unwrap_or(0) as f32;
let current = prev + (next - prev) * progress;

// Convert back to u8 and write to universe
universe.set_channel(ch, current as u8);
```

### egui Immediate-Mode UI

```rust
impl eframe::App for EasyCueApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top menu bar
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Show").clicked() {
                        self.new_show();
                    }
                });
            });
        });
        
        // Side panels, central panel, etc.
        egui::SidePanel::left("cue_list").show(ctx, |ui| {
            // Render cue list
        });
        
        // Request repaint for animations
        ctx.request_repaint();
    }
}
```

### DmxBackend Trait Implementation

```rust
pub trait DmxBackend: Send {
    /// Send DMX data to the output device
    /// 
    /// # Arguments
    /// 
    /// * `universe_id` - Universe number (1-based)
    /// * `data` - 512-byte array of DMX values
    fn send(&mut self, universe_id: u16, data: &[u8]) -> Result<()>;
}

// Example implementation
pub struct VirtualBackend;

impl DmxBackend for VirtualBackend {
    fn send(&mut self, universe_id: u16, data: &[u8]) -> Result<()> {
        log::info!("[VirtualDMX] Universe {}: {:?}", universe_id, 
                   &data[0..10]);  // Log first 10 channels
        Ok(())
    }
}
```

---

## Feature Flags

Control optional functionality in `Cargo.toml`:

```toml
[features]
default = []
usb = ["serialport"]      # USB DMX interfaces
audio = ["rodio"]         # Audio playback
video = ["lumina-video"]  # Video playback
media = ["audio", "video"]
full = ["usb", "media"]
```

**Use in code:**
```rust
#[cfg(feature = "usb")]
use serialport::{SerialPort, SerialPortBuilder};

#[cfg(feature = "audio")]
pub mod audio_player;
```

---

## Common Patterns

### Adding a DMX Backend

1. Create `src/dmx/backends/my_backend.rs`
2. Implement `DmxBackend` trait
3. Export in `src/dmx/backends/mod.rs`:
   ```rust
   pub mod my_backend;
   pub use my_backend::MyBackend;
   ```

### Adding a UI Panel

1. Create `src/ui/my_panel.rs`
2. Define render function:
   ```rust
   pub fn render_my_panel(ui: &mut egui::Ui, app_state: &mut AppState) {
       ui.heading("My Panel");
       // Add UI elements
   }
   ```
3. Call from `src/app.rs` in `update()`:
   ```rust
   egui::SidePanel::right("my_panel").show(ctx, |ui| {
       ui::my_panel::render_my_panel(ui, &mut self.state);
   });
   ```

### Recording a Cue

```rust
// Snapshot current universe state into a new cue
pub fn record_cue(&mut self) {
    let universe = self.universe.lock().unwrap();
    let next_number = self.cue_list.next_cue_number();
    
    let mut cue = Cue::new(next_number, format!("Cue {}", next_number));
    
    // Copy all non-zero channels from universe
    for ch in 1..=512 {
        if let Some(value) = universe.get_channel(ch) {
            if value > 0 {
                cue.set_channel(ch, value);
            }
        }
    }
    
    self.cue_list.add_cue(cue);
    log::info!("Recorded cue {}", next_number);
}
```

---

## Testing Guidelines

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_universe_channel_bounds() {
        let mut universe = Universe::new();
        
        // Valid channel
        assert!(universe.set_channel(1, 255).is_ok());
        assert_eq!(universe.get_channel(1), Some(255));
        
        // Out of bounds
        assert!(universe.set_channel(0, 255).is_err());
        assert!(universe.set_channel(513, 255).is_err());
    }
    
    #[test]
    fn test_cue_fade_interpolation() {
        let mut prev = Cue::new(1.0, "Prev".into());
        prev.set_channel(1, 0);
        
        let mut next = Cue::new(2.0, "Next".into());
        next.set_channel(1, 255);
        
        // At 50% progress, should be ~127
        let progress = 0.5;
        let interpolated = interpolate_channel(&prev, &next, 1, progress);
        assert_eq!(interpolated, 127);
    }
}
```

---

## Current Phase: Phase 3 (DMX Hardware & Fixture Control)

**In Progress:**
- USB DMX backend (FTDI-based devices)
- Art-Net backend (DMX over Ethernet)
- Fixture profiles (Generic Dimmer, RGB, RGBW, Moving Light)
- Fixture patching UI
- Live fixture control panel

**When generating code:**
- Prioritize real-time performance (avoid allocations in hot paths)
- Use `log::` macros, not `println!`
- Handle errors gracefully (no `.unwrap()` in production code)
- Document theatre-specific concepts for Rust developers
- Follow ownership best practices (prefer borrowing)
- Keep UI code in `src/ui/`, logic in respective modules

---

## Dependencies

```toml
egui = "0.31"                    # Immediate-mode GUI
eframe = "0.31"                  # egui window framework
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"               # Show file serialization
anyhow = "1.0"                   # Error handling
log = "0.4"                      # Logging facade
env_logger = "0.11"              # Log implementation
chrono = { version = "0.4", features = ["serde"] }
tokio = { version = "1", features = ["rt-multi-thread", "sync", "time"] }

# Optional
serialport = { version = "4.5", optional = true }  # USB DMX
artnet_protocol = "0.3"          # Art-Net DMX
rodio = { version = "0.19", optional = true }      # Audio
lumina-video = { git = "...", optional = true }    # Video
```

---

## Constraints & Limitations

- **Universe Count:** 2-16 (not 100+)
- **Fixture Count:** ~200 max (not thousands)
- **DMX Refresh:** 40 Hz (not 1000 Hz)
- **Channel Precision:** 8-bit (0-255), not 16-bit
- **Target Audience:** Educational, small venues, hobbyists (not large concerts)

**When suggesting features:**
- Keep it simple and beginner-friendly
- Consider educational use cases
- Ensure cross-platform compatibility
- Maintain real-time performance requirements
- Stay within scope (small-to-medium scale productions)
