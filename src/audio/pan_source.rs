//! Dynamic stereo pan source wrapper for rodio.
//!
//! `PanSource` wraps any `Source<Item = f32>` and applies a constant-power
//! stereo pan that can be updated in real-time via an `Arc<AtomicU32>` handle.
//! The main thread writes new pan values through the handle; the audio thread
//! reads them lock-free on each stereo frame.

use rodio::Source;
use std::num::NonZero;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use std::time::Duration;

/// Stereo pan source.  Wraps a `Source<Item = f32>` and scales left/right
/// channel amplitudes according to a constant-power pan law.
///
/// Mono sources (1 channel) pass through unchanged — pan has no effect.
pub struct PanSource<S: Source<Item = f32>> {
    inner: S,
    pan_ctrl: Arc<AtomicU32>,
    channel: usize,
    current_pan: f32,
    num_channels: usize,
}

impl<S: Source<Item = f32>> PanSource<S> {
    /// Wrap `inner` with the given `initial_pan` (-1.0 = full left, 0.0 = centre,
    /// 1.0 = full right).  Returns the source and a shared control handle.
    /// Write new f32 bits to the handle to update pan while playing.
    pub fn new(inner: S, initial_pan: f32) -> (Self, Arc<AtomicU32>) {
        let ctrl = Arc::new(AtomicU32::new(initial_pan.to_bits()));
        let num_channels = usize::from(inner.channels().get()).max(1);
        let s = Self {
            inner,
            pan_ctrl: Arc::clone(&ctrl),
            channel: 0,
            current_pan: initial_pan,
            num_channels,
        };
        (s, ctrl)
    }
}

/// Constant-power pan law.  Returns (left_gain, right_gain) for `pan` ∈ [-1, 1].
fn pan_gains(pan: f32) -> (f32, f32) {
    let angle = (pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
    (angle.cos(), angle.sin())
}

impl<S: Source<Item = f32>> Iterator for PanSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        // Refresh pan at the start of each frame (left/channel-0 sample).
        if self.channel == 0 {
            self.current_pan = f32::from_bits(self.pan_ctrl.load(Ordering::Relaxed));
        }

        let sample = self.inner.next()?;

        let out = if self.num_channels >= 2 {
            let (l_gain, r_gain) = pan_gains(self.current_pan);
            if self.channel % 2 == 0 {
                sample * l_gain
            } else {
                sample * r_gain
            }
        } else {
            sample
        };

        self.channel = (self.channel + 1) % self.num_channels;
        Some(out)
    }
}

impl<S: Source<Item = f32>> Source for PanSource<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }
    fn channels(&self) -> NonZero<u16> {
        self.inner.channels()
    }
    fn sample_rate(&self) -> NonZero<u32> {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}
