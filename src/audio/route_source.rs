//! Output-routing source wrapper for rodio.
//!
//! `RouteSource` adapts a decoded source to one stereo pair of an N-channel
//! output stream: the input is folded to stereo (mono is duplicated, channels
//! beyond the first two are dropped), a constant-power pan is applied, and the
//! result lands on output channels `offset`/`offset + 1` with silence
//! everywhere else.  With a stereo output and offset 0 this is a plain pan —
//! the common case for simple devices.
//!
//! The output channel count must match the count the device stream was opened
//! with: rodio's mixer silently *drops* extra channels when converting, so a
//! mismatched source would lose its signal instead of being remapped.
//!
//! Pan can be updated in real time via an `Arc<AtomicU32>` handle — the main
//! thread writes f32 bits, the audio thread reads them lock-free once per
//! output frame.

use rodio::Source;
use std::num::NonZero;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use std::time::Duration;

pub struct RouteSource<S: Source<Item = f32>> {
    inner: S,
    pan_ctrl: Arc<AtomicU32>,
    in_channels: usize,
    out_channels: NonZero<u16>,
    /// First output channel (0-based) of the target stereo pair.
    offset: usize,
    /// Panned (left, right) of the input frame currently being emitted.
    frame: (f32, f32),
    /// Next output-channel index to emit; `>= out_channels` means a fresh
    /// input frame must be pulled first.
    frame_pos: usize,
}

impl<S: Source<Item = f32>> RouteSource<S> {
    /// Wrap `inner`, targeting the pair starting at `channel_offset` of an
    /// `out_channels`-wide stream.  `initial_pan`: -1.0 = full left, 0.0 =
    /// centre, 1.0 = full right.  Returns the source and a shared pan handle.
    pub fn new(
        inner: S,
        initial_pan: f32,
        out_channels: NonZero<u16>,
        channel_offset: u16,
    ) -> (Self, Arc<AtomicU32>) {
        let ctrl = Arc::new(AtomicU32::new(initial_pan.to_bits()));
        let in_channels = usize::from(inner.channels().get()).max(1);
        // Keep the whole pair inside the stream, whatever the show file says —
        // the device may have fewer channels than when the cue was recorded.
        let offset = usize::from(channel_offset.min(out_channels.get().saturating_sub(2)));
        let out = usize::from(out_channels.get());
        let s = Self {
            inner,
            pan_ctrl: Arc::clone(&ctrl),
            in_channels,
            out_channels,
            offset,
            frame: (0.0, 0.0),
            frame_pos: out,
        };
        (s, ctrl)
    }
}

/// Constant-power pan law.  Returns (left_gain, right_gain) for `pan` ∈ [-1, 1].
fn pan_gains(pan: f32) -> (f32, f32) {
    let angle = (pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
    (angle.cos(), angle.sin())
}

impl<S: Source<Item = f32>> Iterator for RouteSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let out_channels = usize::from(self.out_channels.get());

        if self.frame_pos >= out_channels {
            // Pull one input frame, fold to stereo, and pan it.
            let pan = f32::from_bits(self.pan_ctrl.load(Ordering::Relaxed));
            let left = self.inner.next()?;
            let right = if self.in_channels >= 2 {
                self.inner.next().unwrap_or(0.0)
            } else {
                left
            };
            for _ in 2..self.in_channels {
                let _ = self.inner.next();
            }
            let (l_gain, r_gain) = pan_gains(pan);
            self.frame = (left * l_gain, right * r_gain);
            self.frame_pos = 0;
        }

        let pos = self.frame_pos;
        self.frame_pos += 1;

        let out = if out_channels == 1 {
            0.5 * (self.frame.0 + self.frame.1)
        } else if pos == self.offset {
            self.frame.0
        } else if pos == self.offset + 1 {
            self.frame.1
        } else {
            0.0
        };
        Some(out)
    }
}

impl<S: Source<Item = f32>> Source for RouteSource<S> {
    fn current_span_len(&self) -> Option<usize> {
        let out_channels = usize::from(self.out_channels.get());
        self.inner.current_span_len().map(|n| {
            let pending = out_channels.saturating_sub(self.frame_pos);
            (n / self.in_channels) * out_channels + pending
        })
    }
    fn channels(&self) -> NonZero<u16> {
        self.out_channels
    }
    fn sample_rate(&self) -> NonZero<u32> {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        self.inner.try_seek(pos)?;
        // Discard any half-emitted frame from the old position.
        self.frame_pos = usize::from(self.out_channels.get());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::buffer::SamplesBuffer;

    const CENTRE: f32 = std::f32::consts::FRAC_1_SQRT_2;

    fn nz16(n: u16) -> NonZero<u16> {
        NonZero::new(n).unwrap()
    }
    fn nz32(n: u32) -> NonZero<u32> {
        NonZero::new(n).unwrap()
    }

    fn collect(src: impl Source<Item = f32>) -> Vec<f32> {
        src.collect()
    }

    fn assert_close(got: &[f32], want: &[f32]) {
        assert_eq!(
            got.len(),
            want.len(),
            "length mismatch: {:?} vs {:?}",
            got,
            want
        );
        for (g, w) in got.iter().zip(want) {
            assert!((g - w).abs() < 1e-6, "expected {:?}, got {:?}", want, got);
        }
    }

    #[test]
    fn stereo_to_stereo_is_plain_pan() {
        let buf = SamplesBuffer::new(nz16(2), nz32(44100), vec![1.0, 1.0, 0.5, 0.25]);
        let (src, _) = RouteSource::new(buf, 0.0, nz16(2), 0);
        assert_close(
            &collect(src),
            &[CENTRE, CENTRE, 0.5 * CENTRE, 0.25 * CENTRE],
        );
    }

    #[test]
    fn stereo_lands_on_rear_pair_of_four_channel_stream() {
        let buf = SamplesBuffer::new(nz16(2), nz32(44100), vec![1.0, 0.5]);
        let (src, _) = RouteSource::new(buf, 0.0, nz16(4), 2);
        assert_close(&collect(src), &[0.0, 0.0, CENTRE, 0.5 * CENTRE]);
    }

    #[test]
    fn mono_is_duplicated_and_panned() {
        let buf = SamplesBuffer::new(nz16(1), nz32(44100), vec![1.0]);
        let (src, _) = RouteSource::new(buf, -1.0, nz16(2), 0);
        // Full left: left gain 1.0, right gain 0.0.
        assert_close(&collect(src), &[1.0, 0.0]);
    }

    #[test]
    fn excess_input_channels_are_dropped() {
        // One 4-channel input frame onto a stereo output: channels 3/4 discarded.
        let buf = SamplesBuffer::new(nz16(4), nz32(44100), vec![1.0, 1.0, 9.0, 9.0]);
        let (src, _) = RouteSource::new(buf, 0.0, nz16(2), 0);
        assert_close(&collect(src), &[CENTRE, CENTRE]);
    }

    #[test]
    fn offset_is_clamped_to_stream_width() {
        // Offset 6 on a 4-channel stream clamps to the last valid pair (2-3).
        let buf = SamplesBuffer::new(nz16(2), nz32(44100), vec![1.0, 1.0]);
        let (src, _) = RouteSource::new(buf, 0.0, nz16(4), 6);
        assert_close(&collect(src), &[0.0, 0.0, CENTRE, CENTRE]);
    }

    /// Minimal stereo source whose `current_span_len` reports *remaining*
    /// samples (`SamplesBuffer` reports a static length, which can't exercise
    /// the mid-frame arithmetic).
    struct SpanSource(std::vec::IntoIter<f32>);
    impl Iterator for SpanSource {
        type Item = f32;
        fn next(&mut self) -> Option<f32> {
            self.0.next()
        }
    }
    impl Source for SpanSource {
        fn current_span_len(&self) -> Option<usize> {
            Some(self.0.len())
        }
        fn channels(&self) -> NonZero<u16> {
            nz16(2)
        }
        fn sample_rate(&self) -> NonZero<u32> {
            nz32(44100)
        }
        fn total_duration(&self) -> Option<Duration> {
            None
        }
    }

    #[test]
    fn span_len_is_scaled_to_output_channels() {
        let (mut src, _) = RouteSource::new(SpanSource(vec![0.0; 8].into_iter()), 0.0, nz16(4), 0);
        // 4 stereo input frames -> 16 output samples.
        assert_eq!(src.current_span_len(), Some(16));
        let _ = src.next();
        // Mid-frame: 3 remaining input frames * 4 + 3 pending samples of this frame.
        assert_eq!(src.current_span_len(), Some(15));
    }

    #[test]
    fn live_pan_updates_apply_on_frame_boundaries() {
        let buf = SamplesBuffer::new(nz16(2), nz32(44100), vec![1.0, 1.0, 1.0, 1.0]);
        let (mut src, ctrl) = RouteSource::new(buf, -1.0, nz16(2), 0);
        let mut got = vec![src.next().unwrap(), src.next().unwrap()]; // full left
        ctrl.store(1.0_f32.to_bits(), Ordering::Relaxed); // swing full right
        got.push(src.next().unwrap());
        got.push(src.next().unwrap());
        assert_close(&got, &[1.0, 0.0, 0.0, 1.0]);
    }
}
