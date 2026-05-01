# EasyCue3 Project Outline

## Vision Statement

EasyCue3 is a **theatrical lighting and media console** designed for small-to-medium scale productions, educational theatre, community playhouses, and hobbyists. It combines the intuitive lighting control of ETC's EOS consoles with the powerful media playback capabilities of QLab, while remaining **free, open-source, and cross-platform**.

### Core Philosophy

- **Simplicity First:** Learn in 10 minutes, master in a day
- **Hardware Flexible:** Works with virtual DMX, USB interfaces, or Art-Net
- **Media Integrated:** Lighting and sound/video cues in one timeline
- **Beginner Friendly:** No manual required for basic operation
- **Professional Grade:** Capable of running real productions

---

## Target Users

1. **Educational Institutions:**
   - High school drama departments
   - College theatre programs
   - Students learning lighting design

2. **Small Venues:**
   - Community theatres (50-300 seats)
   - Church stages
   - Small performance spaces
   - Rehearsal studios

3. **Hobbyists & Designers:**
   - Lighting designers pre-programming at home
   - DIY stage lighting enthusiasts
   - Event producers on a budget

4. **Developers:**
   - Theatre technicians learning programming
   - Students studying real-time control systems

---

## Technical Overview

### Architecture

**Platform:** Cross-platform desktop application (macOS, Linux, Windows)

**Core Technologies:**
- **Language:** Rust (for memory safety, performance, and reliability)
- **UI Framework:** egui (immediate-mode GUI, 60fps updates)
- **DMX Output:** Multi-backend system (Virtual, USB, Art-Net, sACN)
- **Audio:** rodio (Rust audio library, supports MP3/WAV/FLAC/AAC)
- **Video:** lumina-video (GStreamer-based video playback)
- **Serialization:** JSON show files (human-readable, git-friendly)

**Key Design Decisions:**

1. **Immediate-Mode GUI (egui):**
   - Updates every frame (60 FPS)
   - Responsive, real-time feedback
   - Simpler than retained-mode GUI (no event handling complexity)
   - Perfect for control surfaces with constantly changing data

2. **Rust Language:**
   - No garbage collection pauses during live shows
   - Memory safety prevents crashes (critical for live performance)
   - Excellent cross-platform support
   - Growing ecosystem of audio/video libraries

3. **Modular Backend System:**
   - Swap DMX hardware without code changes
   - Test shows with virtual DMX (no hardware required)
   - Feature flags allow building without system dependencies

4. **JSON Show Files:**
   - Human-readable (can edit in text editor if needed)
   - Version control friendly (Git can track changes)
   - Easy to parse/generate from other tools
   - Self-documenting format

---

## Multi-Phase Development Plan

### Phase 1: Foundation ✅ COMPLETE

**Goal:** Establish core architecture and prove the concept works.

**Deliverables:**
- ✅ Rust project structure with cargo workspace
- ✅ DMX Universe (512 channels, 1-indexed)
- ✅ Virtual DMX backend (logging-based, for testing)
- ✅ Basic Cue structure (channel values, fade times, labels)
- ✅ CueList (sorted collection of cues)
- ✅ PlaybackEngine (linear crossfade between cues)
- ✅ Basic UI layout (cue list, transport controls)
- ✅ GO/BACK/STOP transport buttons
- ✅ egui window with menu bar

**Outcome:** 
A working prototype that can load a show file, display cues, and fade between them with virtual DMX output. No hardware required.

---

### Phase 2: DMX Foundation & Usability ✅ COMPLETE

**Goal:** Make the app actually usable for creating and running simple shows.

**Deliverables:**
- ✅ Example show loads on startup
- ✅ Keyboard shortcuts (Space=GO, B=BACK, S=STOP)
- ✅ Show file management (New, Open, Save with dialogs)
- ✅ Cue recording (Ctrl+R: snapshot current universe state)
- ✅ Cue selection and editing (click to select, edit in center panel)
- ✅ Edit cue labels, fade times, and notes
- ✅ View channel values in selected cue
- ✅ Transport panel shows current cue name and status
- ✅ Keyboard shortcut reference card

**Outcome:**
You can now create a simple lighting show from scratch, record cues, edit them, save/load shows, and play them back - all without writing code or editing JSON.

---

### Phase 3: DMX Hardware & Fixture Control 🚧 IN PROGRESS

**Goal:** Support real DMX hardware and provide fixture control interfaces.

**Deliverables:**
- 🚧 USB DMX backend (FTDI-based devices, Enttec OpenDMX)
- 🚧 Art-Net backend (DMX over Ethernet, up to 16 universes)
- 🚧 sACN backend (Streaming ACN, industry standard)
- 🚧 Universe selector (switch between Universe 1 and 2)
- 🚧 Fixture profiles (Generic Dimmer, RGB, RGBW, Moving Light)
- 🚧 Fixture patching UI (assign fixtures to channels)
- 🚧 Live fixture control panel (sliders for intensity, color, position)
- 🚧 Intensity wheel (select fixtures, adjust intensity with mouse/arrow keys)
- 📋 EOS-style command line ("1 Thru 10 At 50 Enter")
- 📋 Channel monitor (visual grid showing all 512 channels)

**Hardware Support Matrix:**

| Backend       | Universes | Requires Hardware | Protocol          |
|---------------|-----------|-------------------|-------------------|
| Virtual DMX   | 1-2       | No (logging only) | N/A               |
| USB DMX       | 1         | Yes (USB adapter) | FTDI serial       |
| Art-Net       | 1-16      | No (uses network) | Art-Net 4         |
| sACN (E1.31)  | 1-63999   | No (uses network) | ANSI E1.31        |

**Fixture Library (Initial):**
- **Generic Dimmer:** 1 channel (intensity)
- **RGB LED:** 3 channels (red, green, blue)
- **RGBW LED:** 4 channels (red, green, blue, white)
- **Moving Light (Basic):** 8 channels (pan, tilt, intensity, color wheel, gobo, etc.)

**Outcome:**
Connect real DMX hardware, patch fixtures, and control them with sliders and a command line. Record these live adjustments as cues.

---

### Phase 4: Audio Playback & Cross-Triggering 📋 PLANNED

**Goal:** Add simple audio playback and allow lighting/audio cues to trigger each other.

**Deliverables:**

**Audio Playback:**
- 📋 Audio cues (MP3, WAV, FLAC, AAC, OGG)
- 📋 Play/Pause/Stop controls
- 📋 Volume control (per-cue volume)
- 📋 Fade in/out (audio crossfades)
- 📋 Playback position indicator

**Cross-Triggering:**
- 📋 Lighting cue can trigger an audio cue (on GO)
- 📋 Audio cue can trigger a lighting cue (on start/end)
- 📋 Trigger settings in cue properties panel
- 📋 Visual indicators showing cue relationships

**Integrated Timeline:**
```
Cue 1.0: [LIGHTS] House Lights                  Fade: 2s
Cue 2.0: [LIGHTS] Blackout                      Fade: 3s → Triggers Audio 2.5
Cue 2.5: [AUDIO]  Overture (music.mp3)          Vol: 80%
Cue 3.0: [LIGHTS] Stage Wash                    Fade: 5s
Cue 4.0: [AUDIO]  Sound Effect (thunder.wav)    Vol: 100% → Triggers Lights 4.5
Cue 4.5: [LIGHTS] Lightning Flash               Fade: 0.1s
```

**Media Management:**
- 📋 Audio file browser (view all audio files in show folder)
- 📋 Drag-and-drop audio import
- 📋 Missing audio warnings (if files are moved/deleted)

**Outcome:**
Run simple shows with integrated audio and lighting from a single timeline. Audio cues can automatically trigger lighting changes and vice versa, enabling coordinated effects like thunder sound with lightning flashes.

---

### Phase 5: Advanced Cueing & Groups 📋 PLANNED

**Goal:** Add professional features for complex shows.

**Deliverables:**
- 📋 Groups (save fixture selections: "All House Lights", "Stage Left RGB")
- 📋 Presets/Palettes (store and recall colors, positions, beams)
- 📋 Cue-only mode (record only selected channels)
- 📋 Tracking vs. Non-tracking cues
- 📋 Block cues (prevent tracking from previous cues)
- 📋 Cue parts (split a cue into multiple follow-ups)
- 📋 Follow cues (Cue 1.5 follows Cue 1 after 2 seconds)
- 📋 Auto-follow macros (automated sequences)
- 📋 Cue copying/moving/renaming
- 📋 Blind mode (edit cues without affecting live output)

**Terminology for Beginners:**

- **Group:** A saved selection of fixtures (e.g., "All Front Lights")
- **Preset/Palette:** A saved attribute (e.g., "Red Color", "Center Position")
- **Tracking:** If you don't change a channel in a cue, it keeps its previous value
- **Non-tracking:** Every cue explicitly sets all its channels (no inheritance)
- **Block Cue:** Stops tracking from previous cues (fresh start)
- **Follow Cue:** Automatically triggers after the previous cue + delay
- **Blind Mode:** Edit cues in the background without affecting the live show

**Outcome:**
Power users can build complex shows with sophisticated timing and organization, while beginners can ignore these features entirely.

---

### Phase 6: Video & Image Playback 📋 FUTURE

**Goal:** Extend media capabilities with video and still images.

**Deliverables:**

**Video Playback:**
- 📋 Video cues (MP4, MOV, MKV, WebM, AVI)
- 📋 Embedded video player window
- 📋 External video output (second monitor/projector)
- 📋 Video transport controls (play/pause/stop/seek)
- 📋 Fade to/from black
- 📋 Video looping
- 📋 Playback rate control (slow-mo, fast-forward)

**Image Display:**
- 📋 Still image cues (PNG, JPEG, SVG, GIF)
- 📋 Image slideshow transitions
- 📋 Crossfade between images
- 📋 Image scaling/positioning

**Enhanced Timeline:**
```
Cue 1.0: [LIGHTS] House Lights                  Fade: 2s
Cue 2.0: [LIGHTS] Blackout                      Fade: 3s
Cue 2.5: [AUDIO]  Overture (music.mp3)          Fade: 0s
Cue 3.0: [LIGHTS] Stage Wash                    Fade: 5s
Cue 4.0: [VIDEO]  Title Card (intro.mp4)        Fade: 1s
Cue 5.0: [LIGHTS] Act 1 Scene 1                 Fade: 3s
```

**Media Management:**
- 📋 Video/image browser (view all media files in show folder)
- 📋 Drag-and-drop media import
- 📋 Missing media warnings
- 📋 Media preview thumbnails

**Outcome:**
Full QLab-style capability with integrated lighting, audio, video, and images all controlled from a single timeline.

---

### Phase 7: Effects & Dynamics 📋 FUTURE

**Goal:** Add moving effects and dynamic lighting behaviors.

**Deliverables:**

**Effects Engine:**
- 📋 Chase effects (sequential flashing of fixtures)
- 📋 Rainbow effects (color cycling)
- 📋 Sine wave effects (smooth pulsing)
- 📋 Random/strobe effects
- 📋 Effect speed/direction control
- 📋 Effect intensity scaling
- 📋 Apply effects to groups

**Live Control:**
- 📋 Grand Master fader (scale all intensities)
- 📋 Submaster faders (manual intensity groups)
- 📋 Bump buttons (flash fixtures)
- 📋 Blackout button (instant all-off)
- 📋 MIDI controller support (map hardware faders/buttons)
- 📋 OSC support (control from iPad/phone)

**Timecode Sync:**
- 📋 MIDI timecode (MTC) input
- 📋 Linear timecode (LTC) audio input
- 📋 OSC timecode
- 📋 Timecode display and offset
- 📋 Trigger cues at specific timecode values

**Outcome:**
Create dynamic, evolving lighting looks and synchronize with audio/video timecode for perfectly timed shows.

---

### Phase 8: Magic Sheets & Visualization 📋 FUTURE

**Goal:** Add custom visual control layouts and 3D visualization.

**Deliverables:**

**Magic Sheets:**
- 📋 Custom control layouts (drag-and-drop fixture buttons)
- 📋 Background images (stage plot, venue layout)
- 📋 Grouped controls (area-based control)
- 📋 Color-coded buttons
- 📋 Touch-screen friendly (large buttons)

**3D Visualization:**
- 📋 3D stage view (see fixtures in space)
- 📋 Real-time lighting preview (see colors/intensities)
- 📋 Camera controls (orbit, zoom, pan)
- 📋 Export visualization as video (pre-viz for directors)

**Plot Import:**
- 📋 Import from Vectorworks
- 📋 Import from AutoCAD DXF
- 📋 Manual fixture placement

**Outcome:**
Visualize your lighting design in 3D before setting up hardware. Control fixtures with custom touch-friendly layouts.

---

### Phase 9: Collaboration & Cloud 📋 FUTURE

**Goal:** Enable multi-user workflows and remote control.

**Deliverables:**
- 📋 Git integration (commit shows to version control)
- 📋 Show notes/comments (annotate cues)
- 📋 Remote control web UI (control from tablet/phone)
- 📋 Multi-user mode (separate lighting/sound operators)
- 📋 Show sharing platform (community show library)
- 📋 Cloud backup (automatic show backups)

**Outcome:**
Collaborate with team members, control remotely, and share shows with the community.

---

## Scope Boundaries

### What EasyCue3 IS:

- ✅ Small-to-medium scale theatrical console (up to ~200 fixtures)
- ✅ Educational tool for learning lighting/sound control
- ✅ Integrated lighting + media solution
- ✅ Pre-programming and show design tool
- ✅ Budget-friendly alternative to commercial consoles

### What EasyCue3 IS NOT:

- ❌ Large-scale concert console (1000+ fixtures, festivals)
- ❌ Professional broadcasting tool (dedicated video switcher)
- ❌ Live music mixing board (use Reaper, Ardour, etc.)
- ❌ Pixel mapping / LED screen controller (specialized tools exist)
- ❌ Hardware console (EasyCue3 is software-only)

### Hardware Limitations:

- **Universe Count:** 2-16 universes (not 100+ like MA Lighting)
- **Fixture Count:** ~200 fixtures max (not thousands)
- **Update Rate:** 40 Hz DMX refresh (not 1000 Hz)
- **Precision:** 8-bit DMX (0-255), not 16-bit fine control

These limitations keep the software simple, maintainable, and appropriate for the target audience.

---

## Success Metrics

### Phase 2 (Current):
- ✅ Can create and run a simple 10-cue show in under 5 minutes
- ✅ Show files load without errors
- ✅ Fades are smooth (no visible stepping)
- ✅ UI is responsive (60 FPS)

### Phase 3 Goals:
- [ ] Successfully control real DMX fixtures (tested with 3+ hardware types)
- [ ] Fixture patching takes under 2 minutes for 10 fixtures
- [ ] Command line parsing is intuitive (90% success rate for new users)
- [ ] No dropped DMX frames during playback

### Phase 4 Goals:
- [ ] Audio stays in sync with lighting cues (±100ms accuracy)
- [ ] Audio crossfades are smooth (no popping/clicking)
- [ ] Audio files load quickly (<1 second for typical files)
- [ ] Cross-triggering works reliably (lighting→audio, audio→lighting)

### Phase 6 Goals:
- [ ] Video playback is smooth (30 FPS minimum)
- [ ] Video files load quickly (<2 seconds)
- [ ] Image transitions are clean (no visible artifacts)

### Long-Term Goals:
- [ ] Used in 10+ real productions
- [ ] 100+ GitHub stars
- [ ] Active community contributing fixture profiles
- [ ] Tutorial videos available on YouTube
- [ ] Mentioned in educational curricula

---

## Use Case Examples

### Example 1: High School Play

**Setup:**
- 20 conventional fixtures (dimmers on DMX)
- USB DMX interface
- Laptop running EasyCue3
- 30 lighting cues
- 10 sound cues (pre-show music, sound effects)

**Workflow:**
1. Student tech crew patches 20 dimmers in EasyCue3
2. They create cues during tech rehearsal by adjusting sliders and pressing Ctrl+R
3. Sound cues are imported by dragging MP3 files into the cue list
4. During the show, operator presses Spacebar to advance cues
5. Backup show file is saved to USB drive

**Result:** Entire show runs from one laptop, one operator.

---

### Example 2: Community Theatre

**Setup:**
- 40 LED fixtures (RGB) on Art-Net network
- 10 moving lights
- Projection screen with video cues
- Desktop PC running EasyCue3
- 100+ cues with complex fades

**Workflow:**
1. Lighting designer pre-programs at home using virtual DMX
2. At the venue, they switch to Art-Net backend (no code changes)
3. Fixtures are patched and grouped ("All Blues", "Downstage Specials")
4. Designer uses command line to adjust groups: "All Blues At 70 Enter"
5. Video cues are added for projection mapping
6. Show file is version-controlled in Git (director can review changes)

**Result:** Professional-looking design with free software.

---

### Example 3: Student Learning Project

**Setup:**
- No hardware (virtual DMX only)
- Student's laptop
- Learning lighting design concepts

**Workflow:**
1. Student downloads EasyCue3 (no license, no cost)
2. They load the example show and experiment with cues
3. They create a 10-cue show from scratch
4. Virtual DMX logs show them exactly what's being output
5. They learn fade curves, crossfades, and timing

**Result:** Risk-free learning environment with immediate feedback.

---

## Technical Challenges & Solutions

### Challenge 1: Smooth Crossfades at 60 FPS

**Problem:** Lighting cues must fade smoothly without visible stepping.

**Solution:**
- Use frame delta time (not fixed timestep) for fade calculations
- Interpolate channel values using float math, convert to u8 at the end
- Update universe 60 times per second (synced to UI refresh)
- Send DMX updates 40 times per second (industry standard)

**Code Pattern:**
```rust
// Calculate fade progress (0.0 to 1.0)
let progress = elapsed_time / fade_time;
let progress = progress.clamp(0.0, 1.0);

// Interpolate between prev and next values
let prev = prev_cue.get_channel(ch).unwrap_or(0) as f32;
let next = next_cue.get_channel(ch).unwrap_or(0) as f32;
let current = prev + (next - prev) * progress;

// Write to universe
universe.set_channel(ch, current as u8);
```

---

### Challenge 2: Cross-Platform Media Playback

**Problem:** Audio/video libraries differ on Windows, macOS, Linux.

**Solution:**
- Use `rodio` for audio (pure Rust, cross-platform)
- Use `lumina-video` for video (GStreamer wrapper)
- Feature flags allow building without media support
- Graceful degradation if system libraries are missing

**Feature Flags:**
```toml
[features]
audio = ["rodio"]
video = ["lumina-video"]
media = ["audio", "video"]
```

---

### Challenge 3: Real-Time DMX Without Dropped Frames

**Problem:** UI updates shouldn't block DMX output.

**Solution:**
- Separate thread for DMX output (never blocks on UI)
- Lock-free communication via `Arc<Mutex<Universe>>`
- UI reads from universe, playback engine writes to universe
- DMX thread sends at consistent 40 Hz regardless of UI FPS

**Architecture:**
```
Main Thread (UI):              Playback Thread:           DMX Thread:
  ┌─────────┐                   ┌──────────┐              ┌────────┐
  │ egui UI │ ◄────reads────────┤ Universe │◄────writes───┤ Backend│
  │ 60 FPS  │                   │ (Mutex)  │              │ 40 Hz  │
  └─────────┘                   └──────────┘              └────────┘
       │                              │                        │
       └────── sends GO event ────────┘                        │
                                      │                        │
                                      └───────── updates ──────┘
```

---

### Challenge 4: Rust Learning Curve for Contributors

**Problem:** Rust's ownership system can be intimidating.

**Solution:**
- Comprehensive CONTRIBUTING.md with Rust basics
- Heavily commented code (especially for ownership patterns)
- Use high-level abstractions (egui, serde, anyhow)
- Provide example implementations for common tasks
- Friendly error messages from compiler (Rust is great at this!)

---

## Glossary of Theatre Terms

For developers unfamiliar with theatrical terminology:

- **DMX:** Digital Multiplex, the standard protocol for controlling stage lighting (1-512 channels per universe)
- **Universe:** A set of 512 DMX channels (like a MIDI port, but for lights)
- **Channel:** A single controllable parameter (e.g., intensity of one light)
- **Fixture:** A lighting device (LED light, moving head, fog machine, etc.)
- **Cue:** A preset lighting/sound state that you transition to
- **Fade:** Gradual transition between cues (measured in seconds)
- **GO:** Command to advance to the next cue
- **BACK:** Command to return to the previous cue
- **Blackout:** Turn all lights off instantly
- **Intensity:** Brightness level (0-100%, or 0-255 in DMX)
- **Patch:** Assign fixtures to DMX channels (e.g., "Moving Light 1 is on channels 10-17")
- **Group:** A saved selection of fixtures
- **Preset/Palette:** A saved attribute value (color, position, beam shape)
- **Art-Net:** Protocol for sending DMX over Ethernet (LAN)
- **sACN:** Streaming ACN, another DMX-over-Ethernet protocol
- **LX:** Abbreviation for "lighting" (from "electrics")
- **SFX:** Sound effects
- **QLab:** Industry-standard Mac software for theatre sound/video playback
- **EOS:** ETC's (Electronic Theatre Controls) lighting console family

---

## Contributing to This Outline

This document is a living roadmap. If you have ideas for features, use cases, or improvements, please:

1. Open a GitHub Issue to discuss the idea
2. Submit a Pull Request to update this document
3. Join discussions in GitHub Discussions

**Questions to consider:**
- Does this feature fit the "small-scale" scope?
- Will it benefit the target users (students, small theatres, hobbyists)?
- Is it technically feasible in Rust with current libraries?
- Does it overlap with existing tools (should we integrate instead)?

---

## License & Credits

EasyCue3 is dual-licensed under **MIT** or **Apache-2.0** (your choice).

**Inspired by:**
- **ETC EOS Family:** Intuitive command-line and UI design
- **QLab:** Integrated media playback timeline
- **GrandMA2:** Effects engine and preset system
- **Open Lighting Architecture (OLA):** DMX backend architecture

**Built with:**
- [Rust](https://www.rust-lang.org/) - Systems programming language
- [egui](https://github.com/emilk/egui) - Immediate-mode GUI
- [rodio](https://github.com/RustAudio/rodio) - Audio playback
- [lumina-video](https://github.com/lumina-video/lumina-video) - Video playback
- [serde](https://serde.rs/) - Serialization framework

---

**Last Updated:** Phase 2 Completion (April 2026)  
**Status:** Phases 1-2 complete, Phase 3 in progress  
**Next Milestone:** USB DMX support and fixture patching
