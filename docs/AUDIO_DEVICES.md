# Audio Output Devices

EasyCue3 opens every output device it can find at startup (`AudioPlayer::open_all_outputs`,
`src/audio/player.rs`) and lets each audio cue route to any combination of them
independently, each with its own volume and pan (`Output Volume & Pan` in the
cue properties panel, and `Output Fades` on Adjust cues for crossfading between
them). This works out of the box for plain stereo devices. Multi-channel USB
interfaces need a bit of one-time setup on Linux — see below.

## Why some devices don't show up, or only play out the front pair

On Linux, EasyCue3 talks to audio hardware through ALSA (via `cpal`/`rodio`).
Most modern distros (including this one) run **PipeWire**, which takes
exclusive control of the raw hardware — a direct ALSA `hw:`/`plughw:` open of
a device PipeWire already owns fails with "Failed to get the config for the
given device". You'll see this in the logs for the raw hardware entries; it's
harmless as long as a working route to the same hardware exists.

A second, separate issue affects **multi-channel interfaces** specifically
(e.g. a Roland Rubix24 in its 4-channel "Analog Surround 4.0" mode, with
front L/R + rear L/R). EasyCue3's audio pipeline only ever produces plain
stereo per route (`PanSource` in `src/audio/pan_source.rs`). When that stereo
stream is handed to a 4-channel device, PipeWire lands it on the first pair
(front) — the rear pair is simply never addressed, with no error or
indication anything's wrong.

## Fix: named devices in `~/.asoundrc`

Both problems are solved the same way: define named ALSA PCM devices that
route *through* PipeWire's own mixing graph (so they don't fight its
exclusive hardware claim) instead of opening hardware directly. EasyCue3
doesn't need any code changes for this — `open_all_outputs()` already
enumerates every ALSA-visible device by name, so anything defined here just
shows up in the existing device dropdown.

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
    hint { show on; description "Onboard Speakers" }
}
ctl.speakers { type pipewire }

pcm.rubix24 {
    type pipewire
    playback_node "alsa_output.usb-Roland_Rubix24-00.analog-surround-40"
    hint { show on; description "Rubix24" }
}
ctl.rubix24 { type pipewire }
```

### Reaching individual channel pairs on a multi-channel interface

For the rear pair of a 4-channel interface, layer an ALSA `route` plugin on
top of a hidden 4-channel node:

```
# Full 4-channel node; hidden from the picker, only used as the route's slave.
pcm.rubix24_4ch {
    type pipewire
    playback_node "alsa_output.usb-Roland_Rubix24-00.analog-surround-40"
    channels 4
    hint { show off }
}
pcm.rubix24_rear {
    type route
    slave.pcm "rubix24_4ch"
    slave.channels 4
    ttable.0.2 1   # input L -> output channel 2 (rear L)
    ttable.1.3 1   # input R -> output channel 3 (rear R)
    hint { show on; description "Rubix24 Rear" }
}
ctl.rubix24_rear { type pipewire }
```

The plain `rubix24` entry above already serves as the front pair (PipeWire
negotiates the stereo stream onto channels 0/1 by default), so there's no
need for a separate explicit "front" alias.

Verify a new entry before relying on it:

```bash
aplay -L | grep -A1 rubix24        # confirm it's listed with the right description
aplay -D rubix24_rear /usr/share/sounds/alsa/Front_Center.wav   # confirm it actually plays
```

Restart EasyCue3 after editing `~/.asoundrc` — devices are enumerated once at
startup.

### `open_all_outputs()` noise filtering

The device picker filters out ALSA meta-plugins that never represent a real
destination (`null`, `default`, `pipewire`, `pulse`, `jack`, `oss`, and the
rate/remix plugins) by their raw ALSA PCM id, not their description text —
some of these get pretty confusing descriptions (e.g. plain `default` shows
up as *"Default ALSA Output (currently PipeWire Media Server)"*). See
`AudioPlayer::NON_DEVICE_PLUGIN_IDS` in `src/audio/player.rs` if a real device
ever gets caught by this filter, or a new noise plugin needs adding.

## Known limitations

- **Linux/PipeWire-only.** This whole approach is ALSA config; it doesn't
  apply to macOS (CoreAudio) or Windows (WASAPI) builds, and doesn't apply
  as-written to plain-PulseAudio or server-less ALSA setups either (those
  need different plugin types — `pulse` or raw `hw:`/`plughw:` respectively).
- **Per-machine, not portable.** `~/.asoundrc` lives outside the show file
  and outside git — it has to be set up again on every machine that needs
  the extra devices.
- **PipeWire node names can drift.** They're built from USB/udev descriptor
  strings and are generally stable, but a driver update, a second identical
  interface, or a PipeWire profile change (e.g. switching the interface from
  "Analog Surround 4.0" to "Pro Audio" mode, which exposes channels
  differently) can invalidate `playback_node` or the `ttable` channel
  mapping. Breakage is silent — the device stays listed in EasyCue3 but
  fails at cue time with only a log line.
- **No hot-plug.** Devices are enumerated once at app startup. Plugging in
  an interface (or a monitor, for HDMI audio) after launch won't add it to
  the dropdown until EasyCue3 restarts; unplugging one mid-show won't
  disable its route or warn the operator, it'll just fail silently.
