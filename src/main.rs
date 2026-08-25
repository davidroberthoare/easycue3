//! EasyCue3 - Theatrical Lighting & Media Console
//!
//! A simple lighting console for small-scale theatre and schools,
//! combining ETC EOS-style lighting control with QLab-style media playback.

mod app;
mod groups;
mod hotkeys;
mod magic_sheet;
mod media;
mod scriptviewer;
mod ui;
mod fixtures;
mod show;
mod command;
#[cfg(feature = "remote")]
mod remote;
mod update;

// Use library modules (dmx, cue, audio, effects are defined in lib.rs)
use easycue3::{dmx, cue, audio, effects};
pub use easycue3::paths;

use app::EasyCueApp;

fn main() -> eframe::Result<()> {
    let process_start = std::time::Instant::now();

    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

    log::info!("Starting EasyCue3... pid={}", std::process::id());

    // Guard eframe's persisted settings file: if a previous run was killed mid-write
    // (Ctrl+C or a crashed exit) it can be left truncated, which eframe would silently
    // treat as "no settings" and reset the UI layout + last-loaded show. Back the bad
    // file up so the app starts clean and the old state stays recoverable.
    heal_storage_if_corrupt();

    // Load embedded application icon
    let icon_start = std::time::Instant::now();
    let icon = load_icon();
    log::info!("[startup] Icon load phase completed in {:.2}ms", icon_start.elapsed().as_secs_f64() * 1000.0);

    // Configure the native window
    let window_setup_start = std::time::Instant::now();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("EasyCue3 - Theatrical Lighting Console")
        .with_inner_size([1280.0, 720.0])
        .with_min_inner_size([800.0, 600.0]);
    
    // Set icon if loaded successfully
    if let Some(icon_data) = icon {
        viewport = viewport.with_icon(icon_data);
    }

    // Present-mode override for A/B benchmarking (EASYCUE_PRESENT_MODE=mailbox
    // | novsync | vsync). Falls back to egui-wgpu's default AutoVsync when unset.
    let mut wgpu_options = eframe::egui_wgpu::WgpuConfiguration::default();
    if let Ok(mode) = std::env::var("EASYCUE_PRESENT_MODE") {
        wgpu_options.present_mode = match mode.as_str() {
            "mailbox" => eframe::wgpu::PresentMode::Mailbox,
            "novsync" => eframe::wgpu::PresentMode::AutoNoVsync,
            "immediate" => eframe::wgpu::PresentMode::Immediate,
            _ => eframe::wgpu::PresentMode::AutoVsync,
        };
        log::info!("[startup] EASYCUE_PRESENT_MODE={mode} -> {:?}", wgpu_options.present_mode);
    }
    // EASYCUE_FRAME_LATENCY=<n> caps the swapchain frame latency (default 2).
    // 1 makes the app pace with the display instead of racing ahead and
    // periodically hitting the full swapchain (the cause of ~every-3rd-frame
    // 30ms+ hitches on 60Hz displays).
    if let Ok(latency) = std::env::var("EASYCUE_FRAME_LATENCY") {
        if let Ok(latency) = latency.parse::<u32>() {
            wgpu_options.desired_maximum_frame_latency = Some(latency);
            log::info!(
                "[startup] EASYCUE_FRAME_LATENCY={latency} -> {:?}",
                wgpu_options.desired_maximum_frame_latency
            );
        }
    }
    
    let native_options = eframe::NativeOptions {
        viewport,
        persist_window: true,  // Save window position
        wgpu_options,
        ..Default::default()
    };
    log::info!("[startup] Native window configured in {:.2}ms", window_setup_start.elapsed().as_secs_f64() * 1000.0);

    // Run the application with persistence enabled
    log::info!("[startup] Entering eframe::run_native at {:.2}ms", process_start.elapsed().as_secs_f64() * 1000.0);
    let run_result = eframe::run_native(
        "EasyCue3",  // App ID used for storing persistent data
        native_options,
        Box::new(|cc| Ok(Box::new(EasyCueApp::new(cc)))),
    );

    match &run_result {
        Ok(()) => {
            log::info!(
                "[shutdown] eframe::run_native returned Ok after {:.2}ms",
                process_start.elapsed().as_secs_f64() * 1000.0
            );
        }
        Err(e) => {
            log::error!(
                "[shutdown] eframe::run_native returned error after {:.2}ms: {}",
                process_start.elapsed().as_secs_f64() * 1000.0,
                e
            );
        }
    }

    run_result
}

/// Load the application icon (embedded at compile time)
fn load_icon() -> Option<egui::IconData> {
    let icon_bytes = include_bytes!("../assets/logo.png");

    match image::load_from_memory(icon_bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            
            Some(egui::IconData {
                rgba: rgba.into_raw(),
                width: width as u32,
                height: height as u32,
            })
        }
        Err(e) => {
            log::warn!("Failed to decode embedded icon: {}", e);
            None
        }
    }
}

/// eframe persists UI settings (dock layout, last-opened show, window state, …)
/// as a RON file at `~/.local/share/easycue3/app.ron`. It writes that file on a
/// background thread, so a process that dies mid-write (Ctrl+C from a terminal,
/// a crash during exit) can leave it truncated. On the next launch eframe's RON
/// parser would fail and the app would silently fall back to default settings —
/// the UI resets and the last show isn't re-opened.
///
/// This runs before eframe starts, backs any unparseable file up as
/// `app.ron.corrupt-<timestamp>`, and lets the app start clean. The backup keeps
/// the previous state recoverable and makes the reset diagnosable rather than a
/// silent surprise.
fn heal_storage_if_corrupt() {
    let Some(dir) = eframe::storage_dir("EasyCue3") else {
        return;
    };
    let path = dir.join("app.ron");
    if path.exists() {
        heal_ron_file(&path);
    }
}

/// Back up `path` as `app.ron.corrupt-<timestamp>` when it isn't a valid RON
/// string map (i.e. a truncated write from a killed process). Returns true if
/// the file was backed up.
fn heal_ron_file(path: &std::path::Path) -> bool {
    if !path.exists() {
        return false;
    }
    let healthy = std::fs::read_to_string(path)
        .ok()
        .map(|text| ron::from_str::<std::collections::HashMap<String, String>>(&text).is_ok())
        .unwrap_or(false);
    if healthy {
        return false;
    }

    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let backup = path.with_file_name(format!("app.ron.corrupt-{}", stamp));
    match std::fs::rename(path, &backup) {
        Ok(()) => {
            log::warn!(
                "Backed up corrupt settings file {:?} -> {:?}; starting with defaults",
                path,
                backup
            );
            true
        }
        Err(e) => {
            log::warn!(
                "Settings file {:?} is corrupt and could not be backed up: {}",
                path,
                e
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::heal_ron_file;

    #[test]
    fn healthy_ron_file_is_left_alone() {
        let dir = std::env::temp_dir().join(format!("easycue3_heal_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.ron");
        std::fs::write(
            &path,
            r#"{"last_file": "/tmp/show.json", "dock_state": "(ok:true)}"}"#,
        )
        .unwrap();
        assert!(!heal_ron_file(&path));
        assert!(path.exists());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            r#"{"last_file": "/tmp/show.json", "dock_state": "(ok:true)}"}"#
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn truncated_ron_file_is_backed_up() {
        let dir = std::env::temp_dir().join(format!("easycue3_heal_test2_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.ron");
        // Simulate a write killed mid-stream: the file is cut off mid-value.
        std::fs::write(&path, r#"{"last_file": "/tmp/show.json", "dock_state": "(ok:tru"#).unwrap();
        assert!(heal_ron_file(&path));
        assert!(!path.exists());
        let backups = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("app.ron.corrupt-"))
            .count();
        assert_eq!(backups, 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}
