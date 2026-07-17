# Effects

Repeating waveform patterns applied on top of the base look — a deliberately
small slice of the ETC EOS effects system, sized for small-venue operators.

## Concept

An **effect** is a show-level library entry: one waveform applied to one target
with a rate, size, and phase spread.

| Control | Meaning |
|---|---|
| Waveform | Sine, Square, Sawtooth Up/Down, Random |
| Target | Intensity, Color, Pan, Tilt, Position (pan+tilt in quadrature → circles) |
| Rate | Cycles per second (Hz). For Random: new-level steps per second |
| Size | Peak deviation from the base value, in percentage points (0–100 scale) |
| Phase spread | Degrees (0–360) distributed across the fixture selection — offset fixtures make waves and chases |
| Smoothing | Random only: 0% snaps between random levels (flicker/lightning), 100% glides (fire/water) |

Effects are **relative**: they modulate around whatever base value the cue or
manual programming put in the universe, clamped to 0–100. A fixture at base 50
with a size-30 sine swings 20–80; at base 0 it swings 0–30 (the negative half
clamps away).

## Triggering — tracking-style via cues

A lighting cue can carry **effect actions** (Cue Properties → Effects):
*Start effect E on fixtures F*, *Stop effect E*, or *Stop all*. Starts ramp the
effect in over the cue's fade-up; stops ramp out over its fade-down. A started
effect keeps running through later cues until one stops it — exactly like
channel tracking. BACK and GOTO replay the action history
(`CueList::effect_state_up_to`) so jumps land with the correct effects running,
without resetting the phase of effects that survive the jump.

Cue 0 / fade-to-black stops all effects with the fade. The blackout *toggle*
and grand master only scale the output — effects keep running underneath and
return when the master comes back. PANIC and ALL STOP kill effects instantly.
The Effects panel also has manual Start-on-Selection/Stop for programming.

## Architecture — output-stage overlay

There are no HTP/LTP layers in EasyCue3: cue fades, manual edits, and virtual
intensity all write into one shared `Universe` buffer. Effects therefore never
touch that buffer. Each frame, `EasyCueApp::update()` clones the universes,
lets `EffectEngine::apply()` modulate the clone, then applies masters and sends
DMX:

```
base universes ──clone──▶ + effects ──▶ + masters ──▶ DMX backend
      │
      └──▶ UI readouts, cue recording, tracking (never see effect values)
```

Consequences:
- **Recording a cue never bakes effect output in** — `record_cue` reads the
  base universes.
- **Channel readouts show the base look** while the DMX output modulates.
- No cross-frame feedback: every frame recomputes from the untouched base.

While any effect runs, the app requests continuous ~60 fps repaints (the app
otherwise idles and the output would freeze).

## Target application

Fixture parameters are resolved to absolute DMX channels once at effect start
(`EasyCueApp::resolve_effect_fixtures`), not per frame:

- **Intensity** with a real intensity channel: base + delta on that channel.
- **Intensity** on an RGB-only fixture: hue-preserving scale of all color
  channels (same model as virtual intensity). A fixture at base black stays
  black — an effect cannot invent a color.
- **Color**: the same hue-preserving scale, regardless of intensity channel.
- **Pan / Tilt**: base + delta on the coarse channel (8-bit scope; fine
  channels are ignored).
- **Position**: pan at the wave phase, tilt 90° behind it.

Random is a deterministic hash of (effect id, fixture id, step) — no RNG state,
so replays and multi-fixture spreads are stable.

## Key files

- `src/effects/mod.rs` — `Effect`, `Waveform`, `EffectTarget`, `EffectList`,
  `EffectAction`, waveform sampling
- `src/effects/engine.rs` — `EffectEngine`: running instances, ramps, per-frame apply
- `src/app.rs` — `resolve_effect_fixtures`, `execute_effect_actions`,
  `sync_effects_to_index`, frame-loop integration
- `src/cue/list.rs` — `effect_state_up_to` (tracking replay)
- `src/ui/effects.rs` — Effects panel; `src/ui/properties.rs` — cue action editor
