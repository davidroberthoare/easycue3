# Phase 4: Audio Playback & Cross-Triggering - Implementation Plan

Add audio playback capabilities to EasyCue3 with rodio, enabling sound cues that integrate with the existing lighting cue system. The audio system will be isolated from existing DMX/lighting code, using a parallel architecture pattern.

**TL;DR:** Create a standalone audio subsystem (`src/audio/`) with AudioCue types, an AudioPlayer wrapping rodio, and cross-triggering between lighting and audio cues. Extend the show file format and UI to display integrated timelines while keeping audio and lighting systems decoupled.

---

## Steps

### Phase A: Audio Engine Foundation (3 files, no existing modifications)

1. **Create `src/audio/mod.rs`** — Audio module exports and public interface
   - Export AudioPlayer, AudioCue, AudioCueList types
   - Similar structure to `src/cue/mod.rs`

2. **Create `src/audio/types.rs`** — AudioCue data structure (*parallel to `src/cue/types.rs`*)
   - `AudioCue` struct with: number (f32), label, audio_path (PathBuf), volume (f32 0-1), fade_in (f32 seconds), fade_out (f32 seconds), notes, triggers (optional lighting cue number)
   - `AudioCueState` enum: Stopped, FadingIn, Playing, FadingOut (*parallel to CueState*)
   - Serialize/Deserialize derives for show file persistence

3. **Create `src/audio/list.rs`** — AudioCueList management (*parallel to `src/cue/list.rs`*)
   - `AudioCueList` struct managing Vec<AudioCue>
   - Methods: add_cue, remove_cue, get_cue, next_index, previous_index
   - Keep sorted by cue number (same pattern as lighting cues)

4. **Create `src/audio/player.rs`** — rodio audio playback engine
   - `AudioPlayer` struct with rodio OutputStream, OutputStreamHandle, and current Sink
   - Methods: play_file(path, volume, fade_in), pause, resume, stop, set_volume, get_position, get_duration, is_playing
   - Handle audio device selection (list_devices, set_device)
   - Error handling for missing files, unsupported formats, device errors

### Phase B: Integration with App State (2 files modified)

5. **Extend `src/app.rs`** — Add audio state to EasyCueApp (*depends on Phase A*)
   - Add fields: `audio_cue_list: AudioCueList`, `audio_player: AudioPlayer`, `current_audio_cue_index: Option<usize>`
   - Initialize in `EasyCueApp::new()` (behind #[cfg(feature = "audio")])
   - Add method: `record_audio_cue(path: PathBuf)` for adding audio cues

6. **Extend `src/show/mod.rs`** — Add audio cues to ShowFile (*depends on Phase A*)
   - Add field `audio_cues: Vec<AudioCue>` to ShowFile struct
   - Update save/load to serialize audio cues
   - Add field for media file paths (relative to show directory)

### Phase C: Audio Playback & Cross-Triggering (2 files)

7. **Create `src/audio/playback.rs`** — AudioPlaybackEngine (*parallel to `src/cue/playback.rs`, depends on Phase A*)
   - `AudioPlaybackEngine` managing audio cue execution
   - Handle fade in/out using rodio's volume control over time
   - Methods: go, back, stop, update (called each frame to update fade progress)
   - Check for cross-triggers: when audio cue starts/ends, trigger specified lighting cue

8. **Add cross-trigger logic to `src/cue/playback.rs`** (*depends on Step 7*)
   - Extend `Cue` struct in `src/cue/types.rs` with optional `triggers_audio_cue: Option<f32>` field
   - When lighting cue executes in PlaybackEngine::go, check for audio trigger and queue it
   - Communication via app state (no direct coupling between playback engines)

### Phase D: UI Implementation (1 file modified, 1 new file) *parallel with Phase C*

9. **Update `src/ui/sound_cues.rs`** — Replace placeholder with functional audio cue list (*depends on Phase B*)
   - Display audio cue list (number, label, file name, duration, volume)
   - Click to select audio cue (shows in properties panel)
   - Add/Remove audio cue buttons
   - Import button (rfd file dialog for audio files)
   - Visual indicators for missing audio files (red warning icon)
   - Show cross-trigger relationships with icons (→ 🎭 or → 🔊)

10. **Create `src/ui/audio_controls.rs`** — Audio-specific transport controls (*parallel with step 9*)
    - Play/Pause/Stop buttons for selected audio cue
    - Playback position slider (seek support)
    - Volume slider (0-100%)
    - Device selector dropdown (list audio output devices)
    - Fade in/out duration controls
    - Could be integrated into Controls panel or separate section

### Phase E: Enhanced UX & Polish *parallel with Phase D*

11. **Update `src/ui/properties.rs`** — Display audio cue properties when selected (*parallel with step 9*)
    - If selected cue is audio: show path, volume, fade in/out, trigger settings
    - Audio file picker button
    - Cross-trigger selector (dropdown of lighting cue numbers)
    - Preview button (play audio once without adding to timeline)

12. **Update `src/ui/lighting_cues.rs`** — Show cross-trigger indicators (*parallel with step 9*)
    - Display icon (→ 🔊) next to lighting cues that trigger audio
    - Visual link between related cues

---

## Verification

1. **Compile check**: `cargo build --features audio` compiles without errors
2. **Audio playback test**: Create show with one audio cue, press GO in sound cues panel, verify audio plays via rodio
3. **Volume control test**: Adjust volume slider while playing, verify volume changes in real-time
4. **Fade test**: Set fade_in=2.0, fade_out=2.0, verify audio volume smoothly ramps up/down
5. **Cross-trigger test**: Create lighting cue 1.0 with `triggers_audio_cue: Some(1.5)`, verify pressing GO on lighting cue auto-triggers audio cue 1.5
6. **Save/load test**: Create show with 2 audio cues, save to JSON, close app, reload, verify audio cues persist
7. **Missing file warning**: Reference non-existent audio file, verify UI shows red warning icon
8. **Device selection**: Open audio device dropdown, select different output, play audio, verify output changes

---

## Decisions

**Separate AudioCue vs. Unified Cue Type**
- **Decision**: Use separate `AudioCue` type parallel to `Cue` (lighting)
- **Rationale**: Keeps audio system isolated, easier to test independently, no risk of breaking existing lighting code. Clean separation of concerns.

**Cross-Triggering Communication**
- **Decision**: Use app-level state polling (check triggers after each GO/BACK command)
- **Rationale**: No direct coupling between playback engines, maintains separation. Each engine only knows about its own cue type.

**Audio File Paths in Show Files**
- **Decision**: Store relative paths from show file directory
- **Rationale**: Makes shows portable (can move show folder with audio files intact). Matches industry standard (QLab uses relative paths).

**Feature Flag Behavior**
- **Decision**: All Phase 4 code behind `#[cfg(feature = "audio")]` guards
- **Rationale**: Users without audio dependencies (ALSA on Linux) can still build. Matches existing pattern for USB DMX.

**Fade Implementation**
- **Decision**: Use rodio's Sink::set_volume in update loop (frame-driven), not rodio's fade_in() method
- **Rationale**: Matches lighting fade pattern (frame-driven in update()), allows consistent UI progress display, gives manual control over fade curve

---

## Further Considerations

1. **Audio device hot-plugging** — If user unplugs audio device during playback, should app auto-switch to default device or pause? **Recommendation**: Pause playback, show error status, require manual device re-selection for safety.

2. **Multiple simultaneous audio cues** — Should we support multiple audio cues playing at once (sound effects + music)? **Recommendation**: Phase 4 = single audio stream (matches Phase 4 outline "simple audio playback"). Phase 5+ can add multi-track support with separate Sinks.

3. **Audio waveform preview** — Should UI display audio waveform or just filename? **Recommendation**: Phase 4 = filename + duration only (simpler). Waveform visualization can be Phase 6 enhancement.

4. **Keyboard shortcuts for audio** — Should audio cues respond to Space/B/S like lighting cues? **Recommendation**: Yes, but only when Sound Cues panel is active (use `app.ui_state.active_pane == Some(TabKind::SoundCues)`). This matches context-aware command system already in place.
