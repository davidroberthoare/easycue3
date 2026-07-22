//! Multi-track, multi-output audio playback engine.
//!
//! Each audio cue gets its own set of Sinks — one per output route.  Firing a
//! new cue never stops existing ones; each stream runs independently.
//!
//! An Adjust cue can fade the volume and/or pan on individual output routes
//! (to move sound between speakers or sweep stereo position) or adjust the
//! overall cue or master volume as before.

use crate::audio::{pan_source::PanSource, AudioCueState, AudioPlayer};
use crate::cue::Cue;
use rodio::{Decoder, Player};
use std::fs::File;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// In-progress volume ramp on a single output-device route.
struct RouteAdjust {
    start_vol: f32,
    target_vol: f32,
    fade_time: f32,
    start: Instant,
    stop_when_complete: bool,
}

/// In-progress pan sweep on a single output-device route.
struct RoutePanAdjust {
    start_pan: f32,
    target_pan: f32,
    fade_time: f32,
    start: Instant,
}

/// One device sink with its own per-route volume/pan and optional in-progress fades.
struct DeviceSink {
    device_name: String,
    sink: Player,
    /// Current per-route volume scale (0.0–1.0).
    volume: f32,
    /// Ongoing route-level volume fade (from an Adjust cue).
    adjust: Option<RouteAdjust>,
    /// Current stereo pan (-1.0 = L, 0.0 = centre, 1.0 = R).
    pan: f32,
    /// Shared control that the PanSource reads from the audio thread.
    pan_ctrl: Arc<AtomicU32>,
    /// Ongoing pan sweep (from an Adjust cue).
    pan_adjust: Option<RoutePanAdjust>,
}

impl DeviceSink {
    fn write_pan(&self) {
        self.pan_ctrl.store(self.pan.to_bits(), Ordering::Relaxed);
    }
}

struct ActiveAudioStream {
    cue_id: u32,
    /// The source file, kept so an Adjust cue can join a device that wasn't
    /// one of the original output routes (see `join_device`).
    audio_path: std::path::PathBuf,
    /// One entry per output route (usually just the default device).
    device_sinks: Vec<DeviceSink>,
    state: AudioCueState,
    fade_in_duration: f32,
    fade_out_duration: f32,
    fade_start: Option<Instant>,
    length: Option<f32>,
    play_start: Instant,
}

/// Ease-in/ease-out curve: slow at both ends, full speed in the middle.
#[inline]
fn smoothstep(p: f32) -> f32 {
    p * p * (3.0 - 2.0 * p)
}

/// Multi-track audio playback engine.
pub struct AudioPlaybackEngine {
    streams: Vec<ActiveAudioStream>,
}

impl AudioPlaybackEngine {
    pub fn new() -> Self {
        Self { streams: Vec::new() }
    }

    /// Start a new audio stream for `cue`.  Does NOT stop existing streams.
    pub fn start(&mut self, cue: &Cue, player: &AudioPlayer) -> bool {
        let Some(data) = cue.audio_data() else { return false };

        let resolved = crate::paths::resolve_media_path(&data.audio_path);

        // Create one sink per output route (or default if routes is empty).
        let route_sinks = player.new_players_for_routes(&data.output_routes);
        if route_sinks.is_empty() {
            log::error!("Audio: no sinks created for cue {:.2}", cue.number);
            return false;
        }

        let (initial_volume, initial_state, fade_start) = if data.fade_in > 0.0 {
            (0.0_f32, AudioCueState::FadingIn { progress: 0.0 }, Some(Instant::now()))
        } else {
            (1.0_f32, AudioCueState::Playing, None)
        };

        let mut device_sinks = Vec::with_capacity(route_sinks.len());
        for (device_name, sink, route_vol, route_pan) in route_sinks {
            // Each device sink needs its own decoder — re-open the file.
            let file = match File::open(&resolved) {
                Ok(f) => f,
                Err(e) => {
                    log::error!("Audio: cannot open {}: {}", resolved.display(), e);
                    continue;
                }
            };
            let raw = match Decoder::try_from(file) {
                Ok(s) => s,
                Err(e) => {
                    log::error!("Audio: decode error on '{}': {}", device_name, e);
                    continue;
                }
            };
            let (panned, pan_ctrl) = PanSource::new(raw, route_pan);
            sink.set_volume(initial_volume * route_vol);
            sink.append(panned);
            device_sinks.push(DeviceSink {
                device_name,
                sink,
                volume: route_vol,
                adjust: None,
                pan: route_pan,
                pan_ctrl,
                pan_adjust: None,
            });
        }

        if device_sinks.is_empty() {
            return false;
        }

        self.streams.push(ActiveAudioStream {
            cue_id: cue.id,
            audio_path: resolved,
            device_sinks,
            state: initial_state,
            fade_in_duration: data.fade_in,
            fade_out_duration: data.fade_out,
            fade_start,
            length: data.length,
            play_start: Instant::now(),
        });

        log::info!(
            "Audio start: cue {:.2} '{}' routes={} fade_in={:.1}s",
            cue.number,
            cue.label,
            data.output_routes.len().max(1),
            data.fade_in,
        );
        true
    }

    /// Open a new device sink on `stream` for a device it wasn't originally routed to,
    /// starting from silence at the same playback position as the stream's other sinks —
    /// so an Adjust cue can fade audio *onto* a device the Play cue never routed to.
    /// Returns the new sink's index, or `None` if the device doesn't exist or fails to open.
    fn join_device(stream: &mut ActiveAudioStream, device_name: &str, player: &AudioPlayer) -> Option<usize> {
        if !player.device_names().iter().any(|n| n == device_name) {
            return None;
        }
        let sink = player.new_player(device_name).ok()?;
        let file = File::open(&stream.audio_path).ok()?;
        let raw = Decoder::try_from(file).ok()?;
        let elapsed = stream.play_start.elapsed();
        let (mut panned, pan_ctrl) = PanSource::new(raw, 0.0);
        if let Err(e) = rodio::Source::try_seek(&mut panned, elapsed) {
            log::warn!(
                "Adjust: joining '{}' couldn't seek to {:.1}s (starting from 0): {}",
                device_name, elapsed.as_secs_f32(), e
            );
        }
        sink.set_volume(0.0);
        sink.append(panned);
        stream.device_sinks.push(DeviceSink {
            device_name: device_name.to_string(),
            sink,
            volume: 0.0,
            adjust: None,
            pan: 0.0,
            pan_ctrl,
            pan_adjust: None,
        });
        log::info!("Adjust: joined device '{}' to cue {} at {:.1}s in", device_name, stream.cue_id, elapsed.as_secs_f32());
        Some(stream.device_sinks.len() - 1)
    }

    /// Fade the per-route volume and/or pan for a specific output device on a stream.
    /// `cue_id == 0` targets all active streams.  `device_name` empty = default device.
    /// `target_pan` of `None` leaves pan unchanged.  If the stream isn't already routed
    /// to `device_name`, it's joined in from silence so the fade can bring it in live.
    pub fn adjust_stream_output(
        &mut self,
        cue_id: u32,
        device_name: &str,
        target_vol: f32,
        target_pan: Option<f32>,
        fade_time: f32,
        stop_when_complete: bool,
        player: &AudioPlayer,
    ) {
        for stream in self.streams.iter_mut() {
            if cue_id != 0 && stream.cue_id != cue_id {
                continue;
            }
            let ds_idx = if device_name.is_empty() {
                Some(0)
            } else {
                stream.device_sinks.iter().position(|ds| ds.device_name == device_name)
                    .or_else(|| Self::join_device(stream, device_name, player))
            };
            if let Some(idx) = ds_idx {
                let ds = &mut stream.device_sinks[idx];

                // Volume
                if fade_time <= 0.0 {
                    ds.volume = target_vol;
                    ds.adjust = None;
                    if stop_when_complete {
                        ds.sink.stop();
                    }
                } else {
                    ds.adjust = Some(RouteAdjust {
                        start_vol: ds.volume,
                        target_vol,
                        fade_time,
                        start: Instant::now(),
                        stop_when_complete,
                    });
                }

                // Pan
                if let Some(tp) = target_pan {
                    if fade_time <= 0.0 {
                        ds.pan = tp;
                        ds.pan_adjust = None;
                        ds.write_pan();
                    } else {
                        ds.pan_adjust = Some(RoutePanAdjust {
                            start_pan: ds.pan,
                            target_pan: tp,
                            fade_time,
                            start: Instant::now(),
                        });
                    }
                }
            } else {
                log::warn!(
                    "Adjust: cue {} has no output device named '{}' — fade skipped",
                    stream.cue_id,
                    if device_name.is_empty() { "Default" } else { device_name },
                );
            }
        }
    }

    /// Stop all active streams immediately.
    pub fn stop_all(&mut self) {
        for s in self.streams.drain(..) {
            for ds in s.device_sinks { ds.sink.stop(); }
        }
        log::debug!("Audio: all streams stopped");
    }

    /// Begin fading out all playing or fading-in streams.
    /// Uses each stream's own `fade_out_duration`; falls back to `default_fade_secs`
    /// for streams that have no configured fade-out.
    pub fn stop_all_with_fade(&mut self, default_fade_secs: f32) {
        for stream in &mut self.streams {
            if matches!(
                stream.state,
                AudioCueState::Playing | AudioCueState::FadingIn { .. }
            ) {
                let fade = if stream.fade_out_duration > 0.0 {
                    stream.fade_out_duration
                } else {
                    default_fade_secs
                };
                stream.fade_out_duration = fade;
                stream.fade_start = Some(Instant::now());
                stream.state = AudioCueState::FadingOut { progress: 0.0 };
                log::debug!("Audio: cue {} fading out over {:.1}s", stream.cue_id, fade);
            }
        }
    }

    /// Advance fade state and apply sound_master each frame.
    pub fn update(&mut self, sound_master: f32) {
        self.streams.retain_mut(|stream| {
            // Drop stream if every sink has finished.
            if stream.device_sinks.iter().all(|ds| ds.sink.empty()) {
                log::debug!("Audio cue {} finished", stream.cue_id);
                return false;
            }

            // Length timer: once Playing, check elapsed.
            if matches!(stream.state, AudioCueState::Playing) {
                if let Some(len) = stream.length {
                    if stream.play_start.elapsed().as_secs_f32() >= len {
                        if stream.fade_out_duration > 0.0 {
                            stream.state = AudioCueState::FadingOut { progress: 0.0 };
                            stream.fade_start = Some(Instant::now());
                        } else {
                            for ds in &stream.device_sinks { ds.sink.stop(); }
                            return false;
                        }
                    }
                }
            }

            // Compute fade factor from state.
            let fade_factor = match stream.state {
                AudioCueState::FadingIn { .. } => {
                    let start = stream.fade_start.get_or_insert_with(Instant::now);
                    let p = (start.elapsed().as_secs_f32() / stream.fade_in_duration)
                        .clamp(0.0, 1.0);
                    if p >= 1.0 {
                        stream.state = AudioCueState::Playing;
                        stream.fade_start = None;
                    } else {
                        stream.state = AudioCueState::FadingIn { progress: p };
                    }
                    smoothstep(p)
                }
                AudioCueState::Playing => 1.0,
                AudioCueState::FadingOut { .. } => {
                    let start = stream.fade_start.get_or_insert_with(Instant::now);
                    let p = (start.elapsed().as_secs_f32() / stream.fade_out_duration)
                        .clamp(0.0, 1.0);
                    if p >= 1.0 {
                        for ds in &stream.device_sinks { ds.sink.stop(); }
                        return false;
                    }
                    stream.state = AudioCueState::FadingOut { progress: p };
                    smoothstep(1.0 - p)
                }
                AudioCueState::Stopped => return false,
            };

            let base = fade_factor;

            // Apply volume and pan to each device sink.
            for ds in stream.device_sinks.iter_mut() {
                // Advance per-route volume adjust.
                if let Some(adj) = ds.adjust.take() {
                    let progress = if adj.fade_time > 0.0 {
                        (adj.start.elapsed().as_secs_f32() / adj.fade_time).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };
                    ds.volume = adj.start_vol + (adj.target_vol - adj.start_vol) * progress;
                    if progress < 1.0 {
                        ds.adjust = Some(adj);
                    } else if adj.stop_when_complete {
                        ds.sink.stop();
                        continue;
                    }
                }

                // Advance per-route pan sweep.
                if let Some(adj) = ds.pan_adjust.take() {
                    let progress = if adj.fade_time > 0.0 {
                        (adj.start.elapsed().as_secs_f32() / adj.fade_time).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };
                    ds.pan = adj.start_pan + (adj.target_pan - adj.start_pan) * progress;
                    ds.write_pan();
                    if progress < 1.0 {
                        ds.pan_adjust = Some(adj);
                    }
                }

                ds.sink.set_volume((base * ds.volume * sound_master).clamp(0.0, 2.0));
            }

            // Remove stopped sinks.
            stream.device_sinks.retain(|ds| !ds.sink.empty());
            !stream.device_sinks.is_empty()
        });
    }

    // ── Query API ────────────────────────────────────────────────────────────

    pub fn active_cue_ids(&self) -> Vec<u32> {
        self.streams.iter().map(|s| s.cue_id).collect()
    }

    pub fn stream_state(&self, cue_id: u32) -> Option<AudioCueState> {
        self.streams.iter().find(|s| s.cue_id == cue_id).map(|s| s.state)
    }

    pub fn current_cue_id(&self) -> Option<u32> {
        self.streams.last().map(|s| s.cue_id)
    }

    pub fn state(&self) -> AudioCueState {
        self.streams.last().map(|s| s.state).unwrap_or(AudioCueState::Stopped)
    }

    pub fn active_count(&self) -> usize {
        self.streams.len()
    }

    pub fn is_playing(&self) -> bool {
        !self.streams.is_empty()
    }

    /// Progress (0–1) of any in-progress per-route volume fade on a stream, or None.
    pub fn volume_adjust_progress(&self, cue_id: u32) -> Option<f32> {
        self.streams
            .iter()
            .find(|s| s.cue_id == cue_id)
            .and_then(|s| {
                s.device_sinks.iter().find_map(|ds| {
                    ds.adjust.as_ref().map(|adj| {
                        if adj.fade_time > 0.0 {
                            (adj.start.elapsed().as_secs_f32() / adj.fade_time).clamp(0.0, 1.0)
                        } else {
                            1.0
                        }
                    })
                })
            })
    }

    /// Current per-route volume for a specific device on a stream (for UI display).
    pub fn route_volume(&self, cue_id: u32, device_name: &str) -> Option<f32> {
        self.streams
            .iter()
            .find(|s| s.cue_id == cue_id)
            .and_then(|s| {
                s.device_sinks
                    .iter()
                    .find(|ds| {
                        ds.device_name == device_name
                            || (device_name.is_empty()
                                && ds.device_name == s.device_sinks[0].device_name)
                    })
                    .map(|ds| ds.volume)
            })
    }

    /// Current per-route pan for a specific device on a stream (for UI display).
    pub fn route_pan(&self, cue_id: u32, device_name: &str) -> Option<f32> {
        self.streams
            .iter()
            .find(|s| s.cue_id == cue_id)
            .and_then(|s| {
                s.device_sinks
                    .iter()
                    .find(|ds| {
                        ds.device_name == device_name
                            || (device_name.is_empty()
                                && ds.device_name == s.device_sinks[0].device_name)
                    })
                    .map(|ds| ds.pan)
            })
    }
}

impl Default for AudioPlaybackEngine {
    fn default() -> Self { Self::new() }
}
