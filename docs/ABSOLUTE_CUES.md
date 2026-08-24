# Absolute Cues

EasyCue3's cue list is a **tracking** system: a lighting cue stores only the
channels that *changed* from the previous state, and any channel it doesn't
mention simply holds. Tracking keeps show files small and updates surgical, but
it has one catch — reconstructing the state at a given cue means replaying every
cue from the start of the list, and a wrong or forgotten cue anywhere upstream
silently poisons everything after it.

**Absolute cues** fix that by acting as a full-state checkpoint. An absolute
cue's channel data is a snapshot of the entire output state — every patched
channel, across all universes — captured from the live board. A channel absent
from an absolute cue is 0.

## What an absolute cue changes

**Backward navigation stops hunting.** `CueList::tracked_state_up_to()` (and its
effect analogue `effect_state_up_to()`) rebuild the state for GOTO/BACK/record
baselines by replaying cues forward from the start of the list. When the list
contains absolute cues, the replay now anchors on the **closest absolute cue at
or before the target index** and works forward from that snapshot instead of cue
0. Effect actions reset at the same boundary: anything started before the anchor
is dropped, and the absolute cue's own `effect_actions` (if any) define the
starting effects.

**Forward playback reproduces the snapshot.** `PlaybackEngine::start()` fills
the fade target with 0 and overlays the cue's channels when `data.absolute` is
set, so a GO into an absolute cue fades *everything* — including channels not in
the cue, to 0 — and lands on exactly what was captured. (Tracking cues keep the
old behaviour: target = live state + the cue's deltas.)

**Only patched channels are snapshotted.** The snapshot walks the fixture patch
(`PatchList`), not all 8 × 512 channels — so files stay small even on bigger
rigs.

## Creating and editing

There is no dedicated "record absolute" button. You record a normal tracking
cue, then convert it via the **Absolute** checkbox in Cue Properties:

- **Tracking → absolute** expands the cue to the full tracked state at that
  index (patched channels, non-zero only; absence = 0). Safe to do after the
  fact — the cue always reflects the complete output state at its position.
- **Absolute → tracking** collapses it back to the patched channels that were
  non-zero and differ from the state *before* the cue. Channels at 0 are
  dropped, so a light that the absolute cue had turned off but that was on
  earlier can track back up — the designer's call to catch at the time.

**Update from Stage** (`capture_stage_to_cue`) does the appropriate thing based
on the cue's current mode: absolute cues are re-snapshotted from the live state;
tracking cues store only the deltas vs the state before them.

## UI

- Absolute lighting cues show a **filament lightbulb in amber** in the cue list
  icon column (tracking cues keep the plain lightbulb).
- The **Absolute** checkbox lives in the lighting cue's Properties panel, with a
  hover explanation.

## Show file format

```json
{
  "id": 7,
  "number": 5.0,
  "label": "Broad wash",
  "type": "Lighting",
  "data": {
    "fade_up": 3.0,
    "fade_down": 3.0,
    "channel_values": { "10": 60, "11": 45, "12": 45 },
    "absolute": true
  }
}
```

`"absolute"` is omitted when false (`#[serde(default, skip_serializing_if =
"is_false")]`), so untouched show files stay byte-identical and pre-0.8.2 files
load as tracking cues.

## Key files

- `src/cue/types.rs` — `LightingData.absolute`
- `src/cue/list.rs` — `tracked_state_up_to`, `effect_state_up_to`, `anchor_index`
- `src/cue/playback.rs` — `PlaybackEngine::start` absolute target semantics
- `src/app.rs` — `set_cue_absolute`, `patched_channel_keys`,
  `capture_stage_to_cue`
- `src/ui/properties.rs` — Absolute checkbox
- `src/ui/cues.rs` — absolute cue icon