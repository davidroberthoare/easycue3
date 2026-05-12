//! Audio device ownership — holds output streams alive and vends new Sinks.
//!
//! `AudioPlayer` enumerates all available output devices at startup and keeps
//! an `OutputStream` open for each one.  Audio cues can then route to any
//! combination of devices simultaneously at independent volume levels.

use anyhow::{Context, Result};
use crate::cue::AudioOutputRoute;
use rodio::{OutputStream, OutputStreamHandle, Sink};

/// One physical (or virtual) audio output device held open.
pub struct NamedOutput {
    pub name: String,
    _stream: OutputStream,
    handle: OutputStreamHandle,
}

/// Owns all audio output streams and creates per-cue Sinks.
pub struct AudioPlayer {
    /// [0] is always the default device; additional devices follow.
    outputs: Vec<NamedOutput>,
}

impl AudioPlayer {
    /// Open the default output only.  Call `open_all_outputs()` after
    /// construction if you want access to secondary devices.
    pub fn new() -> Result<Self> {
        let (stream, handle) = OutputStream::try_default()
            .context("Failed to open default audio output device")?;

        let name = {
            #[cfg(feature = "audio")]
            {
                use cpal::traits::{DeviceTrait, HostTrait};
                cpal::default_host()
                    .default_output_device()
                    .and_then(|d| d.name().ok())
                    .unwrap_or_else(|| "Default".to_string())
            }
            #[cfg(not(feature = "audio"))]
            "Default".to_string()
        };

        log::info!("Audio player: default device = '{}'", name);
        Ok(Self {
            outputs: vec![NamedOutput { name, _stream: stream, handle }],
        })
    }

    /// Enumerate all output devices and open a stream for each one that isn't
    /// already open.  Silently skips devices that fail to open.
    pub fn open_all_outputs(&mut self) {
        #[cfg(feature = "audio")]
        {
            use cpal::traits::{DeviceTrait, HostTrait};
            let host = cpal::default_host();
            let already: Vec<String> = self.outputs.iter().map(|o| o.name.clone()).collect();

            let devices = match host.output_devices() {
                Ok(d) => d,
                Err(e) => { log::warn!("Audio: could not enumerate devices: {}", e); return; }
            };

            for device in devices {
                let name = match device.name() {
                    Ok(n) => n,
                    Err(_) => continue,
                };
                // "default" is an ALSA virtual device that aliases the actual
                // default output; skip it to avoid a duplicate entry.
                if name.to_ascii_lowercase() == "default" {
                    continue;
                }
                if already.iter().any(|a| a.eq_ignore_ascii_case(&name)) {
                    continue;
                }
                match OutputStream::try_from_device(&device) {
                    Ok((stream, handle)) => {
                        log::info!("Audio: opened secondary device '{}'", name);
                        self.outputs.push(NamedOutput { name, _stream: stream, handle });
                    }
                    Err(e) => log::warn!("Audio: skipping '{}': {}", name, e),
                }
            }
        }
    }

    /// Names of all currently open output devices.
    pub fn device_names(&self) -> Vec<String> {
        self.outputs.iter().map(|o| o.name.clone()).collect()
    }

    /// Name of the default (index 0) device.
    pub fn default_name(&self) -> &str {
        self.outputs.first().map(|o| o.name.as_str()).unwrap_or("Default")
    }

    /// Create a Sink on the named device.  Falls back to the default device
    /// if `device_name` is empty or not found.
    pub fn new_sink(&self, device_name: &str) -> Result<Sink> {
        let output = if device_name.is_empty() {
            self.outputs.first()
        } else {
            self.outputs
                .iter()
                .find(|o| o.name == device_name)
                .or_else(|| self.outputs.first())
        };
        let handle = &output
            .ok_or_else(|| anyhow::anyhow!("No audio output available"))?
            .handle;
        Sink::try_new(handle).context("Failed to create audio sink")
    }

    /// Create sinks for all routes in `routes`.  If `routes` is empty, returns
    /// a single sink on the default device with volume 1.0 and pan 0.0.
    /// Each element is `(device_name, Sink, per_route_volume, pan)`.
    pub fn new_sinks_for_routes(
        &self,
        routes: &[AudioOutputRoute],
    ) -> Vec<(String, Sink, f32, f32)> {
        if routes.is_empty() {
            match self.new_sink("") {
                Ok(sink) => vec![(self.default_name().to_string(), sink, 1.0, 0.0)],
                Err(e) => {
                    log::error!("Audio: failed to create default sink: {}", e);
                    vec![]
                }
            }
        } else {
            routes
                .iter()
                .filter_map(|route| {
                    match self.new_sink(&route.device_name) {
                        Ok(sink) => {
                            let name = if route.device_name.is_empty() {
                                self.default_name().to_string()
                            } else {
                                route.device_name.clone()
                            };
                            Some((name, sink, route.volume, route.pan))
                        }
                        Err(e) => {
                            log::warn!(
                                "Audio: route to '{}' failed: {}",
                                route.device_name, e
                            );
                            None
                        }
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
