//! Sound cue list panel

use egui::Ui;
use crate::app::EasyCueApp;

/// Render the sound cue list panel
pub fn render_sound_cues_panel(ui: &mut Ui, _app: &mut EasyCueApp) {
    ui.vertical_centered(|ui| {
        ui.add_space(30.0);
        ui.label(egui::RichText::new("🔊 Sound Cues").size(24.0));
        ui.add_space(10.0);
        ui.label(egui::RichText::new("Coming in Phase 5").color(egui::Color32::GRAY));
        ui.add_space(20.0);
        
        ui.label("Sound cue features will include:");
        ui.add_space(6.0);
        ui.label("• Audio file playback (WAV, MP3, etc.)");
        ui.label("• Fade in/out controls");
        ui.label("• Volume and pan adjustment");
        ui.label("• Sound effects triggering");
        ui.label("• Multi-track audio support");
    });
    
    ui.add_space(20.0);
    ui.separator();
    
    // Placeholder controls
    ui.horizontal(|ui| {
        ui.add_enabled(false, egui::Button::new("Add Sound Cue"));
        ui.add_enabled(false, egui::Button::new("Import Audio"));
    });
}
