//! EasyCue3 - Theatrical Lighting & Media Console
//!
//! A simple lighting console for small-scale theatre and schools,
//! combining ETC EOS-style lighting control with QLab-style media playback.

mod app;
mod groups;
mod magic_sheet;
mod media;
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
    
    let native_options = eframe::NativeOptions {
        viewport,
        persist_window: true,  // Save window position
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
