//! EasyCue3 - Theatrical Lighting & Media Console
//!
//! A simple lighting console for small-scale theatre and schools,
//! combining ETC EOS-style lighting control with QLab-style media playback.

mod app;
mod dmx;
mod media;
mod cue;
mod ui;
mod fixtures;
mod show;

use app::EasyCueApp;

fn main() -> eframe::Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

    log::info!("Starting EasyCue3...");

    // Configure the native window
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("EasyCue3 - Theatrical Lighting Console")
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    // Run the application
    eframe::run_native(
        "EasyCue3",
        native_options,
        Box::new(|cc| Ok(Box::new(EasyCueApp::new(cc)))),
    )
}
