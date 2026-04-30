//! Audio playback engine using rodio

use anyhow::{Context, Result};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Resolve an audio file path, falling back to the media/ directory if needed
fn resolve_audio_path(path: &Path) -> PathBuf {
    // If path is absolute or exists as-is, use it
    if path.is_absolute() || path.exists() {
        return path.to_path_buf();
    }
    
    // Try prepending "media/" directory
    let media_path = PathBuf::from("media").join(path);
    if media_path.exists() {
        log::debug!("Resolved audio path: {} -> {}", path.display(), media_path.display());
        return media_path;
    }
    
    // Fall back to original path (will fail when opened, with proper error message)
    path.to_path_buf()
}

/// Audio player state (Note: Cannot derive Debug due to rodio types)
pub struct AudioPlayer {
    /// Audio output stream (must be kept alive)
    _stream: OutputStream,
    
    /// Handle to the output stream
    stream_handle: OutputStreamHandle,
    
    /// Current playback sink (None when not playing)
    sink: Arc<Mutex<Option<Sink>>>,
    
    /// Current audio duration (None if unknown)
    duration: Arc<Mutex<Option<Duration>>>,
    
    /// Target volume for current playback
    target_volume: Arc<Mutex<f32>>,
}

impl AudioPlayer {
    /// Create a new audio player with default output device
    pub fn new() -> Result<Self> {
        let (stream, stream_handle) = OutputStream::try_default()
            .context("Failed to open default audio output device")?;
        
        log::info!("Audio player initialized with default output device");
        
        Ok(Self {
            _stream: stream,
            stream_handle,
            sink: Arc::new(Mutex::new(None)),
            duration: Arc::new(Mutex::new(None)),
            target_volume: Arc::new(Mutex::new(1.0)),
        })
    }
    
    /// Play an audio file
    pub fn play(&mut self, path: &Path, volume: f32) -> Result<()> {
        // Stop any current playback
        self.stop();
        
        // Resolve the audio file path (fallback to media/ if needed)
        let resolved_path = resolve_audio_path(path);
        
        // Open and decode the audio file
        let file = File::open(&resolved_path)
            .context(format!("Failed to open audio file: {}", resolved_path.display()))?;
        let source = Decoder::new(BufReader::new(file))
            .context("Failed to decode audio file")?;
        
        // Try to get duration (may fail for some formats)
        let duration = source.total_duration();
        *self.duration.lock().unwrap() = duration;
        
        // Create a new sink
        let sink = Sink::try_new(&self.stream_handle)
            .context("Failed to create audio sink")?;
        
        // Set initial volume
        sink.set_volume(volume);
        *self.target_volume.lock().unwrap() = volume;
        
        // Add the source to the sink
        sink.append(source);
        
        // Store the sinkresolved_
        *self.sink.lock().unwrap() = Some(sink);
        
        log::info!("Playing audio file: {}", path.display());
        Ok(())
    }
    
    /// Pause playback
    pub fn pause(&self) {
        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            sink.pause();
            log::debug!("Audio paused");
        }
    }
    
    /// Resume playback
    pub fn resume(&self) {
        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            sink.play();
            log::debug!("Audio resumed");
        }
    }
    
    /// Stop playback
    pub fn stop(&mut self) {
        if let Some(sink) = self.sink.lock().unwrap().take() {
            sink.stop();
            log::debug!("Audio stopped");
        }
        *self.duration.lock().unwrap() = None;
    }
    
    /// Set playback volume (0.0 to 1.0)
    pub fn set_volume(&mut self, volume: f32) {
        let clamped_volume = volume.clamp(0.0, 1.0);
        *self.target_volume.lock().unwrap() = clamped_volume;
        
        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            sink.set_volume(clamped_volume);
        }
    }
    
    /// Get current volume
    pub fn volume(&self) -> f32 {
        *self.target_volume.lock().unwrap()
    }
    
    /// Check if audio is currently playing
    pub fn is_playing(&self) -> bool {
        self.sink.lock().unwrap()
            .as_ref()
            .map(|s| !s.is_paused() && !s.empty())
            .unwrap_or(false)
    }
    
    /// Check if audio is paused
    pub fn is_paused(&self) -> bool {
        self.sink.lock().unwrap()
            .as_ref()
            .map(|s| s.is_paused())
            .unwrap_or(false)
    }
    
    /// Check if playback has finished
    pub fn is_finished(&self) -> bool {
        self.sink.lock().unwrap()
            .as_ref()
            .map(|s| s.empty())
            .unwrap_or(true)
    }
    
    /// Get the total duration of the current audio (if available)
    pub fn duration(&self) -> Option<Duration> {
        *self.duration.lock().unwrap()
    }
    
    /// Format duration as MM:SS
    pub fn format_duration(duration: Duration) -> String {
        let total_secs = duration.as_secs();
        let minutes = total_secs / 60;
        let seconds = total_secs % 60;
        format!("{:02}:{:02}", minutes, seconds)
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
