#!/usr/bin/env python3
"""Measure frame times over five consecutive EasyCue3 runs.

The release binary must include the temporary EASYCUE_PERF_LOG instrumentation
in src/app.rs. Each run launches the app, waits 10 seconds, sends Space to
start the real default show, samples for 5 seconds, then terminates the
process. No screen scraping is used for frame timings.
"""

import csv
import os
import statistics
import subprocess
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APP = ROOT / "target" / "release" / "easycue3"
RUNS = 5
WARMUP_SECONDS = 10.0
MEASURE_SECONDS = 5.0


def find_window(pid, timeout=20):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = subprocess.run(
            ["xdotool", "search", "--onlyvisible", "--pid", str(pid)],
            capture_output=True,
            text=True,
            check=False,
        )
        windows = result.stdout.split()
        if windows:
            return windows[0]
        time.sleep(0.25)
    raise RuntimeError(f"no visible EasyCue3 window for pid {pid}")


def read_samples(path):
    with path.open(newline="") as stream:
        return [
            (float(row["timestamp_ms"]), float(row["frame_time_ms"]))
            for row in csv.DictReader(stream)
        ]


def summarize(path):
    # PerfLogger starts in App::new. Startup is sub-second here and the app is
    # idle until Space after the 10s warmup, so this excludes startup samples
    # while retaining the complete 5s active interval.
    values = [
        dt for timestamp, dt in read_samples(path)
        if 9_000.0 <= timestamp <= 17_000.0 and 0.0 < dt < 1000.0
    ]
    if len(values) < 5:
        raise RuntimeError(f"only {len(values)} usable frame samples in {path}")
    values.sort()
    p95 = values[min(len(values) - 1, int(len(values) * 0.95))]
    return {
        "samples": len(values),
        "mean": statistics.fmean(values),
        "median": statistics.median(values),
        "min": values[0],
        "max": values[-1],
        "p95": p95,
    }


def run_once(index, data_home):
    subprocess.run(["pkill", "-x", "easycue3"], check=False, capture_output=True)
    time.sleep(1.0)
    storage = data_home / "easycue3"
    storage.mkdir(parents=True, exist_ok=True)
    (storage / "app.ron").write_text(
        '{"last_file": "shows/default_show.json", '
        '"last_update_check": "Some(\\"2026-08-25T00:00:00.000000000Z\\")"}'
    )
    log_path = Path(tempfile.gettempdir()) / f"easycue3-perf-{os.getpid()}-{index}.csv"
    log_path.unlink(missing_ok=True)

    env = os.environ.copy()
    env["XDG_DATA_HOME"] = str(data_home)
    env["EASYCUE_PERF_LOG"] = str(log_path)
    run_log = Path(tempfile.gettempdir()) / f"easycue3-perf-{os.getpid()}-{index}.log"
    with run_log.open("w") as output:
        process = subprocess.Popen([str(APP)], cwd=ROOT, env=env, stdout=output, stderr=subprocess.STDOUT)
    try:
        window = find_window(process.pid)
        time.sleep(WARMUP_SECONDS)
        subprocess.run(["xdotool", "windowraise", window], check=False)
        subprocess.run(["xdotool", "windowactivate", "--sync", window], check=False)
        subprocess.run(["xdotool", "windowfocus", window], check=False)
        time.sleep(0.5)
        subprocess.run(["xdotool", "key", "--window", window, "space"], check=False)
        time.sleep(MEASURE_SECONDS)
        # This is intentional: the request is a repeated launch/measure/kill
        # stress test. PerfLogger flushes periodically, so SIGTERM retains the
        # samples without adding a flush syscall to every frame.
        process.terminate()
        process.wait(timeout=15)
        return summarize(log_path)
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()


def main():
    if not APP.exists():
        raise SystemExit(f"missing binary: {APP}")
    with tempfile.TemporaryDirectory(prefix="easycue3-perf-home-") as home:
        data_home = Path(home)
        for index in range(1, RUNS + 1):
            result = run_once(index, data_home)
            print(
                f"run {index}: samples={result['samples']} "
                f"mean={result['mean']:.3f}ms median={result['median']:.3f}ms "
                f"min={result['min']:.3f}ms max={result['max']:.3f}ms "
                f"p95={result['p95']:.3f}ms"
            )


if __name__ == "__main__":
    main()
