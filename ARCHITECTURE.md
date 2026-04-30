# EasyCue3 Architecture

## System Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        EasyCue3 Application                      │
│                       (egui/eframe Window)                       │
└─────────────────────────────────────────────────────────────────┘
                               │
        ┌──────────────────────┼──────────────────────┐
        │                      │                      │
        ▼                      ▼                      ▼
┌──────────────┐      ┌──────────────┐      ┌──────────────┐
│   UI Layer   │      │  App State   │      │ Show Files   │
│   (egui)     │◄────►│   Manager    │◄────►│   (JSON)     │
└──────────────┘      └──────────────┘      └──────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        │                     │                     │
        ▼                     ▼                     ▼
┌──────────────┐      ┌──────────────┐      ┌──────────────┐
│ Cue Engine   │      │ DMX Output   │      │Media Manager │
│              │      │              │      │              │
│ ┌──────────┐ │      │ ┌──────────┐ │      │ ┌──────────┐ │
│ │PlaybackEng│ │      │ │Universe  │ │      │ │Audio     │ │
│ │CueList    │ │      │ │(512ch×2) │ │      │ │Video     │ │
│ └──────────┘ │      │ └──────────┘ │      │ │Image     │ │
└──────────────┘      └──────┬───────┘      └──────────────┘
                              │
                 ┌────────────┼────────────┐
                 │            │            │
                 ▼            ▼            ▼
         ┌──────────┐  ┌──────────┐  ┌──────────┐
         │ Virtual  │  │   USB    │  │ Art-Net  │
         │   DMX    │  │   DMX    │  │   DMX    │
         │(Logging) │  │(Serial)  │  │(Network) │
         └──────────┘  └──────────┘  └──────────┘
```

## Data Flow

### Cue Playback Flow

```
User presses GO button
        │
        ▼
UI sends event to App
        │
        ▼
App calls PlaybackEngine.go()
        │
        ▼
PlaybackEngine reads next cue from CueList
        │
        ▼
Fade timer starts (uses frame delta time)
        │
        ▼
Each frame: update() interpolates channel values
        │
        ▼
Write interpolated values to Universe
        │
        ▼
Universe sends to active DMX backend
        │
        ▼
Backend outputs to hardware/virtual/network
```

### Cue Recording Flow (Planned)

```
User adjusts fixture controls (sliders)
        │
        ▼
UI updates Universe channel values
        │
        ▼
User presses "Record" button
        │
        ▼
App reads current Universe state
        │
        ▼
Create new Cue with channel values
        │
        ▼
Add Cue to CueList
        │
        ▼
UI updates to show new cue
```

## Module Responsibilities

### `src/main.rs`
- Application entry point
- Initialize logging
- Configure eframe window
- Launch application

### `src/app.rs` (AppState)
- **Owns**: Universes, DMX backend, CueList, PlaybackEngine, MediaManager
- **Responsibilities**:
  - Update loop (called every frame by eframe)
  - Coordinate between subsystems
  - Handle UI state flags
- **Key Methods**:
  - `new()`: Initialize application
  - `update()`: Frame update callback from eframe

### `src/dmx/` (DMX Subsystem)
- **`universe.rs`**: 512-channel DMX universe data structure
- **`backends/mod.rs`**: DmxBackend trait definition
- **`backends/virtual_dmx.rs`**: Logging-based output
- **Responsibilities**:
  - Store channel values (0-255)
  - Provide channel get/set API
  - Send universe data to output backend

### `src/cue/` (Cue Subsystem)
- **`types.rs`**: Cue and CueState definitions
- **`list.rs`**: Cue list management (sorted, navigation)
- **`playback.rs`**: Playback engine with crossfades
- **Responsibilities**:
  - Store lighting states
  - Manage cue sequencing
  - Interpolate between cues (fades)
  - Track playback state (Stopped/Fading/Active)

### `src/ui/` (User Interface)
- **`mod.rs`**: All egui rendering code
- **Functions**:
  - `render()`: Main UI entry point
  - `render_cue_list_panel()`: Cue list display
  - `render_transport_panel()`: GO/BACK/STOP buttons
  - `render_fixture_panel()`: Fixture controls (placeholder)
  - `render_media_panel()`: Media controls (placeholder)
- **Responsibilities**:
  - Display application state
  - Capture user input
  - Send commands to App

### `src/fixtures/` (Fixture Subsystem - Placeholder)
- **`mod.rs`**: Fixture library and patching
- **Responsibilities** (planned):
  - Define fixture types (dimmer, RGB, mover, etc.)
  - Map fixtures to DMX addresses
  - Provide fixture parameter controls

### `src/media/` (Media Subsystem - Placeholder)
- **`mod.rs`**: Media playback manager
- **Responsibilities** (planned):
  - Audio playback (rodio)
  - Video playback (lumina-video)
  - Image display
  - Sync media with cues

### `src/show/` (Persistence)
- **`mod.rs`**: Show file format and I/O
- **ShowFile struct**: Serializable show data
- **Responsibilities**:
  - Save shows to JSON
  - Load shows from JSON
  - Manage show metadata

## Threading Model

**Single-threaded** (main thread only):
- egui runs on main thread
- All state updates happen in `App::update()`
- DMX output is synchronous (called each frame)
- Media playback will use internal threads (rodio/lumina handle this)

**Benefits**:
- Simple to reason about
- No synchronization needed
- Predictable frame timing
- Easy debugging

**Considerations**:
- DMX output must be fast (<16ms per frame for 60fps)
- Heavy operations (file I/O) should be async
- Media playback uses background threads internally

## Memory Layout

```
AppState (heap)
├── universes: Vec<Universe>           ~1KB per universe
├── dmx_backend: Box<dyn DmxBackend>   ~100 bytes
├── cue_list: CueList                  ~varies with # cues
│   └── cues: Vec<Cue>                 ~200 bytes per cue
├── playback: PlaybackEngine           ~2KB
│   ├── previous_values: [u8; 512]     512 bytes
│   └── target_values: [u8; 512]       512 bytes
├── media: MediaManager                ~varies
├── fixtures: FixtureLibrary           ~varies
└── ui_state: UiState                  ~20 bytes
```

**Total memory** (estimate):
- Base application: ~50 MB (egui + dependencies)
- Per universe: ~1 KB
- Per cue: ~200 bytes (+ channel values)
- Playback buffers: ~2 KB

## Performance Characteristics

### Critical Path (per frame)
1. egui event handling: <1ms
2. PlaybackEngine update: <1ms (simple linear interpolation)
3. Universe DMX send: <1ms (virtual backend)
4. egui rendering: 1-5ms (depends on complexity)

**Total frame time**: <10ms → 100+ fps possible

### Non-critical (async)
- File I/O: Save/load shows (100-500ms)
- Media loading: Audio/video file open (500ms-2s)
- DMX device scanning: USB enumeration (1-5s)

## Extension Points

### Adding a new DMX backend
1. Create `src/dmx/backends/my_backend.rs`
2. Implement `DmxBackend` trait
3. Export from `backends/mod.rs`
4. Instantiate in `App::new()` based on settings

### Adding a new fixture type
1. Define fixture profile (JSON or struct)
2. Add to fixture library
3. Create UI controls for parameters
4. Map parameters to DMX channels

### Adding a new cue action
1. Add field to `Cue` struct
2. Extend `PlaybackEngine` to handle action
3. Add UI for configuring action
4. Serialize/deserialize in ShowFile

## Data Format Specifications

### Decimal Precision

All floating-point values in saved show files are limited to **2 decimal places** for consistency and readability. This applies to:

- Cue numbers (e.g., `1.0`, `2.5`, `3.75`)
- Fade times (e.g., `2.50` seconds)
- Audio volume levels (e.g., `0.80` for 80%)
- Cross-trigger references (e.g., lighting cue triggers audio cue `3.00`)

**Implementation:** Custom serde serializers in `src/serde_helpers.rs` round all f32 values during serialization:
- `round_f32_2()` - for required f32 fields
- `round_option_f32_2()` - for `Option<f32>` fields

This prevents floating-point precision artifacts (like `0.800000011920929`) from appearing in saved JSON files.

**Usage in types:**
```rust
#[serde(serialize_with = "crate::serde_helpers::round_f32_2")]
pub fade_up: f32,

#[serde(serialize_with = "crate::serde_helpers::round_option_f32_2")]
pub triggers_audio_cue: Option<f32>,
```

## Future Optimizations

1. **DMX output threading**: Send DMX in parallel with rendering
2. **Cue preloading**: Load next cue during current fade
3. **GPU-accelerated effects**: Use shaders for complex fades
4. **Memory pooling**: Reuse fade buffers instead of allocating
5. **Incremental save**: Only write changed cues to disk
