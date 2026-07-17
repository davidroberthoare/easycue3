//! Per-frame effect runtime.
//!
//! The engine holds the set of currently running effects and modulates a
//! *staged clone* of the universes each frame, just before masters are applied
//! and DMX is sent. The stored universes are never touched, so the base look
//! (cue tracking, recording, channel readouts) stays clean, and each frame is
//! recomputed from the untouched base — no cross-frame feedback.

use super::{sample, EffectFixture, EffectList, EffectTarget};
use crate::dmx::Universe;
use std::time::Instant;

/// Entry/exit envelope for a running effect. `from` is the scale the ramp
/// started at, so retargeting mid-ramp never snaps.
#[derive(Debug, Clone)]
enum Ramp {
    In { since: Instant, dur: f32, from: f32 },
    Full,
    Out { since: Instant, dur: f32, from: f32 },
}

impl Ramp {
    fn progress(since: &Instant, dur: f32) -> f32 {
        if dur <= 0.0 {
            1.0
        } else {
            (since.elapsed().as_secs_f32() / dur).min(1.0)
        }
    }

    /// Current scale 0–1 without advancing state.
    fn value(&self) -> f32 {
        match self {
            Ramp::In { since, dur, from } => from + (1.0 - from) * Self::progress(since, *dur),
            Ramp::Full => 1.0,
            Ramp::Out { since, dur, from } => from * (1.0 - Self::progress(since, *dur)),
        }
    }
}

/// One running effect instance.
#[derive(Debug, Clone)]
pub struct RunningEffect {
    effect_id: u32,
    /// The fixture IDs as requested (sorted) — compared during tracking sync.
    fixture_ids: Vec<usize>,
    /// Resolved channel data, same conceptual set as `fixture_ids` minus any
    /// that failed to resolve.
    fixtures: Vec<EffectFixture>,
    /// Effect clock origin; phase = elapsed × rate. Survives retargeting so
    /// fixture-set changes never cause a phase snap.
    start: Instant,
    ramp: Ramp,
}

impl RunningEffect {
    pub fn effect_id(&self) -> u32 {
        self.effect_id
    }

    pub fn fixture_ids(&self) -> &[usize] {
        &self.fixture_ids
    }

    /// True while ramping out toward removal.
    pub fn is_stopping(&self) -> bool {
        matches!(self.ramp, Ramp::Out { .. })
    }
}

/// Holds and applies all running effects. Owned by the app; `apply` is called
/// once per frame on the output-stage universe clone.
#[derive(Debug, Default)]
pub struct EffectEngine {
    running: Vec<RunningEffect>,
}

impl EffectEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start an effect, or retarget it if already running: the fixture set is
    /// replaced but the effect clock keeps running, and a ramp-out reverses
    /// into a ramp-in from its current level.
    pub fn start(
        &mut self,
        effect_id: u32,
        fixture_ids: Vec<usize>,
        fixtures: Vec<EffectFixture>,
        ramp_in: f32,
    ) {
        if let Some(inst) = self.running.iter_mut().find(|r| r.effect_id == effect_id) {
            inst.fixture_ids = fixture_ids;
            inst.fixtures = fixtures;
            if !matches!(inst.ramp, Ramp::Full) {
                let from = inst.ramp.value();
                inst.ramp = Ramp::In {
                    since: Instant::now(),
                    dur: ramp_in,
                    from,
                };
            }
        } else {
            self.running.push(RunningEffect {
                effect_id,
                fixture_ids,
                fixtures,
                start: Instant::now(),
                ramp: Ramp::In {
                    since: Instant::now(),
                    dur: ramp_in,
                    from: 0.0,
                },
            });
        }
    }

    /// Begin ramping an effect out; the instance is removed by `apply` once
    /// the ramp completes (immediately if `ramp_out` is 0).
    pub fn stop(&mut self, effect_id: u32, ramp_out: f32) {
        if let Some(inst) = self.running.iter_mut().find(|r| r.effect_id == effect_id) {
            if !inst.is_stopping() {
                let from = inst.ramp.value();
                inst.ramp = Ramp::Out {
                    since: Instant::now(),
                    dur: ramp_out,
                    from,
                };
            }
        }
    }

    pub fn stop_all(&mut self, ramp_out: f32) {
        let ids: Vec<u32> = self.running.iter().map(|r| r.effect_id).collect();
        for id in ids {
            self.stop(id, ramp_out);
        }
    }

    /// Instant removal of everything — new show / load show.
    pub fn clear(&mut self) {
        self.running.clear();
    }

    /// True while anything is running (including ramping out). Drives both the
    /// output-stage apply and the continuous-repaint request.
    pub fn is_active(&self) -> bool {
        !self.running.is_empty()
    }

    /// Running and not on its way out.
    pub fn is_running(&self, effect_id: u32) -> bool {
        self.running
            .iter()
            .any(|r| r.effect_id == effect_id && !r.is_stopping())
    }

    pub fn running(&self) -> &[RunningEffect] {
        &self.running
    }

    /// Modulate `universes` in place (a per-frame clone of the base look).
    /// Effect parameters are looked up live in `effects` so panel edits are
    /// audible immediately; instances whose effect was deleted, or whose
    /// ramp-out has finished, are removed.
    pub fn apply(&mut self, universes: &mut [Universe], effects: &EffectList) {
        self.running.retain_mut(|inst| {
            let Some(effect) = effects.find(inst.effect_id) else {
                log::warn!(
                    "Effect {} deleted from library while running — stopping it",
                    inst.effect_id
                );
                return false;
            };

            let scale = inst.ramp.value();
            match &inst.ramp {
                Ramp::In { since, dur, .. } if Ramp::progress(since, *dur) >= 1.0 => {
                    inst.ramp = Ramp::Full;
                }
                Ramp::Out { since, dur, .. } if Ramp::progress(since, *dur) >= 1.0 => {
                    return false;
                }
                _ => {}
            }

            let t = inst.start.elapsed().as_secs_f32();
            let n = inst.fixtures.len();
            for (i, fx) in inst.fixtures.iter().enumerate() {
                let Some(universe) = universes.get_mut(fx.universe_idx) else {
                    continue;
                };
                let delta = sample(effect, t, i, n, fx.fixture_id, false) * effect.size * scale;
                match effect.target {
                    EffectTarget::Intensity => {
                        if let Some(ch) = fx.intensity_ch {
                            add_delta(universe, ch, delta);
                        } else {
                            // RGB-only fixture: hue-preserving scale of the color
                            // engine, matching the VirtualIntensity model.
                            scale_colors(universe, &fx.color_chs, delta);
                        }
                    }
                    EffectTarget::Color => scale_colors(universe, &fx.color_chs, delta),
                    EffectTarget::Pan => {
                        if let Some(ch) = fx.pan_ch {
                            add_delta(universe, ch, delta);
                        }
                    }
                    EffectTarget::Tilt => {
                        if let Some(ch) = fx.tilt_ch {
                            add_delta(universe, ch, delta);
                        }
                    }
                    EffectTarget::Position => {
                        if let Some(ch) = fx.pan_ch {
                            add_delta(universe, ch, delta);
                        }
                        if let Some(ch) = fx.tilt_ch {
                            let tilt_delta =
                                sample(effect, t, i, n, fx.fixture_id, true) * effect.size * scale;
                            add_delta(universe, ch, tilt_delta);
                        }
                    }
                }
            }
            true
        });
    }
}

/// Offset one channel by `delta` around its base value, clamped to 0–100.
fn add_delta(universe: &mut Universe, channel: u16, delta: f32) {
    if let Ok(base) = universe.get_channel(channel) {
        let new = (base as f32 + delta).clamp(0.0, 100.0).round() as u8;
        let _ = universe.set_channel(channel, new);
    }
}

/// Scale all color channels so their maximum moves by `delta`, preserving the
/// ratios between them (hue). A fixture at base black stays black — an effect
/// cannot invent a color.
fn scale_colors(universe: &mut Universe, channels: &[u16], delta: f32) {
    let max = channels
        .iter()
        .filter_map(|&ch| universe.get_channel(ch).ok())
        .max()
        .unwrap_or(0);
    if max == 0 {
        return;
    }
    let factor = (max as f32 + delta).clamp(0.0, 100.0) / max as f32;
    for &ch in channels {
        if let Ok(v) = universe.get_channel(ch) {
            let new = (v as f32 * factor).round().clamp(0.0, 100.0) as u8;
            let _ = universe.set_channel(ch, new);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::{Effect, EffectList, Waveform};

    fn dimmer_fixture(ch: u16) -> EffectFixture {
        EffectFixture {
            fixture_id: 1,
            universe_idx: 0,
            intensity_ch: Some(ch),
            color_chs: Vec::new(),
            pan_ch: None,
            tilt_ch: None,
        }
    }

    fn rgb_fixture(start: u16) -> EffectFixture {
        EffectFixture {
            fixture_id: 2,
            universe_idx: 0,
            intensity_ch: None,
            color_chs: vec![start, start + 1, start + 2],
            pan_ch: None,
            tilt_ch: None,
        }
    }

    #[test]
    fn intensity_effect_modulates_around_base() {
        let mut effects = EffectList::new();
        let id = effects.add(Effect {
            size: 30.0,
            ..Effect::new()
        });
        let mut engine = EffectEngine::new();
        engine.start(id, vec![1], vec![dimmer_fixture(1)], 0.0);

        let mut universes = vec![Universe::new(1)];
        universes[0].set_channel(1, 50).unwrap();
        engine.apply(&mut universes, &effects);
        let v = universes[0].get_channel(1).unwrap();
        assert!((20..=80).contains(&v), "value {} outside base±size", v);
    }

    #[test]
    fn rgb_only_fixture_at_black_stays_black() {
        let mut effects = EffectList::new();
        let id = effects.add(Effect {
            size: 50.0,
            ..Effect::new()
        });
        let mut engine = EffectEngine::new();
        engine.start(id, vec![2], vec![rgb_fixture(10)], 0.0);

        let mut universes = vec![Universe::new(1)];
        engine.apply(&mut universes, &effects);
        for ch in 10..=12 {
            assert_eq!(universes[0].get_channel(ch).unwrap(), 0);
        }
    }

    #[test]
    fn color_scale_preserves_ratios() {
        let mut effects = EffectList::new();
        // Square wave sits at exactly +1 in the first half-cycle → factor is exact.
        let id = effects.add(Effect {
            size: 20.0,
            waveform: Waveform::Square,
            ..Effect::new()
        });
        let mut engine = EffectEngine::new();
        engine.start(id, vec![2], vec![rgb_fixture(10)], 0.0);

        let mut universes = vec![Universe::new(1)];
        universes[0].set_channel(10, 80).unwrap(); // R
        universes[0].set_channel(11, 40).unwrap(); // G
        universes[0].set_channel(12, 0).unwrap(); // B
        engine.apply(&mut universes, &effects);
        let r = universes[0].get_channel(10).unwrap() as f32;
        let g = universes[0].get_channel(11).unwrap() as f32;
        assert_eq!(universes[0].get_channel(12).unwrap(), 0);
        assert!(
            (r / g - 2.0).abs() < 0.1,
            "hue ratio drifted: R={} G={}",
            r,
            g
        );
    }

    #[test]
    fn stop_with_zero_ramp_removes_instance() {
        let mut effects = EffectList::new();
        let id = effects.add(Effect::new());
        let mut engine = EffectEngine::new();
        engine.start(id, vec![1], vec![dimmer_fixture(1)], 0.0);
        assert!(engine.is_running(id));
        engine.stop(id, 0.0);
        assert!(!engine.is_running(id));
        let mut universes = vec![Universe::new(1)];
        engine.apply(&mut universes, &effects);
        assert!(!engine.is_active());
    }

    #[test]
    fn deleted_effect_is_dropped_on_apply() {
        let mut effects = EffectList::new();
        let id = effects.add(Effect::new());
        let mut engine = EffectEngine::new();
        engine.start(id, vec![1], vec![dimmer_fixture(1)], 0.0);
        effects.remove(id);
        let mut universes = vec![Universe::new(1)];
        engine.apply(&mut universes, &effects);
        assert!(!engine.is_active());
    }
}
