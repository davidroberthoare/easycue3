# Performance, renderer backend & frame-time benchmarking

Status of the renderer decision, the frame-rate investigation, and the tooling
available to measure it. Read this before doing any performance work — it
records what was already measured and how to reproduce it.

## Current renderer: wgpu (default)

Since commit `bdcb5bc`, eframe is built with the **wgpu** renderer instead of
glow:

```toml
# Cargo.toml
eframe = { version = "0.31", default-features = false, features = ["wgpu", "default_fonts", "x11", "wayland", "persistence"] }
```

- Backends: **Vulkan** on Linux (Mesa Intel iGPU verified), **Metal** on macOS.
  If Vulkan is unavailable wgpu falls back to its GLES/EGL path.
- Deps: `egui-wgpu 0.31.1` + `wgpu 24.0.5` were already in the lockfile
  (transitively via `lumina-video`), so the switch added no version risk.
- `egui`/`eframe` remain pinned to **0.31** (hard constraint, lumina-video).

### Why

Measured on the reference machine (Mesa Intel UHD Graphics, CML GT2 iGPU,
3840×2160 window) with the same 5-run `EASYCUE_PERF_LOG` harness:

| backend | mean frame time | median |
|---|---|---|
| glow (OpenGL/GLX) | ~24.6 ms | ~27 ms |
| **wgpu / Vulkan** | ~16.5 ms | ~10 ms |

`egui-wgpu` uses `PresentMode::AutoVsync` (throttles only when a frame is still
pending), so the sub-16.7 ms median is real render time, not a vsync artifact.

### App::on_exit is renderer-agnostic

eframe's `App::on_exit` takes `Option<&glow::Context>` only under the glow
feature; the wgpu build has a no-arg signature. `EasyCueApp::on_exit` therefore
just calls `EasyCueApp::shutdown_sequence()` (`src/app.rs`), which holds the
real shutdown body. Do not re-add the `eframe::glow` signature without also
re-adding the `glow` feature.

## CPU-profile findings (for context)

A `samply`/`perf` sample of the active render window (playback running) showed
the ~24 ms/frame (glow) was **not** a single app function. Top attribution by
nearest Rust caller:

- `egui_glow::Painter::paint_mesh` + glutin `glXSwapBuffers` + Mesa gallium:
  ~33% of main-thread samples (~8–9 ms/frame) — the GL submit path.
- `alloc::raw_vec::RawVecInner::finish_grow`: ~1.6 ms/frame (per-frame mesh
  buffer growth).
- x11rb `wait_for_reply_or_error` ← `Window::is_minimized` ←
  `egui_winit::update_viewport_info`: ~0.6 ms/frame (a synchronous X11
  round-trip **every frame** — independently fixable rough edge, not yet done).
- Long tail: egui widget creation, text layout/tessellation, Vec drops.

The chromaticity picker (`src/ui/chromaticity.rs`) is empty — replaced by
`src/ui/color_wheel.rs`, whose gradient is **cached** as an `egui::TextureHandle`
(rebuilt only on size change, `color_wheel.rs:57-60`); it is not a per-frame cost.

Cargo.lock history showed egui/eframe/glow/glutin/winit were **unchanged since
the first commit**, ruling out a renderer dependency bump as the regression cause.

## 2026-08 finding: the 30–40fps "slowness" is swapchain frame pacing, not render cost

Measured with the extended `EASYCUE_PERF_LOG` CSV (adds `ui_render_ms`,
`dmx_send_ms`, `update_cpu_ms` columns) on the reference machine, default
7-tab layout, playback running:

| quantity | value |
|---|---|
| total CPU per frame (`update_cpu`, everything incl. UI + DMX) | median **0.8ms**, mean 1.4ms, max 4.6ms |
| CPU UI render (`ui_render`) | median **0.76ms**, max 3.9ms |
| DMX send | ~0.04ms |

So the app is **not** CPU-bound. Nor is it fill-rate bound: halving the window
(1024×576) and quartering the UI via `EASYCUE_PIXELS_PER_POINT=0.5` changed
nothing (identical mean/median/p95). `perf` shows only a diffuse tail (alloc,
tessellation, text layout) — no single app hotspot.

The actual pattern is a **frame-pacing cycle**: ~2 fast frames (~10ms) then one
~34ms stall, repeating every ~3 frames. The app renders in ~2ms, races ahead of
the 60Hz swapchain (`egui-wgpu` default `PresentMode::AutoVsync` = Fifo, depth
2–3), fills the queue, and every third `acquire` blocks ~30ms. That stall is the
"30–40fps" the FPS counter shows.

**Fix (`src/app.rs`): request continuous repaints at a 20ms cadence instead of
16ms.** A cadence slightly *above* one refresh period (16.67ms @ 60Hz) means the
loop never produces frames faster than the display consumes them, so the
swapchain never fills and the periodic stalls vanish. Result (5 runs each, same
harness, playback animating):

| config | median | p95 | max |
|---|---|---|---|
| old (16ms cadence) | ~10.7ms (hitching) | **~34.8ms** | ~41ms |
| new (20ms cadence) | **16.667ms** | **16.667ms** | ~30ms (rare, on cue-fire/audio-load) |
| new + all 8 views incl. PDF script viewer | **16.667ms** | **16.667ms** | ~35ms (same rare cue-fire frames) |

Rock-steady 60fps (the display's maximum) with zero hitches in every run. The
remaining rare ~30ms one-offs coincide with GO presses that load/stop audio
samples on the main thread — a separate, pre-existing cost.

**Trade-off:** on 120/144Hz displays the 20ms floor caps animation at ~50fps
(still hitch-free). Override with `EASYCUE_REPAINT_MS=9` (or 7) on fast panels.

**Bonus — repaints are now animation-gated.** The scheduler in `update()` only
requests continuous frames while something actually animates: a fade in
progress (`fade_progress()`), a running effect, audio playback, a pending
autofollow (wakes at the *remaining* delay, not at full cadence), the audio-
master ramp, DMX reconnect, or a dead audio output's recovery scan. When the
output is static (e.g. an `Active` cue holding levels), the app sleeps and
wakes on input — verified: zero PerfLogger records accumulate while static.

Benchmark note: with gating, the harness must keep pressing GO so fades run for
the whole sampling window (it now does — Space every 2s).

## Frame-time measurement tooling

### In-app instrumentation: `EASYCUE_PERF_LOG`

`EasyCueApp` has an env-gated `PerfLogger` (`src/app.rs`). It is a **no-op when
the env var is unset** (a single `None` check per frame), so it is safe to leave
in place.

```bash
# writes CSV to the given path (timestamp_ms, frame_time_ms) per rendered frame
EASYCUE_PERF_LOG=/tmp/perf.csv ./target/release/easycue3
# or: EASYCUE_PERF_LOG=1  ->  $TMPDIR/easycue3-perf-<pid>.csv
```

- `frame_time_ms` is egui's `stable_dt` (inter-frame delta) × 1000.
- Since the 2026-08 finding the CSV also carries `ui_render_ms` (CPU time of the
  UI pass), `dmx_send_ms`, and `update_cpu_ms` (total CPU time of the whole
  `update()` frame) — use these to attribute a spike to CPU work vs.
  present/swapchain wait (frame_time − update_cpu).
- Writes are buffered; flushed every 64 records and on shutdown. Intended to be
  parsed after the process exits.
- egui only repaints on demand, so **idle frames produce few samples**. Start
  playback (Space) or an effect to exercise the continuous-repaint path.

### Benchmark harness: `scripts/benchmark_frame_time.py`

Linux-only (uses `xdotool`). Requires the release binary built with the
`EASYCUE_PERF_LOG` instrumentation (i.e. just build normally — it's always
compiled in).

```bash
cargo build --release
python3 scripts/benchmark_frame_time.py
# environment:
#   RUNS=n            runs (default 5)
#   BENCH_LAYOUT=all  worst-case layout: default 7 tabs + Script Viewer with
#                     media/lorem.pdf (needs PDFIUM_LIBRARY_PATH, auto-set from
#                     target/debug/libpdfium.so if present)
```

Per run it: launches the app (isolated `XDG_DATA_HOME`), waits 10 s, sends
Space to start playback, samples for 5 s, then SIGTERMs the process. Prints
per-run `samples / mean / median / min / max / p95` (5 runs by default; set
`RUNS`).

2026-08 changes to the harness (and why):
- **Moves `shows/.autosave.json` aside per run** — a newer autosave triggers the
  "Recover Unsaved Work?" modal at startup, which blocked the Space key that
  starts playback (the old harness silently measured idle: empty CSV).
- **Presses Space every 2s during sampling** — with repaints now animation-gated
  (see above), a single GO would let fades finish and the app fall idle mid-run.
- **`BENCH_LAYOUT=all`** sets `EASYCUE_PERF_LAYOUT=all` in the app (a startup
  hook that opens the Script Viewer tab and points it at `media/lorem.pdf`;
  no-op when unset) so the worst-case multi-view layout — fixtures list, cue
  list, magic sheet, fixture properties, script viewer — is measured together.

Other benchmark/diagnostic env vars (all no-op when unset):
- `EASYCUE_REPAINT_MS` — continuous-repaint cadence (default 20; the 16ms value
  reproduces the old hitching).
- `EASYCUE_PIXELS_PER_POINT` — UI scale override (used to prove fill-rate
  independence; on this machine it changes nothing).
- `EASYCUE_PRESENT_MODE` (`vsync|novsync|mailbox|immediate`) and
  `EASYCUE_FRAME_LATENCY` — wgpu swapchain knobs, wired in `main.rs`; present
  mode and latency changes did not fix the pacing stalls (only the repaint
  cadence did).

Caveats:
- The 5 s sampling window mixes a little idle + active playback (identical
  across runs, so comparisons are still fair).
- First-frame wgpu shader/pipeline compilation causes slow outliers (p95 ~34 ms,
  max ~40 ms) in each short session — compare **median** primarily, or run
  longer than the default.
- `xdotool` is Linux-only; a macOS port would need a different input driver, but
  the `EASYCUE_PERF_LOG` CSV format and analysis are cross-platform.

## Re-enabling CPU profiling

`[profile.release] debug = true` was deliberately **removed** (it bloated the
release binary to ~422 MB and isn't needed for frame-time work). To CPU-profile
again, add it temporarily, then use `samply record --save-only -o prof.json --`
or `perf record`. Requires `kernel.perf_event_paranoid <= 1` for a non-root user
(`echo '1' | sudo tee /proc/sys/kernel/perf_event_paranoid`). Note samply's JSON
export comes back `symbolicated: false`; resolve addresses with `nm -C -n` /
`addr2line` (slow on full-DWARF builds) or open the profile in the samply web UI.

## Key file references

- Renderer switch: `Cargo.toml`
- Frame logger: `PerfLogger` in `src/app.rs`
- Shutdown path: `EasyCueApp::shutdown_sequence` / `on_exit` in `src/app.rs`
- Color wheel caching: `src/ui/color_wheel.rs`
- Benchmark: `scripts/benchmark_frame_time.py`
