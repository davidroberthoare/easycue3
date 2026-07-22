# Audio Output Devices

EasyCue3 opens every output device it can find at startup (`AudioPlayer::open_all_outputs`,
`src/audio/player.rs`) and lets each audio cue route to any combination of them
independently, each with its own volume and pan (`Output Volume & Pan` in the
cue properties panel, and `Output Fades` on Adjust cues for crossfading between
them).

**Multi-channel devices are supported natively.** A device that reports more
than two channels (e.g. a Roland Rubix24 with front L/R + rear L/R) is opened
at its full width — capped at 8 channels (`MAX_OUTPUT_CHANNELS`) — and each
stereo pair appears as its own entry in the output dropdowns: "Rubix24 ·
Out 1-2", "Rubix24 · Out 3-4", and so on. A cue can play on any pair, on
several pairs at once, and an Adjust cue can crossfade between pairs exactly
like between separate devices. Plain stereo devices appear as a single entry,
same as always.

Routing onto a pair is done in the app's output stage (`RouteSource` in
`src/audio/route_source.rs`): the decoded audio is folded to stereo, panned,
and placed on the pair's two channels of the device stream with silence
everywhere else. Because it's the app doing the placement, this works the same
on every platform — no OS-level channel-mapping config involved.

## What decides how many pairs a device offers

The channel count comes from the device's *default output config*
(`AudioPlayer::preferred_channels`), not from its advertised
supported-config range. On Windows (WASAPI) and macOS (CoreAudio) the default
config is the device's native mix format, so multi-channel interfaces just
work. On Linux it depends on what ALSA reports — see below.

## Linux: PipeWire setup

On Linux, EasyCue3 talks to audio hardware through ALSA (via `cpal`/`rodio`).
Most modern distros run **PipeWire**, which takes exclusive control of the raw
hardware — a direct ALSA `hw:`/`plughw:` open of a device PipeWire already
owns fails with "Failed to get the config for the given device". You'll see
this in the logs for the raw hardware entries; it's harmless as long as a
working route to the same hardware exists.

The fix is to define named ALSA PCM devices that route *through* PipeWire's
own mixing graph instead of opening hardware directly. Anything defined this
way shows up in EasyCue3's device dropdowns automatically.

Find the PipeWire node name for the sink you want (stable across reboots,
tied to the hardware):

```bash
pactl list sinks short
```

Then add an entry per device to `~/.asoundrc`:

```
pcm.speakers {
    type pipewire
    playback_node "alsa_output.pci-..._sink"     # from pactl list sinks short
    channels 2
    hint { show on; description "Onboard Speakers" }
}
ctl.speakers { type pipewire }

pcm.rubix24 {
    type pipewire
    playback_node "alsa_output.usb-Roland_Rubix24-00.analog-surround-40"
    channels 4
    hint { show on; description "Rubix24" }
}
ctl.rubix24 { type pipewire }
```

**The `channels N` line matters.** PipeWire alias devices advertise support
for 1–32 channels no matter what the real sink looks like, so EasyCue3 reads
the *default* channel count to decide what to offer — and that default is
stereo unless the alias pins it. Pin `channels 4` and EasyCue3 opens the
device 4-wide and offers "Out 1-2" / "Out 3-4"; leave it unpinned and only
the front pair is reachable. Match `N` to the sink's real channel count —
PipeWire maps stream channels to sink channels by position, so a mismatch
lands audio on the wrong outputs.

Verify a new entry before relying on it:

```bash
aplay -L | grep -A1 rubix24        # confirm it's listed with the right description
aplay -D rubix24 /usr/share/sounds/alsa/Front_Center.wav   # confirm it actually plays
```

Restart EasyCue3 after editing `~/.asoundrc` — devices are enumerated once at
startup.

> Older versions (≤ v0.5.x) couldn't address channel pairs and needed an ALSA
> `route`-plugin remap (`ttable`) to expose e.g. a rear pair as its own
> device. That workaround is obsolete — delete or hide those entries so the
> same outputs don't show up twice.

### `open_all_outputs()` noise filtering

The device picker filters out ALSA meta-plugins that never represent a real
destination (`null`, `default`, `pipewire`, `pulse`, `jack`, `oss`, and the
rate/remix plugins) by their raw ALSA PCM id, not their description text —
some of these get pretty confusing descriptions (e.g. plain `default` shows
up as *"Default ALSA Output (currently PipeWire Media Server)"*). See
`AudioPlayer::NON_DEVICE_PLUGIN_IDS` in `src/audio/player.rs` if a real device
ever gets caught by this filter, or a new noise plugin needs adding.

## Known limitations

- **Linux needs the `~/.asoundrc` setup above** for anything beyond the
  default device; it's per-machine config, outside the show file and git.
  Windows/macOS builds need nothing.
- **PipeWire node names can drift.** They're built from USB/udev descriptor
  strings and are generally stable, but a driver update, a second identical
  interface, or a PipeWire profile change (e.g. switching the interface from
  "Analog Surround 4.0" to "Pro Audio" mode, which exposes channels
  differently) can invalidate `playback_node` or the pinned channel count.
  Breakage is silent — the device stays listed in EasyCue3 but its audio
  falls back to the default sink.
- **No hot-plug.** Devices are enumerated once at app startup. Plugging in
  an interface (or a monitor, for HDMI audio) after launch won't add it to
  the dropdown until EasyCue3 restarts; unplugging one mid-show won't
  disable its route or warn the operator, it'll just fail silently.
- **Pairs, not arbitrary channels.** Routing targets stereo pairs (1-2, 3-4,
  …). Odd channel counts ignore the trailing channel; there's no per-single-
  channel (mono) routing.
