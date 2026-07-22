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

/// One physical (or virtual) audio output device held open.
pub struct NamedOutput {
    pub name: String,
    /// Channel count the stream was actually opened with.
    pub channels: NonZero<u16>,
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
        let device_sink = DeviceSinkBuilder::open_default_sink()
            .context("Failed to open default audio output device")?;
        let channels = device_sink.config().channel_count();

        let name = {
            let host = rodio::cpal::default_host();
            host.default_output_device()
                .and_then(|d| d.description().ok())
                .map(|desc| desc.name().to_string())
                .unwrap_or_else(|| "Default".to_string())
        };

        log::info!("Audio player: default device = '{}' ({}ch)", name, channels);
        Ok(Self {
            outputs: vec![NamedOutput {
                name,
                channels,
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

    /// Open a stream on `device` at its preferred channel count, falling back
    /// to whatever configuration the backend accepts.  Returns the sink and
    /// the channel count it actually opened with.
    fn open_device(device: rodio::cpal::Device) -> Result<(MixerDeviceSink, NonZero<u16>)> {
        let want = Self::preferred_channels(&device);
        let mut builder = DeviceSinkBuilder::from_device(device)?;
        if want > 2 {
            if let Some(nz) = NonZero::new(want) {
                builder = builder.with_channels(nz);
            }
        }
        let sink = builder.open_sink_or_fallback()?;
        let channels = sink.config().channel_count();
        Ok((sink, channels))
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
                    match Self::open_device(device) {
                        Ok((sink, channels)) if channels.get() > have => {
                            log::info!(
                                "Audio: re-opened '{}' at {}ch (was {}ch)",
                                name,
                                channels,
                                have
                            );
                            self.outputs[i]._sink = sink;
                            self.outputs[i].channels = channels;
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
            match Self::open_device(device) {
                Ok((sink, channels)) => {
                    log::info!("Audio: opened secondary device '{}' ({}ch)", name, channels);
                    self.outputs.push(NamedOutput {
                        name,
                        channels,
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
