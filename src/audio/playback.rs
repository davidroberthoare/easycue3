//! Multi-track audio playback engine.
//!
//! Each audio cue gets its own Sink. Firing a new cue never stops existing ones;
//! each stream runs independently until its length timer expires, the file ends,
//! or stop_all() is called.

use crate::audio::{AudioCueState, AudioPlayer};
use crate::cue::Cue;
use rodio::{Decoder, Source};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn resolve_audio_path(path: &Path) -> PathBuf {
    if path.is_absolute() || path.exists() {
        return path.to_path_buf();
    }
    let media_path = PathBuf::from("media").join(path);
    if media_path.exists() {
        return media_path;
    }
    path.to_path_buf()
}

/// In-progress per-stream volume ramp driven by an Adjust cue.
struct VolumeAdjust {
    start_vol: f32,
    target_vol: f32,
    fade_time: f32,
    start: Instant,
    stop_when_complete: bool,
}

struct ActiveAudioStream {
    cue_id: u32,
    sink: rodio::Sink,
    state: AudioCueState,
    base_volume: f32,
    fade_in_duration: f32,
    fade_out_duration: f32,
    fade_start: Option<Instant>,
    /// Optional max play time from play_start; triggers fade/stop when elapsed.
    length: Option<f32>,
    play_start: Instant,
    /// In-progress volume adjustment from an Adjust cue targeting this stream.
    volume_adjust: Option<VolumeAdjust>,
}

/// Multi-track audio playback engine. Maintains a list of concurrently running streams.
pub struct AudioPlaybackEngine {
    streams: Vec<ActiveAudioStream>,
    /// Cross-triggers queued at start() time, drained each frame by app.rs.
    pending_lighting_triggers: Vec<u32>,
}

impl AudioPlaybackEngine {
    pub fn new() -> Self {
        Self { streams: Vec::new(), pending_lighting_triggers: Vec::new() }
    }

    /// Start a new audio stream for this cue. Does NOT stop any currently playing streams.
    pub fn start(&mut self, cue: &Cue, player: &AudioPlayer) -> bool {
        let Some(data) = cue.audio_data() else { return false };

        let resolved = resolve_audio_path(&data.audio_path);
        let file = match File::open(&resolved) {
            Ok(f) => f,
            Err(e) => {
                log::error!("Audio: cannot open {}: {}", resolved.display(), e);
                return false;
            }
        };
        let source = match Decoder::new(BufReader::new(file)) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Audio: decode error: {}", e);
                return false;
            }
        };
        let sink = match player.new_sink() {
            Ok(s) => s,
            Err(e) => {
                log::error!("Audio: cannot create sink: {}", e);
                return false;
            }
        };

        let (initial_volume, initial_state, fade_start) = if data.fade_in > 0.0 {
            (0.0_f32, AudioCueState::FadingIn { progress: 0.0 }, Some(Instant::now()))
        } else {
            (data.volume, AudioCueState::Playing, None)
        };

        sink.set_volume(initial_volume);
        sink.append(source);

        if let Some(trigger) = data.triggers_lighting_cue {
            self.pending_lighting_triggers.push(trigger);
        }

        self.streams.push(ActiveAudioStream {
            cue_id: cue.id,
            sink,
            state: initial_state,
            base_volume: data.volume,
            fade_in_duration: data.fade_in,
            fade_out_duration: data.fade_out,
            fade_start,
            length: data.length,
            play_start: Instant::now(),
            volume_adjust: None,
        });

        log::info!(
            "Audio start: cue {:.2} '{}' vol={:.0}% fade_in={:.1}s length={:?}",
            cue.number, cue.label, data.volume * 100.0, data.fade_in, data.length
        );
        true
    }

    /// Apply a volume ramp to the stream for a specific cue. Only takes effect while Playing.
    /// If no stream for `cue_id` is active, this is a no-op.
    pub fn adjust_stream(&mut self, cue_id: u32, target_vol: f32, fade_time: f32, stop_when_complete: bool) {
        if let Some(stream) = self.streams.iter_mut().find(|s| s.cue_id == cue_id) {
            if fade_time <= 0.0 {
                stream.base_volume = target_vol;
                if stop_when_complete {
                    stream.sink.stop();
                }
            } else {
                stream.volume_adjust = Some(VolumeAdjust {
                    start_vol: stream.base_volume,
                    target_vol,
                    fade_time,
                    start: Instant::now(),
                    stop_when_complete,
                });
            }
        }
    }

    /// Stop all active streams immediately.
    pub fn stop_all(&mut self) {
        for s in self.streams.drain(..) {
            s.sink.stop();
        }
        self.pending_lighting_triggers.clear();
        log::debug!("Audio: all streams stopped");
    }

    /// Advance every stream's fade state and apply sound_master each frame.
    pub fn update(&mut self, sound_master: f32) {
        self.streams.retain_mut(|stream| {
            // File ended naturally
            if stream.sink.empty() {
                log::debug!("Audio cue {} finished (file end)", stream.cue_id);
                return false;
            }

            // Length timer: once Playing, check if we've hit the length limit
            if matches!(stream.state, AudioCueState::Playing) {
                if let Some(len) = stream.length {
                    if stream.play_start.elapsed().as_secs_f32() >= len {
                        if stream.fade_out_duration > 0.0 {
                            stream.state = AudioCueState::FadingOut { progress: 0.0 };
                            stream.fade_start = Some(Instant::now());
                            log::debug!("Audio cue {} length expired, fading out", stream.cue_id);
                        } else {
                            stream.sink.stop();
                            log::debug!("Audio cue {} length expired, stopping", stream.cue_id);
                            return false;
                        }
                    }
                }
            }

            // Volume adjust from an Adjust cue — only runs while Playing
            if matches!(stream.state, AudioCueState::Playing) {
                if let Some(adj) = stream.volume_adjust.take() {
                    let progress = if adj.fade_time > 0.0 {
                        (adj.start.elapsed().as_secs_f32() / adj.fade_time).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };
                    stream.base_volume = adj.start_vol + (adj.target_vol - adj.start_vol) * progress;
                    if progress < 1.0 {
                        stream.volume_adjust = Some(adj); // still running
                    } else if adj.stop_when_complete {
                        stream.sink.stop();
                        return false;
                    }
                    // otherwise adj is done, volume_adjust stays None
                }
            }

            // Compute per-stream volume based on fade state
            let volume = match stream.state {
                AudioCueState::FadingIn { .. } => {
                    let start = stream.fade_start.get_or_insert_with(Instant::now);
                    let progress =
                        (start.elapsed().as_secs_f32() / stream.fade_in_duration).clamp(0.0, 1.0);
                    if progress >= 1.0 {
                        stream.state = AudioCueState::Playing;
                        stream.fade_start = None;
                    } else {
                        stream.state = AudioCueState::FadingIn { progress };
                    }
                    stream.base_volume * progress
                }
                AudioCueState::Playing => stream.base_volume,
                AudioCueState::FadingOut { .. } => {
                    let start = stream.fade_start.get_or_insert_with(Instant::now);
                    let progress =
                        (start.elapsed().as_secs_f32() / stream.fade_out_duration).clamp(0.0, 1.0);
                    if progress >= 1.0 {
                        stream.sink.stop();
                        log::debug!("Audio cue {} fade out complete", stream.cue_id);
                        return false;
                    }
                    stream.state = AudioCueState::FadingOut { progress };
                    stream.base_volume * (1.0 - progress)
                }
                AudioCueState::Stopped => return false,
            };

            stream.sink.set_volume((volume * sound_master).clamp(0.0, 2.0));
            true
        });
    }

    // ── Query API ────────────────────────────────────────────────────────────

    /// IDs of all currently active streams (for row coloring).
    pub fn active_cue_ids(&self) -> Vec<u32> {
        self.streams.iter().map(|s| s.cue_id).collect()
    }

    /// Playback state for a specific cue, or None if that cue is not active.
    pub fn stream_state(&self, cue_id: u32) -> Option<AudioCueState> {
        self.streams.iter().find(|s| s.cue_id == cue_id).map(|s| s.state)
    }

    /// The ID of the most recently started active stream (used for footer display).
    pub fn current_cue_id(&self) -> Option<u32> {
        self.streams.last().map(|s| s.cue_id)
    }

    /// State of the most recently started active stream, or Stopped.
    pub fn state(&self) -> AudioCueState {
        self.streams.last().map(|s| s.state).unwrap_or(AudioCueState::Stopped)
    }

    /// Number of streams currently active.
    pub fn active_count(&self) -> usize {
        self.streams.len()
    }

    pub fn is_playing(&self) -> bool {
        !self.streams.is_empty()
    }

    /// Returns the 0–1 progress of the active volume-adjust fade for a stream, or None if no
    /// fade is running on that stream (or the stream doesn't exist).
    pub fn volume_adjust_progress(&self, cue_id: u32) -> Option<f32> {
        self.streams.iter()
            .find(|s| s.cue_id == cue_id)
            .and_then(|s| s.volume_adjust.as_ref())
            .map(|adj| {
                if adj.fade_time > 0.0 {
                    (adj.start.elapsed().as_secs_f32() / adj.fade_time).clamp(0.0, 1.0)
                } else {
                    1.0
                }
            })
    }

    /// Drain audio→lighting cross-triggers queued since last call.
    pub fn take_pending_lighting_triggers(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.pending_lighting_triggers)
    }
}

impl Default for AudioPlaybackEngine {
    fn default() -> Self { Self::new() }
}
