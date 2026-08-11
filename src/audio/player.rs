//! Audio device ownership — holds output streams alive and vends new Players.
//!
//! `AudioPlayer` enumerates all available output devices at startup and keeps
//! a `MixerDeviceSink` open for each one.  Multi-channel devices are opened at
//! their full channel width (capped at `MAX_OUTPUT_CHANNELS`) so audio cues
//! can target any stereo pair of the device, not just the first one.  Audio
//! cues can route to any combination of outputs simultaneously at independent
//! volume levels.

use crate::cue::AudioOutputRoute;
use anyhow::{Context, Result};
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player};
use std::num::NonZero;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// One physical (or virtual) audio output device held open.
pub struct NamedOutput {
    pub name: String,
    /// Channel count the stream was actually opened with.
    pub channels: NonZero<u16>,
    /// Set by the stream's error callback when the device is lost (e.g. after
    /// system sleep the ALSA stream dies with POLLERR). Cleared when the
    /// device is successfully re-opened.
    failed: Arc<AtomicBool>,
    /// Consecutive failed recovery attempts — drives exponential backoff and
    /// quiets the log after the first couple.
    recovery_failures: u32,
    /// Don't attempt recovery for this output before this instant.
    next_recovery_attempt: Option<std::time::Instant>,
    _sink: MixerDeviceSink,
}

/// One selectable destination: a whole stereo device, or one stereo pair of a
/// multi-channel device.  What the UI's output dropdowns list.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputChoice {
    pub device_name: String,
    /// First channel (0-based) of the pair; 0 for plain stereo devices.
    pub channel_offset: u16,
    /// Display label, e.g. "Rubix24 · Out 3-4" (or just the name for stereo).
    pub label: String,
}

/// Everything `AudioPlaybackEngine::start` needs to build one route's sink.
pub struct RoutePlayer {
    pub device_name: String,
    pub player: Player,
    pub volume: f32,
    pub pan: f32,
    pub channel_offset: u16,
    pub device_channels: NonZero<u16>,
}

/// Owns all audio output streams and creates per-cue Players.
pub struct AudioPlayer {
    /// [0] is always the default device; additional devices follow.
    outputs: Vec<NamedOutput>,
}

/// Cap on how many channels a device stream is opened with.  Some backends
/// (notably ALSA plugin devices) advertise absurd channel ranges; anything
/// beyond 8 (four stereo pairs) is outside this app's scope.
const MAX_OUTPUT_CHANNELS: u16 = 8;

impl AudioPlayer {
    /// Open the default output only.  Call `open_all_outputs()` after
    /// construction if you want access to secondary devices.
    pub fn new() -> Result<Self> {
        let host = rodio::cpal::default_host();
        let name = host
            .default_output_device()
            .and_then(|d| d.description().ok())
            .map(|desc| desc.name().to_string())
            .unwrap_or_else(|| "Default".to_string());

        let (device_sink, channels, failed) = Self::open_default_sink(&name)?;

        log::info!("Audio player: default device = '{}' ({}ch)", name, channels);
        Ok(Self {
            outputs: vec![NamedOutput {
                name,
                channels,
                failed,
                recovery_failures: 0,
                next_recovery_attempt: None,
                _sink: device_sink,
            }],
        })
    }

    /// ALSA meta-plugins that never represent a distinct physical (or
    /// user-named) output — they either discard audio, resample/remix for
    /// another plugin, or transparently re-target whatever the system
    /// default happens to be. Matched against the raw ALSA PCM id (not the
    /// human-readable description, which some backends override with
    /// confusing text like "Default ALSA Output (currently PipeWire ...)").
    const NON_DEVICE_PLUGIN_IDS: &'static [&'static str] = &[
        "null",
        "default",
        "pipewire",
        "pulse",
        "jack",
        "oss",
        "lavrate",
        "samplerate",
        "speexrate",
        "speex",
        "upmix",
        "vdownmix",
    ];

    /// Channel count to open the device with, clamped to what the app supports.
    ///
    /// Uses the *default* output config, not the supported-config maximum:
    /// ALSA plugin devices (PipeWire/pulse aliases) claim to support 1–32
    /// channels no matter what the real sink looks like, so the maximum would
    /// invent phantom pairs on plain stereo devices.  The default config
    /// reports the device's native width on Windows/macOS/raw ALSA, and on
    /// PipeWire aliases it reports whatever `channels N` the user pinned in
    /// `~/.asoundrc` (stereo if unpinned) — see docs/AUDIO_DEVICES.md.
    fn preferred_channels(device: &rodio::cpal::Device) -> u16 {
        device
            .default_output_config()
            .map(|c| c.channels())
            .unwrap_or(2)
            .clamp(1, MAX_OUTPUT_CHANNELS)
    }

    /// Build a stream error callback for one output device.  Sets `failed`
    /// (so the recovery loop can reopen the device) and logs through `log::`
    /// instead of rodio's default stderr print.  Rate-limited: cpal's ALSA
    /// worker keeps calling the callback on every poll while a stream is
    /// broken, so without throttling one failure would flood the log.
    fn stream_error_callback(
        name: &str,
        failed: Arc<AtomicBool>,
    ) -> impl FnMut(rodio::cpal::StreamError) + Send + Clone + 'static {
        let name = name.to_string();
        let last_logged = Arc::new(AtomicU64::new(0));
        move |err: rodio::cpal::StreamError| {
            failed.store(true, Ordering::Relaxed);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let last = last_logged.load(Ordering::Relaxed);
            if now.saturating_sub(last) >= 30 {
                last_logged.store(now, Ordering::Relaxed);
                log::error!("Audio stream error on '{}': {}", name, err);
            }
        }
    }

    /// Open the default output device as a sink, with our stream-error
    /// callback and drop-logging disabled.  Separate from `open_device`
    /// because the default device (e.g. a PipeWire "Default Audio Device"
    /// alias) is frequently *not* enumerable via `output_devices()` — it can
    /// only be reached through `from_default_device()`.
    fn open_default_sink(name: &str) -> Result<(MixerDeviceSink, NonZero<u16>, Arc<AtomicBool>)> {
        let failed = Arc::new(AtomicBool::new(false));
        // Build the sink manually so we can install our own stream-error
        // callback (rodio's default prints to stderr; we route through `log::`
        // and flag the output for recovery) and disable the "Dropping
        // DeviceSink" drop spam.
        let mut sink = DeviceSinkBuilder::from_default_device()
            .context("Failed to open default audio output device")?
            .with_error_callback(Self::stream_error_callback(name, Arc::clone(&failed)))
            .open_sink_or_fallback()
            .context("Failed to open default audio output device")?;
        sink.log_on_drop(false);
        let channels = sink.config().channel_count();
        Ok((sink, channels, failed))
    }

    /// Open a stream on `device` at its preferred channel count, falling back
    /// to whatever configuration the backend accepts.  Returns the sink, the
    /// channel count it actually opened with, and the failure flag its error
    /// callback writes to.
    fn open_device(
        device: rodio::cpal::Device,
        name: &str,
    ) -> Result<(MixerDeviceSink, NonZero<u16>, Arc<AtomicBool>)> {
        let want = Self::preferred_channels(&device);
        let failed = Arc::new(AtomicBool::new(false));
        let builder = DeviceSinkBuilder::from_device(device)?
            .with_error_callback(Self::stream_error_callback(name, Arc::clone(&failed)));
        let builder = if want > 2 {
            if let Some(nz) = NonZero::new(want) {
                builder.with_channels(nz)
            } else {
                builder
            }
        } else {
            builder
        };
        let mut sink = builder.open_sink_or_fallback()?;
        sink.log_on_drop(false);
        let channels = sink.config().channel_count();
        Ok((sink, channels, failed))
    }

    /// Enumerate all output devices and open a stream for each one that isn't
    /// already open.  If a device that's already open (e.g. the default) turns
    /// out to support more channels than it was opened with, it's re-opened at
    /// the wider width so its extra pairs become routable.  Silently skips
    /// devices that fail to open.
    pub fn open_all_outputs(&mut self) {
        let host = rodio::cpal::default_host();
        // Names already attempted (case-insensitive) — the same card can be
        // enumerated repeatedly under different underlying PCMs.
        let mut attempted: std::collections::HashSet<String> = std::collections::HashSet::new();

        let devices = match host.output_devices() {
            Ok(d) => d,
            Err(e) => {
                log::warn!("Audio: could not enumerate devices: {}", e);
                return;
            }
        };

        for device in devices {
            let id = device.id().map(|d| d.1).unwrap_or_default();
            if Self::NON_DEVICE_PLUGIN_IDS.contains(&id.as_str()) {
                continue;
            }
            let name = match device.description() {
                Ok(desc) => desc.name().to_string(),
                Err(_) => continue,
            };
            let lower = name.to_ascii_lowercase();

            if let Some(i) = self
                .outputs
                .iter()
                .position(|o| o.name.to_ascii_lowercase() == lower)
            {
                // Already open — upgrade in place if this enumeration shows the
                // device is wider than the stream we're holding (the default
                // device is initially opened at its default config).
                let have = self.outputs[i].channels.get();
                if Self::preferred_channels(&device) > have {
                    match Self::open_device(device, &name) {
                        Ok((sink, channels, failed)) if channels.get() > have => {
                            log::info!(
                                "Audio: re-opened '{}' at {}ch (was {}ch)",
                                name,
                                channels,
                                have
                            );
                            self.outputs[i]._sink = sink;
                            self.outputs[i].channels = channels;
                            self.outputs[i].failed = failed;
                        }
                        Ok(_) => {}
                        Err(e) => log::warn!("Audio: couldn't widen '{}': {}", name, e),
                    }
                }
                continue;
            }
            if !attempted.insert(lower) {
                continue; // already failed once under another PCM alias
            }
            match Self::open_device(device, &name) {
                Ok((sink, channels, failed)) => {
                    log::info!("Audio: opened secondary device '{}' ({}ch)", name, channels);
                    self.outputs.push(NamedOutput {
                        name,
                        channels,
                        failed,
                        recovery_failures: 0,
                        next_recovery_attempt: None,
                        _sink: sink,
                    });
                }
                Err(e) => log::warn!("Audio: skipping '{}': {}", name, e),
            }
        }
    }

    /// Names of all currently open output devices.
    pub fn device_names(&self) -> Vec<String> {
        self.outputs.iter().map(|o| o.name.clone()).collect()
    }

    /// Whether any output's stream has reported an error since it was opened
    /// or last recovered.  Drives the main-loop repaint so recovery runs even
    /// when the app would otherwise be idle.
    pub fn any_output_failed(&self) -> bool {
        self.outputs
            .iter()
            .any(|o| o.failed.load(Ordering::Relaxed))
    }

    /// Re-open every output whose stream failed (e.g. after system sleep).
    /// Per-output exponential backoff so a device that's genuinely gone (or
    /// whose ALSA node isn't back yet) doesn't trigger a re-enumeration every
    /// frame, and so the log doesn't spam after the first couple of failures.
    /// Returns the names of outputs successfully re-opened this call.
    pub fn recover_dead_outputs(&mut self) -> Vec<String> {
        let mut recovered = Vec::new();
        let host = rodio::cpal::default_host();

        for (idx, out) in self.outputs.iter_mut().enumerate() {
            if !out.failed.load(Ordering::Relaxed) {
                continue;
            }
            // Backoff: wait 2s after the first failure, 5s after the second,
            // then 15s, then 60s — a dead-but-enumerable device shouldn't be
            // hammered, and a truly-gone one should quiet right down.
            if let Some(next) = out.next_recovery_attempt {
                if std::time::Instant::now() < next {
                    continue;
                }
            }
            let fail = out.recovery_failures;

            let attempt: Result<(MixerDeviceSink, NonZero<u16>, Arc<AtomicBool>)> = if idx == 0 {
                // The default output is often a virtual alias (e.g. PipeWire's
                // "Default Audio Device") that is NOT enumerable through
                // `output_devices()` — reopen it through the default path.
                Self::open_default_sink(&out.name)
            } else {
                let devices = match host.output_devices() {
                    Ok(d) => d,
                    Err(e) => {
                        log::warn!("Audio: could not enumerate devices to recover: {}", e);
                        break;
                    }
                };
                // Match by name (case-insensitive), re-resolving a fresh
                // `cpal::Device` handle — the stored one is stale after sleep.
                let device = devices.into_iter().find(|d| {
                    d.description()
                        .map(|desc| desc.name().eq_ignore_ascii_case(&out.name))
                        .unwrap_or(false)
                });
                match device {
                    Some(device) => Self::open_device(device, &out.name),
                    None => {
                        out.recovery_failures += 1;
                        out.next_recovery_attempt =
                            Some(std::time::Instant::now() + Self::recovery_backoff(fail + 1));
                        if fail == 0 {
                            log::warn!(
                                "Audio: output '{}' not currently enumerable — will retry",
                                out.name
                            );
                        } else {
                            log::debug!(
                                "Audio: output '{}' still not enumerable — backing off",
                                out.name
                            );
                        }
                        continue;
                    }
                }
            };

            match attempt {
                Ok((sink, channels, failed)) => {
                    log::info!("Audio: recovered output '{}' ({}ch)", out.name, channels);
                    out._sink = sink;
                    out.channels = channels;
                    out.failed = failed;
                    out.recovery_failures = 0;
                    out.next_recovery_attempt = None;
                    recovered.push(out.name.clone());
                }
                Err(e) => {
                    out.recovery_failures += 1;
                    out.next_recovery_attempt =
                        Some(std::time::Instant::now() + Self::recovery_backoff(fail + 1));
                    if fail == 0 {
                        log::warn!(
                            "Audio: could not re-open '{}': {} — will retry",
                            out.name,
                            e
                        );
                    } else {
                        log::debug!(
                            "Audio: could not re-open '{}' (attempt {}): {} — backing off",
                            out.name,
                            fail + 1,
                            e
                        );
                    }
                }
            }
        }
        recovered
    }

    /// Exponential recovery backoff: 2s, 5s, 15s, 60s, then capped at 60s.
    fn recovery_backoff(failures: u32) -> Duration {
        const BACKOFFS: [u64; 4] = [2, 5, 15, 60];
        let secs = BACKOFFS
            .get(failures.saturating_sub(1) as usize)
            .copied()
            .unwrap_or(60);
        Duration::from_secs(secs)
    }

    /// All selectable outputs: one entry per stereo device, one per stereo
    /// pair of each multi-channel device.
    pub fn output_choices(&self) -> Vec<OutputChoice> {
        let mut choices = Vec::new();
        for o in &self.outputs {
            let ch = o.channels.get();
            if ch <= 2 {
                choices.push(OutputChoice {
                    device_name: o.name.clone(),
                    channel_offset: 0,
                    label: o.name.clone(),
                });
            } else {
                for pair in 0..ch / 2 {
                    let first = pair * 2;
                    choices.push(OutputChoice {
                        device_name: o.name.clone(),
                        channel_offset: first,
                        label: Self::pair_label(&o.name, first),
                    });
                }
            }
        }
        choices
    }

    /// Display label for a device + channel-offset pair, e.g. "Rubix24 · Out 3-4".
    pub fn pair_label(device_name: &str, channel_offset: u16) -> String {
        format!(
            "{} · Out {}-{}",
            device_name,
            channel_offset + 1,
            channel_offset + 2
        )
    }

    /// Name of the default (index 0) device.
    pub fn default_name(&self) -> &str {
        self.outputs
            .first()
            .map(|o| o.name.as_str())
            .unwrap_or("Default")
    }

    /// Whether a device with this name is currently open.
    pub fn has_output(&self, device_name: &str) -> bool {
        self.outputs.iter().any(|o| o.name == device_name)
    }

    /// Channel count of the named device's open stream (empty name = default).
    /// Falls back to stereo if the device isn't found.
    pub fn device_channels(&self, device_name: &str) -> NonZero<u16> {
        let output = if device_name.is_empty() {
            self.outputs.first()
        } else {
            self.outputs.iter().find(|o| o.name == device_name)
        };
        output
            .map(|o| o.channels)
            .unwrap_or_else(|| NonZero::new(2).unwrap())
    }

    /// Create a Player on the named device.  Falls back to the default device
    /// if `device_name` is empty or not found.
    pub fn new_player(&self, device_name: &str) -> Result<Player> {
        let output = if device_name.is_empty() {
            self.outputs.first()
        } else {
            self.outputs
                .iter()
                .find(|o| o.name == device_name)
                .or_else(|| self.outputs.first())
        };
        let sink = output.ok_or_else(|| anyhow::anyhow!("No audio output available"))?;
        Ok(Player::connect_new(sink._sink.mixer()))
    }

    /// Create players for all routes in `routes`.  If `routes` is empty, returns
    /// a single player on the default device at full volume, centre pan.
    pub fn new_players_for_routes(&self, routes: &[AudioOutputRoute]) -> Vec<RoutePlayer> {
        if routes.is_empty() {
            match self.new_player("") {
                Ok(player) => vec![RoutePlayer {
                    device_name: self.default_name().to_string(),
                    player,
                    volume: 1.0,
                    pan: 0.0,
                    channel_offset: 0,
                    device_channels: self.device_channels(""),
                }],
                Err(e) => {
                    log::error!("Audio: failed to create default player: {}", e);
                    vec![]
                }
            }
        } else {
            routes
                .iter()
                .filter_map(|route| match self.new_player(&route.device_name) {
                    Ok(player) => {
                        let device_name = if route.device_name.is_empty() {
                            self.default_name().to_string()
                        } else {
                            route.device_name.clone()
                        };
                        let device_channels = self.device_channels(&route.device_name);
                        if route.channel_offset > 0
                            && route.channel_offset + 2 > device_channels.get()
                        {
                            log::warn!(
                                "Audio: route '{}' targets channels {}-{} but the device \
                                     opened with {}ch — playing on its last pair instead",
                                device_name,
                                route.channel_offset + 1,
                                route.channel_offset + 2,
                                device_channels,
                            );
                        }
                        Some(RoutePlayer {
                            device_name,
                            player,
                            volume: route.volume,
                            pan: route.pan,
                            channel_offset: route.channel_offset,
                            device_channels,
                        })
                    }
                    Err(e) => {
                        log::warn!("Audio: route to '{}' failed: {}", route.device_name, e);
                        None
                    }
                })
                .collect()
        }
    }
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            log::error!("Failed to create AudioPlayer: {}", e);
            panic!("Could not initialise audio player: {}", e);
        })
    }
}
