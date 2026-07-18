//! Lighting effects — repeating waveforms applied on top of the base look.
//!
//! An [`Effect`] is a show-level library entry: one waveform (sine, square,
//! sawtooth, random) applied to one target (intensity, color, pan/tilt/position)
//! with a rate, size, and per-fixture phase spread. Effects modulate *relative*
//! to the base value the cue/manual programming put in the universe, and are
//! applied at the output stage only (see [`engine::EffectEngine`]) — they are
//! never written back into the stored universes, so recorded cues always
//! capture the un-modulated base look. The UI shows the live modulated values
//! (in a distinct FX color) via [`EffectDisplay`], refreshed each frame.
//!
//! Values follow the internal 0–100 range. A fixture at base 0 with an
//! intensity sine of size 30 swings 0–30 (the negative half clamps away);
//! that asymmetry is accepted for simplicity.

pub mod engine;

pub use engine::{EffectEngine, EffectFootprint, RunningEffect};

use serde::{Deserialize, Serialize};

/// Per-frame snapshot of the effect-modulated output for UI display: the
/// staged universes (effects applied, masters not) plus which fixtures and
/// channels the effects touched. Rebuilt every frame while effects run; the
/// UI reads values from here for modulated channels and from the base
/// universes for everything else — interactions always edit the base.
#[derive(Debug, Clone)]
pub struct EffectDisplay {
    pub universes: Vec<crate::dmx::Universe>,
    pub footprint: EffectFootprint,
}

/// Waveform shapes available to effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Waveform {
    Sine,
    Square,
    SawUp,
    SawDown,
    /// New random level each step; `Effect::smoothing` blends snap → glide.
    Random,
}

impl Waveform {
    pub fn label(&self) -> &'static str {
        match self {
            Waveform::Sine => "Sine",
            Waveform::Square => "Square",
            Waveform::SawUp => "Sawtooth Up",
            Waveform::SawDown => "Sawtooth Down",
            Waveform::Random => "Random",
        }
    }

    pub const ALL: [Waveform; 5] = [
        Waveform::Sine,
        Waveform::Square,
        Waveform::SawUp,
        Waveform::SawDown,
        Waveform::Random,
    ];
}

/// Which fixture parameter the effect modulates.
///
/// `Color` scales all color channels together (hue-preserving brightness).
/// `Position` drives Pan at the wave phase and Tilt 90° behind it, so a sine
/// makes circles and phase spread makes fans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectTarget {
    Intensity,
    Color,
    Pan,
    Tilt,
    Position,
}

impl EffectTarget {
    pub fn label(&self) -> &'static str {
        match self {
            EffectTarget::Intensity => "Intensity",
            EffectTarget::Color => "Color",
            EffectTarget::Pan => "Pan",
            EffectTarget::Tilt => "Tilt",
            EffectTarget::Position => "Position (circle)",
        }
    }

    pub const ALL: [EffectTarget; 5] = [
        EffectTarget::Intensity,
        EffectTarget::Color,
        EffectTarget::Pan,
        EffectTarget::Tilt,
        EffectTarget::Position,
    ];
}

/// A show-level effect definition (library entry).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    /// Stable identity — assigned once by [`EffectList::add`], never reused.
    pub id: u32,
    #[serde(default)]
    pub label: String,
    pub target: EffectTarget,
    pub waveform: Waveform,
    /// Cycles per second (Hz). For Random: new-level steps per second.
    #[serde(serialize_with = "crate::serde_helpers::round_f32_2")]
    pub rate: f32,
    /// Depth in percentage points (0–100): peak deviation from the base value.
    #[serde(serialize_with = "crate::serde_helpers::round_f32_2")]
    pub size: f32,
    /// Degrees (0–360) distributed across the fixture selection so fixtures
    /// run offset from each other (wave/chase looks).
    #[serde(default, serialize_with = "crate::serde_helpers::round_f32_2")]
    pub phase_spread: f32,
    /// Random only: portion of each step (0–100%) spent gliding from the
    /// previous random level to the next. 0 = stepped snap, 100 = smooth drift.
    #[serde(
        default = "default_smoothing",
        serialize_with = "crate::serde_helpers::round_f32_2"
    )]
    pub smoothing: f32,
}

fn default_smoothing() -> f32 {
    50.0
}

impl Effect {
    /// New effect with sensible defaults; id 0 means "unassigned" until added.
    pub fn new() -> Self {
        Self {
            id: 0,
            label: String::new(),
            target: EffectTarget::Intensity,
            waveform: Waveform::Sine,
            rate: 1.0,
            size: 25.0,
            phase_spread: 0.0,
            smoothing: default_smoothing(),
        }
    }
}

impl Default for Effect {
    fn default() -> Self {
        Self::new()
    }
}

/// Effect library with stable-ID management — mirrors `CueList`.
#[derive(Debug, Clone)]
pub struct EffectList {
    effects: Vec<Effect>,
    next_id: u32,
}

impl Default for EffectList {
    fn default() -> Self {
        Self::new()
    }
}

impl EffectList {
    pub fn new() -> Self {
        Self {
            effects: Vec::new(),
            next_id: 1,
        }
    }

    /// Rebuild from persisted parts, assigning IDs to any effect missing one.
    pub fn from_parts(effects: Vec<Effect>, next_id: u32) -> Self {
        let mut list = Self {
            effects,
            next_id: next_id.max(1),
        };
        let max_existing = list.effects.iter().map(|e| e.id).max().unwrap_or(0);
        list.next_id = list.next_id.max(max_existing + 1);
        for effect in &mut list.effects {
            if effect.id == 0 {
                effect.id = list.next_id;
                list.next_id += 1;
            }
        }
        list
    }

    /// Add an effect, assigning a stable ID if id == 0. Returns the ID.
    pub fn add(&mut self, mut effect: Effect) -> u32 {
        if effect.id == 0 {
            effect.id = self.next_id;
            self.next_id += 1;
        } else {
            self.next_id = self.next_id.max(effect.id + 1);
        }
        let id = effect.id;
        self.effects.push(effect);
        id
    }

    pub fn remove(&mut self, id: u32) -> Option<Effect> {
        let idx = self.effects.iter().position(|e| e.id == id)?;
        Some(self.effects.remove(idx))
    }

    pub fn find(&self, id: u32) -> Option<&Effect> {
        self.effects.iter().find(|e| e.id == id)
    }

    pub fn find_mut(&mut self, id: u32) -> Option<&mut Effect> {
        self.effects.iter_mut().find(|e| e.id == id)
    }

    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }

    pub fn next_id(&self) -> u32 {
        self.next_id
    }

    pub fn len(&self) -> usize {
        self.effects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    pub fn clear(&mut self) {
        self.effects.clear();
        self.next_id = 1;
    }
}

/// A cue-attached effect action, executed when the cue fires. Running effects
/// track through subsequent cues until explicitly stopped (like channel values).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum EffectAction {
    /// Start (or retarget) an effect on a set of fixture IDs (patch IDs, sorted).
    Start {
        effect_id: u32,
        fixtures: Vec<usize>,
    },
    Stop {
        effect_id: u32,
    },
    StopAll,
}

/// A fixture resolved to plain DMX channel data, built by the app from the
/// patch + profile at effect-start time. The engine only sees this — it keeps
/// the lib-side engine independent of the fixture-library types and keeps the
/// per-frame hot path free of profile lookups.
#[derive(Debug, Clone)]
pub struct EffectFixture {
    /// Patch ID — determines phase order and seeds the random generator.
    pub fixture_id: usize,
    /// 0-based index into the app's universe vec.
    pub universe_idx: usize,
    /// Absolute 1-based DMX channel of the dedicated intensity, if any.
    pub intensity_ch: Option<u16>,
    /// Absolute channels of all color parameters (R/G/B/A/W/UV present).
    pub color_chs: Vec<u16>,
    pub pan_ch: Option<u16>,
    pub tilt_ch: Option<u16>,
}

/// Deterministic per-(effect, fixture, step) random in [0, 1) — splitmix64-style
/// hash, so Random effects need no stored RNG state and replay identically.
#[inline]
fn hash01(effect_id: u32, salt: u64, step: i64) -> f32 {
    let mut z =
        (effect_id as u64) ^ (salt << 24) ^ (step as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z ^= z >> 30;
    z = z.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z ^= z >> 27;
    z = z.wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / 16_777_216.0
}

/// Sample the effect's waveform in [-1, 1] at absolute time `t` seconds for
/// fixture `i` of `n`. `quadrature` shifts the phase 90° (used for the Tilt
/// half of a Position effect, and decorrelates its random stream).
pub fn sample(
    effect: &Effect,
    t: f32,
    i: usize,
    n: usize,
    fixture_id: usize,
    quadrature: bool,
) -> f32 {
    let phase_off = (effect.phase_spread / 360.0) * (i as f32 / n.max(1) as f32)
        + if quadrature { 0.25 } else { 0.0 };
    let x = t * effect.rate + phase_off;
    let p = x.fract();
    match effect.waveform {
        Waveform::Sine => (x * std::f32::consts::TAU).sin(),
        Waveform::Square => {
            if p < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Waveform::SawUp => 2.0 * p - 1.0,
        Waveform::SawDown => 1.0 - 2.0 * p,
        Waveform::Random => {
            let step = x.floor() as i64;
            let salt = fixture_id as u64 | if quadrature { 1 << 22 } else { 0 };
            let cur = hash01(effect.id, salt, step) * 2.0 - 1.0;
            let s = effect.smoothing / 100.0;
            if s <= 0.001 {
                cur
            } else {
                let prev = hash01(effect.id, salt, step - 1) * 2.0 - 1.0;
                prev + (cur - prev) * (p / s).min(1.0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effect(waveform: Waveform) -> Effect {
        Effect {
            id: 7,
            waveform,
            rate: 2.0,
            size: 30.0,
            ..Effect::new()
        }
    }

    #[test]
    fn waveforms_stay_in_range() {
        for wf in Waveform::ALL {
            let e = effect(wf);
            for step in 0..1000 {
                let t = step as f32 * 0.0173;
                let v = sample(&e, t, 0, 1, 3, false);
                assert!(
                    (-1.0..=1.0).contains(&v),
                    "{:?} out of range at t={}: {}",
                    wf,
                    t,
                    v
                );
            }
        }
    }

    #[test]
    fn periodic_waveforms_repeat_each_cycle() {
        for wf in [
            Waveform::Sine,
            Waveform::Square,
            Waveform::SawUp,
            Waveform::SawDown,
        ] {
            let e = effect(wf);
            let period = 1.0 / e.rate;
            for step in 0..50 {
                let t = step as f32 * 0.031;
                let a = sample(&e, t, 0, 1, 3, false);
                let b = sample(&e, t + period, 0, 1, 3, false);
                assert!((a - b).abs() < 1e-3, "{:?} not periodic at t={}", wf, t);
            }
        }
    }

    #[test]
    fn phase_spread_offsets_fixtures() {
        let e = Effect {
            phase_spread: 360.0,
            ..effect(Waveform::SawUp)
        };
        // Fixture 1 of 4 runs a quarter-cycle behind fixture 0.
        let quarter = 0.25 / e.rate;
        let a = sample(&e, 0.4 + quarter, 0, 4, 1, false);
        let b = sample(&e, 0.4, 1, 4, 2, false);
        assert!((a - b).abs() < 1e-3);
    }

    #[test]
    fn stepped_random_holds_within_a_step() {
        let e = Effect {
            smoothing: 0.0,
            ..effect(Waveform::Random)
        };
        // rate 2.0 → step duration 0.5s; samples inside one step are identical.
        let a = sample(&e, 0.05, 0, 1, 3, false);
        let b = sample(&e, 0.45, 0, 1, 3, false);
        assert_eq!(a, b);
        // ...and differ across steps (deterministic hash makes this stable).
        let c = sample(&e, 0.55, 0, 1, 3, false);
        assert_ne!(a, c);
    }

    #[test]
    fn smooth_random_is_continuous_at_step_boundaries() {
        let e = Effect {
            smoothing: 100.0,
            ..effect(Waveform::Random)
        };
        let before = sample(&e, 0.499, 0, 1, 3, false);
        let after = sample(&e, 0.501, 0, 1, 3, false);
        assert!(
            (before - after).abs() < 0.05,
            "jump at boundary: {} vs {}",
            before,
            after
        );
    }

    #[test]
    fn random_is_deterministic_per_fixture() {
        let e = effect(Waveform::Random);
        assert_eq!(
            sample(&e, 1.23, 0, 2, 5, false),
            sample(&e, 1.23, 0, 2, 5, false)
        );
        // Different fixtures get independent streams.
        assert_ne!(
            sample(&e, 1.23, 0, 2, 5, false),
            sample(&e, 1.23, 1, 2, 6, false)
        );
    }

    #[test]
    fn effect_list_assigns_stable_ids() {
        let mut list = EffectList::new();
        let a = list.add(Effect::new());
        let b = list.add(Effect::new());
        assert_eq!((a, b), (1, 2));
        list.remove(a);
        let c = list.add(Effect::new());
        assert_eq!(c, 3, "IDs are never reused");
    }

    #[test]
    fn effect_action_serde_round_trip() {
        let actions = vec![
            EffectAction::Start {
                effect_id: 1,
                fixtures: vec![1, 2, 3],
            },
            EffectAction::Stop { effect_id: 1 },
            EffectAction::StopAll,
        ];
        let json = serde_json::to_string(&actions).unwrap();
        let back: Vec<EffectAction> = serde_json::from_str(&json).unwrap();
        assert_eq!(actions, back);
    }
}
