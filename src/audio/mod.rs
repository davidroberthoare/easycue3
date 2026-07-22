//! Audio playback system
//!
//! Provides audio cue management and playback using rodio.
//! Audio cues live in the unified CueList (as CueKind::Audio variants);
//! this module provides only the playback engine and player.

#[cfg(feature = "audio")]
pub mod playback;
#[cfg(feature = "audio")]
pub mod player;
#[cfg(feature = "audio")]
pub mod route_source;
#[cfg(feature = "audio")]
pub mod types;

#[cfg(feature = "audio")]
pub use playback::AudioPlaybackEngine;
#[cfg(feature = "audio")]
pub use player::{AudioPlayer, OutputChoice};
#[cfg(feature = "audio")]
pub use types::{AudioCue, AudioCueState};

// Stub implementations when the audio feature is disabled
#[cfg(not(feature = "audio"))]
pub mod stub {
    /// Stub AudioPlayer for non-audio builds
    pub struct AudioPlayer;
    impl AudioPlayer {
        pub fn new() -> anyhow::Result<Self> {
            Ok(Self)
        }
    }

    /// Stub AudioPlaybackEngine for non-audio builds
    #[derive(Debug)]
    pub struct AudioPlaybackEngine;
    impl AudioPlaybackEngine {
        pub fn new() -> Self {
            Self
        }
        pub fn is_playing(&self) -> bool {
            false
        }
        pub fn stop_all(&mut self) {}
    }
}

#[cfg(not(feature = "audio"))]
pub use stub::*;
