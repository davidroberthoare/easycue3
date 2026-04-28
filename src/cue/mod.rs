//! Cue system - data structures and playback engine
//!
//! Manages cue lists, playback, and recording.

pub mod types;
pub mod list;
pub mod playback;

pub use types::{Cue, CueState};
pub use list::CueList;
pub use playback::PlaybackEngine;
