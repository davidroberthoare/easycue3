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
```

Per run it: launches the app (isolated `XDG_DATA_HOME`), waits 10 s, sends
Space to start playback, samples for 5 s, then SIGTERMs the process. Prints
per-run `samples / mean / median / min / max / p95` (5 runs by default; set
`RUNS`).

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
