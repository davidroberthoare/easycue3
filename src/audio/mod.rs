//! Audio playback system
//!
//! Provides audio cue management and playback using rodio.
//! Parallel architecture to the lighting cue system.

#[cfg(feature = "audio")]
pub mod types;
#[cfg(feature = "audio")]
pub mod list;
#[cfg(feature = "audio")]
pub mod player;
#[cfg(feature = "audio")]
pub mod playback;

#[cfg(feature = "audio")]
pub use types::{AudioCue, AudioCueState};
#[cfg(feature = "audio")]
pub use list::AudioCueList;
#[cfg(feature = "audio")]
pub use player::AudioPlayer;
#[cfg(feature = "audio")]
pub use playback::AudioPlaybackEngine;

// Re-export for non-audio builds (provides empty stubs)
#[cfg(not(feature = "audio"))]
pub mod stub {
    //! Stub implementations when audio feature is disabled
    
    /// Stub AudioCueList for non-audio builds
    #[derive(Debug, Clone, Default)]
    pub struct AudioCueList;
    
    impl AudioCueList {
        pub fn new() -> Self {
            Self
        }
    }
    
    /// Stub AudioPlayer for non-audio builds (no Debug derive - doesn't need it)
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
    }
}

#[cfg(not(feature = "audio"))]
pub use stub::*;
