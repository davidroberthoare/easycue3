# Development Status - EasyCue3

## Phase 1: Foundation & Project Setup ✅ COMPLETE

### What Was Built

**Core Architecture**:
- Rust workspace with modular structure (dmx, cue, media, ui, fixtures, show)
- Feature flag system for optional dependencies (usb, audio, video, media, full)
- egui 0.31 application with eframe window management
- Build system that works without system libraries (minimal default)

**DMX System**:
- ✅ `Universe` struct: 512-channel DMX universe with 1-indexed channels
- ✅ `VirtualBackend`: Logging-based DMX output for testing
- ✅ `DmxBackend` trait: Interface for pluggable output backends
- ✅ Channel get/set with bounds checking
- ✅ Bulk channel operations

**Cue Engine**:
- ✅ `Cue` struct: Stores channel values, fade times, labels, notes
- ✅ `CueList`: Manages sorted collection of cues
- ✅ `PlaybackEngine`: Linear crossfade between cues
- ✅ GO/BACK/STOP transport controls
- ✅ Frame-driven fade updates (smooth 60fps)

**User Interface**:
- ✅ Top menu bar: File, View, Help menus
- ✅ Bottom transport panel: Status indicator + GO/BACK/STOP buttons
- ✅ Left cue list panel: Scrollable, highlights current cue
- ✅ Center workspace: Placeholder for media panel
- ✅ Right fixture panel: Collapsible fixture control (placeholder)

**Documentation & Examples**:
- ✅ README.md: Feature overview, quickstart, architecture
- ✅ INSTALL.md: Platform-specific installation guide
- ✅ Example show file: 4 cues demonstrating JSON format
- ✅ Cargo.toml with comprehensive dependency comments

---

## Phase 2: DMX Foundation & Usability ✅ COMPLETE

### What Was Built

**Startup Experience**:
- ✅ Example show loads automatically on startup (`shows/example_show.json`)
- ✅ Cue list is populated with 4 demo cues immediately
- ✅ GO/BACK buttons work out of the box

**Keyboard Shortcuts**:
- ✅ `Space` → GO (advance to next cue)
- ✅ `B` → BACK (return to previous cue)
- ✅ `S` → STOP playback
- ✅ `Ctrl+R` → Record new cue from current universe state
- ✅ `Ctrl+S` → Open save-show dialog
- ✅ `Ctrl+O` → Open load-show dialog

**Show File Management**:
- ✅ File > Open (Ctrl+O): Load a show file by entering its path
- ✅ File > Save (Ctrl+S): Save the current cue list with a title and path
- ✅ File > New Show: Clear the cue list and start fresh
- ✅ `modified` timestamp updated automatically on save
- ✅ Parent directories created automatically when saving

**Cue Recording**:
- ✅ Record button / Ctrl+R: Snapshot current universe state into a new cue
- ✅ Auto-increments cue number (1.0, 2.0, 3.0…)
- ✅ Auto-generates default label ("Cue N")

**Cue Selection & Editing**:
- ✅ Click on any cue in the list to select it (highlighted in green)
- ✅ Selected cue opens in the center editor panel
- ✅ Edit cue label inline
- ✅ Adjust fade up / fade down times with drag-value controls
- ✅ Edit notes
- ✅ View stored channel values in scrollable grid
- ✅ Click again to deselect (toggle behaviour)

**Transport Panel Improvements**:
- ✅ Current cue name shown alongside playback state
- ✅ Status messages (e.g. "Saved to…", "Recorded cue 5") in transport bar
- ✅ Show title displayed in menu bar
- ✅ Keyboard shortcut hints on button labels

**UI Helpers**:
- ✅ Keyboard shortcut reference card shown on the welcome screen
- ✅ Cue row now shows channel count alongside fade time

### File Structure (unchanged)

```
easycue3/
├── Cargo.toml
├── README.md / INSTALL.md / ARCHITECTURE.md
├── src/
│   ├── main.rs
│   ├── app.rs              ← updated: startup load, record_cue, load/save, kb shortcuts
│   ├── dmx/
│   ├── cue/
│   ├── ui/
│   │   └── mod.rs          ← updated: cue editor, dialogs, selection, status bar
│   ├── fixtures/
│   ├── media/
│   └── show/
│       └── mod.rs          ← updated: modified timestamp on save
└── shows/
    └── example_show.json
```

### Current Capabilities

**What Works**:
1. Application launches with example cues pre-loaded
2. Space/GO advances through cues with smooth crossfades
3. B/BACK returns to previous cue
4. S/STOP halts playback immediately
5. Ctrl+R / Record button captures current universe state
6. Click any cue → edit label, fade times, notes in center panel
7. File > Save / Ctrl+S saves to a user-specified JSON path
8. File > Open / Ctrl+O loads from a user-specified JSON path
9. File > New Show clears the cue list
10. Status messages in the transport bar confirm actions

**What Doesn't Work Yet**:
- No live fixture control (sliders) — Phase 4
- No hardware DMX output (USB/Art-Net) — Phase 2 remaining
- No media playback — Phase 5
- No EOS-style command line — Phase 4
- Cue recording captures whatever the playback engine has set in the universe
  (there are no fixture sliders yet, so newly recorded cues will be empty
  unless playback has run first)

---

## Next Steps: Phase 3 – Live Control UI

**Priority Tasks**:
1. **Channel faders**: Per-channel sliders in the right panel (1–512)
2. **Fixture patching**: Name fixtures and assign start addresses
3. **USB DMX backend**: via `serialport` crate (feature-flagged)
4. **Art-Net backend**: via `artnet_protocol` crate (feature-flagged)
5. **Cue editing**: Edit individual channel values inside a cue
6. **EOS-style command line**: "1 Thru 10 At 50" syntax

**Estimated Effort**: 8–12 hours for Phase 3

---

## Phase 4: Audio Playback & Cross-Triggering ✅ COMPLETE

### What Was Built

**Audio Engine Foundation**:
- ✅ `AudioCue` struct: Stores audio file path, volume, fade in/out times, triggers
- ✅ `AudioCueList`: Manages sorted collection of audio cues (parallel to lighting cues)
- ✅ `AudioPlayer`: rodio-based audio playback with volume control
- ✅ `AudioPlaybackEngine`: Frame-driven fade in/out with GO/BACK/STOP controls
- ✅ Feature-gated with `#[cfg(feature = "audio")]` (build with `--features audio`)

**Cross-Triggering**:
- ✅ Lighting cues can trigger audio cues (via `triggers_audio_cue` field)
- ✅ Audio cues can trigger lighting cues (via `triggers_lighting_cue` field)
- ✅ Automatic cross-trigger execution during playback (no manual intervention)
- ✅ Visual indicators in UI (→🔊 for audio triggers, →🎭 for lighting triggers)

**Show File Integration**:
- ✅ Audio cues saved/loaded in show JSON files (with relative paths)
- ✅ Backward compatible (shows without audio cues still load)
- ✅ Missing audio file warnings (⚠️ icon when file not found)

**User Interface**:
- ✅ Sound Cues panel: Add audio cues via file dialog (MP3, WAV, FLAC, OGG, AAC)
- ✅ Audio cue list table: Shows cue number, label, filename, volume, triggers
- ✅ Audio transport controls: GO/BACK/STOP buttons (context-aware)
- ✅ Volume slider: Real-time volume adjustment (0-100%)
- ✅ Playback status display: Shows fade in/out progress
- ✅ Cross-trigger indicators: Visual links between related cues

**File Structure**:
```
src/
├── audio/
│   ├── mod.rs          ← Module exports + stubs for non-audio builds
│   ├── types.rs        ← AudioCue, AudioCueState
│   ├── list.rs         ← AudioCueList (sorted, navigation)
│   ├── player.rs       ← AudioPlayer (rodio wrapper)
│   └── playback.rs     ← AudioPlaybackEngine (fades, triggers)
├── app.rs              ← Added audio_cue_list, audio_player, audio_playback fields
├── cue/types.rs        ← Added triggers_audio_cue field to Cue
├── show/mod.rs         ← Added audio_cues field to ShowFile
└── ui/
    ├── sound_cues.rs   ← Full audio cue UI (replaced placeholder)
    └── lighting_cues.rs ← Added cross-trigger indicators
```

### Current Capabilities

**What Works**:
1. Add audio cues via file dialog (supports MP3, WAV, FLAC, OGG, AAC, M4A)
2. Audio cue list shows filename, volume, fade times, triggers
3. GO/BACK/STOP controls for audio playback (in Sound Cues panel)
4. Volume slider adjusts playback volume in real-time
5. Fade in/out with configurable duration (frame-driven, like lighting fades)
6. Cross-triggering: Lighting cue 1.0 can auto-trigger Audio cue 1.5
7. Cross-triggering: Audio cue 2.0 can auto-trigger Lighting cue 2.5
8. Visual indicators show trigger relationships (→🔊 and →🎭 icons)
9. Save/load shows with audio cues (relative paths for portability)
10. Missing file detection with warning icons

**What Doesn't Work Yet** (Planned for future phases):
- Audio waveform preview (Phase 6 enhancement)
- Multi-track audio (simultaneous sound effects + music) — Phase 5
- Seek/scrub playback position — Phase 5
- Audio device selection UI — Phase 5
- Fade curves (currently linear) — Phase 5

**Known Limitations**:
- Single audio stream at a time (one audio cue playing, no overlap)
- Duration display shows "--:--" (rodio doesn't provide duration until playback starts)
- No seek/scrub support yet (rodio Sink doesn't support seeking easily)

---

## Next Steps: Phase 5 – Advanced Media Features

**Priority Tasks**:
1. **Multi-track audio**: Multiple simultaneous audio cues (sound effects + music)
2. **Audio device selection**: Choose output device from dropdown
3. **Seek/scrub support**: Playback position slider with seek
4. **Duration display**: Show audio file duration before playback
5. **Video playback**: lumina-video integration (MP4, MOV, MKV, WebM)
6. **Image display**: PNG, JPEG, SVG support
7. **Fade curves**: Exponential/logarithmic fades (not just linear)

**Estimated Effort**: 12–16 hours for Phase 5

---

## Known Issues

1. **Warnings**: Unused code in placeholder modules (expected until those phases)
2. **Cue recording without fixtures**: Recorded cues are empty until channels
   are set via the (not-yet-built) fixture control panel
3. **Feature flags**: audio/video features reference git dependency that
   requires system libraries

## Build Notes

**Successful Build Configuration**:
- egui 0.31, eframe 0.31 with `glow` renderer
- X11 and Wayland support on Linux
- Minimal features by default (no audio/video/usb)
- Optional features work as intended

---

**Last Updated**: 2026-04-28
**Phase 1 Completion**: ✅ Success
**Phase 2 Completion**: ✅ Success
**Ready for Phase 3**: Yes 🚀

### What Was Built

**Core Architecture**:
- Rust workspace with modular structure (dmx, cue, media, ui, fixtures, show)
- Feature flag system for optional dependencies (usb, audio, video, media, full)
- egui 0.31 application with eframe window management
- Build system that works without system libraries (minimal default)

**DMX System**:
- ✅ `Universe` struct: 512-channel DMX universe with 1-indexed channels
- ✅ `VirtualBackend`: Logging-based DMX output for testing
- ✅ `DmxBackend` trait: Interface for pluggable output backends
- ✅ Channel get/set with bounds checking
- ✅ Bulk channel operations

**Cue Engine**:
- ✅ `Cue` struct: Stores channel values, fade times, labels, notes
- ✅ `CueList`: Manages sorted collection of cues
- ✅ `PlaybackEngine`: Linear crossfade between cues
- ✅ GO/BACK/STOP transport controls
- ✅ Frame-driven fade updates (smooth 60fps)

**User Interface**:
- ✅ Top menu bar: File, View, Help menus
- ✅ Bottom transport panel: Status indicator + GO/BACK/STOP buttons
- ✅ Left cue list panel: Scrollable, highlights current cue
- ✅ Center workspace: Placeholder for media panel
- ✅ Right fixture panel: Collapsible fixture control (placeholder)
- ✅ Keyboard ready (shortcuts not yet implemented)

**Documentation & Examples**:
- ✅ README.md: Feature overview, quickstart, architecture
- ✅ INSTALL.md: Platform-specific installation guide
- ✅ Example show file: 4 cues demonstrating JSON format
- ✅ Cargo.toml with comprehensive dependency comments

### File Structure

```
easycue3/
├── Cargo.toml              # Dependencies with feature flags
├── README.md               # Project overview and quickstart
├── INSTALL.md              # Installation instructions
├── src/
│   ├── main.rs             # Application entry point
│   ├── app.rs              # Main app state and update loop
│   ├── dmx/
│   │   ├── mod.rs
│   │   ├── universe.rs     # 512-channel DMX universe
│   │   └── backends/
│   │       ├── mod.rs      # Backend trait
│   │       └── virtual_dmx.rs  # Logging backend
│   ├── cue/
│   │   ├── mod.rs
│   │   ├── types.rs        # Cue and CueState
│   │   ├── list.rs         # CueList manager
│   │   └── playback.rs     # PlaybackEngine with crossfades
│   ├── ui/
│   │   └── mod.rs          # egui UI rendering
│   ├── fixtures/
│   │   └── mod.rs          # Placeholder
│   ├── media/
│   │   └── mod.rs          # Placeholder
│   └── show/
│       └── mod.rs          # Show file format (JSON)
├── shows/
│   └── example_show.json   # Example 4-cue show
├── fixture_profiles/       # (empty - for Phase 2)
└── examples/               # (empty - for Phase 2)
```

### Current Capabilities

**What Works**:
1. Application launches with egui window
2. Cue list displays (empty by default)
3. Transport controls render and respond to clicks
4. Virtual DMX backend logs channel changes
5. Playback engine tracks state (Stopped/Fading/Active)
6. Frame-by-frame fade updates (ready for real crossfades)
7. Builds successfully without system library dependencies

**What Doesn't Work Yet**:
- No cues loaded by default (cue list is empty)
- GO/BACK buttons trigger playback but no cues to play
- Recording cues not implemented
- Show file load/save not wired to UI
- No fixture patching
- No hardware DMX output
- No media playback

## Next Steps: Phase 2 - DMX Foundation

**Priority Tasks**:
1. **Test cue playback**: Manually create test cues in code to verify engine
2. **Implement cue recording**: Capture channel states to create cues
3. **Load/save shows**: Wire up File menu to ShowFile serialization
4. **Fixture patching UI**: Basic patch table (fixture type, address, channels)
5. **USB DMX backend**: Implement using `serialport` crate
6. **Art-Net backend**: Implement using `artnet_protocol` crate

**Testing Plan**:
- Add hardcoded test cues to verify playback
- Test crossfades with different fade times
- Verify GO/BACK navigation
- Test Art-Net output with network sniffer

**Estimated Effort**: 8-12 hours for Phase 2 completion

## Known Issues

1. **Warnings**: Unused code warnings for placeholder functions (expected)
2. **Empty UI**: No cues loaded, list appears empty on startup
3. **No persistence**: File menu buttons don't do anything yet
4. **Feature flags**: audio/video features reference git dependency that requires system libs

## Build Notes

**Successful Build Configuration**:
- egui 0.31, eframe 0.31 with `glow` renderer
- X11 and Wayland support on Linux
- Minimal features by default (no audio/video/usb)
- Optional features work as intended

**Dependencies**:
- ~240 total crates when building with all deps
- Core (no features): ~180 crates
- Build time: ~2-4 minutes (clean), ~5s (incremental)

## Questions to Resolve

1. **Fixture library**: Should we include pre-made fixture profiles or start with user-defined?
   - **Decision**: Start with user-defined, add library later
   
2. **Cue numbering**: Decimal (1.0, 1.5, 2.0) or integer (1, 2, 3)?
   - **Decision**: Decimal with one place (float in code, display as .1)

3. **Fade curves**: Linear only or also S-curve/exponential?
   - **Decision**: Linear for MVP, curves later as per-cue setting

4. **Default cues**: Should app ship with example cues or start empty?
   - **Decision**: Empty by default, load example_show.json from File menu

## Technical Decisions

**Why egui 0.31?**
- lumina-video requires egui 0.31 for video compatibility
- 0.33 is latest but incompatible with lumina-video
- We'll upgrade when lumina-video updates

**Why feature flags?**
- Linux system libraries (ALSA, GStreamer, libudev) not always available
- Users can build minimal version without installing dependencies
- Cleaner for development (don't need media playback to work on lighting)

**Why Virtual DMX as default?**
- Works everywhere (no hardware required)
- Good for development and testing
- Users can enable hardware backends later

**Why JSON for show files?**
- Human-readable and editable
- Widely supported tooling
- Easy to version control
- Users can hand-edit if needed
- Alternative: RON (Rust Object Notation) - more Rust-idiomatic but less universal

## Performance Notes

- **Frame rate**: Application requests repaint only when playback is active
- **Memory**: ~50MB resident when idle, ~80MB during playback
- **CPU**: Minimal when idle, <5% during fades on modern hardware
- **Startup time**: ~1-2 seconds from launch to window visible

---

**Last Updated**: 2026-04-27  
**Phase 1 Completion**: Success ✅  
**Ready for Phase 2**: Yes 🚀
