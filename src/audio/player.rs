//! Audio device ownership — holds the output stream alive and vends new Sinks.

use anyhow::{Context, Result};
use rodio::{OutputStream, OutputStreamHandle, Sink};

/// Owns the audio output stream and creates per-cue Sinks.
/// All playback state lives in AudioPlaybackEngine; this is just the device handle.
pub struct AudioPlayer {
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
}

impl AudioPlayer {
    pub fn new() -> Result<Self> {
        let (stream, stream_handle) = OutputStream::try_default()
            .context("Failed to open default audio output device")?;
        log::info!("Audio player initialized with default output device");
        Ok(Self { _stream: stream, stream_handle })
    }

    /// Create a new independent Sink tied to this device.
    pub fn new_sink(&self) -> Result<Sink> {
        Sink::try_new(&self.stream_handle).context("Failed to create audio sink")
    }
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            log::error!("Failed to create audio player: {}", e);
            panic!("Could not initialize audio player");
        })
    }
}
