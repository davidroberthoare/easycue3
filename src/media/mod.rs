//! Media playback (audio, video, images)
//!
//! Integrates lumina-video for video, rodio for audio, and egui for images.

// Placeholder for media playback implementation
// Will integrate lumina-video and rodio in Phase 5

pub struct MediaManager {
    // TODO: Audio player state
    // TODO: Video player state
    // TODO: Image display state
}

impl MediaManager {
    pub fn new() -> Self {
        log::info!("Media manager initialized");
        Self {}
    }
}

impl Default for MediaManager {
    fn default() -> Self {
        Self::new()
    }
}
