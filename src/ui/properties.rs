//! Properties panel - shows details of selected channel or cue

use egui::Ui;
use crate::app::EasyCueApp;

/// Render the properties panel
pub fn render_properties_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    let has_channels = !app.ui_state.selected_channels.is_empty();
    let has_fixtures = !app.ui_state.selected_fixtures.is_empty();
    let has_cue     = app.ui_state.selected_cue_id.is_some();

    // Cue properties take precedence over fixture/channel properties
    if has_cue {
        if let Some(sel_id) = app.ui_state.selected_cue_id {
            let cue = app.cue_list.find_by_id(sel_id).cloned();
            if let Some(cue) = cue {
                let abs_idx = app.cue_list.cues().iter().position(|c| c.id == sel_id);
                if cue.is_lighting() {
                    render_lighting_cue_properties(ui, app, &cue, abs_idx);
                } else {
                    #[cfg(feature = "audio")]
                    render_audio_cue_properties(ui, app, &cue, abs_idx);
                    #[cfg(not(feature = "audio"))]
                    ui.label("(audio feature not enabled)");
                }
            }
        }
        if !has_channels && !has_fixtures {
            return;
        }
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);
    }

    if !has_channels && !has_fixtures {
        if !has_cue {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(egui::RichText::new("No Selection").color(egui::Color32::GRAY));
                ui.add_space(10.0);
                ui.label("Select a cue or channel to view properties");
            });
        }
        return;
    }

    // Fixture / channel properties (below cue section if cue also selected)
    if has_fixtures {
        if app.ui_state.selected_fixtures.len() == 1 {
            let fixture_id = *app.ui_state.selected_fixtures.iter().next().unwrap();
            render_selected_fixture_properties(ui, app, fixture_id);
        } else {
            render_multi_fixture_properties(ui, app);
        }
    } else if has_channels {
        if app.ui_state.selected_channels.len() == 1 {
            let channel = *app.ui_state.selected_channels.iter().next().unwrap();
            render_single_channel_properties(ui, app, channel);
        } else {
            render_multi_channel_properties(ui, app);
        }
    }
}

// ── Cue properties ────────────────────────────────────────────────────────────

fn render_lighting_cue_properties(ui: &mut Ui, app: &mut EasyCueApp, cue: &crate::cue::Cue, abs_idx: Option<usize>) {
    ui.label(egui::RichText::new(format!("💡 Cue {:.1}", cue.number)).strong());

    let Some(idx) = abs_idx else { return };

    egui::Grid::new("lx_cue_props")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            // Label
            ui.label("Label:");
            let mut label = cue.label.clone();
            if ui.add(egui::TextEdit::singleline(&mut label).desired_width(160.0)).changed() {
                if let Some(c) = app.cue_list.get_cue_mut(idx) { c.label = label; }
            }
            ui.end_row();

            // Fade times
            let (fade_up, fade_down) = cue.lighting_data()
                .map(|d| (d.fade_up, d.fade_down))
                .unwrap_or((0.0, 0.0));

            ui.label("Fade ↑:");
            let mut fu = fade_up;
            if ui.add(egui::DragValue::new(&mut fu).speed(0.1).range(0.0..=30.0).suffix("s")).changed() {
                if let Some(c) = app.cue_list.get_cue_mut(idx) {
                    if let Some(d) = c.lighting_data_mut() { d.fade_up = fu; }
                }
            }
            ui.end_row();

            ui.label("Fade ↓:");
            let mut fd = fade_down;
            if ui.add(egui::DragValue::new(&mut fd).speed(0.1).range(0.0..=30.0).suffix("s")).changed() {
                if let Some(c) = app.cue_list.get_cue_mut(idx) {
                    if let Some(d) = c.lighting_data_mut() { d.fade_down = fd; }
                }
            }
            ui.end_row();

            // Channel count
            let ch_count = cue.lighting_data().map(|d| d.channel_values.len()).unwrap_or(0);
            ui.label("Channels:");
            ui.label(ch_count.to_string());
            ui.end_row();
        });

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button("Load to Stage").on_hover_text("Push cue values to live output").clicked() {
            let values: Vec<(u16, u8)> = cue.lighting_data()
                .map(|d| d.channel_values.iter().map(|(&k, &v)| (k, v)).collect())
                .unwrap_or_default();
            if let Some(universe) = app.universes.first_mut() {
                for (ch, val) in values {
                    let _ = universe.set_channel(ch, val);
                }
            }
            app.ui_state.status_message = format!("Loaded cue {:.1} to stage", cue.number);
        }

        if ui.button("Capture Stage").on_hover_text("Overwrite cue with current live levels").clicked() {
            let channel_values: Vec<(u16, u8)> = if let Some(universe) = app.universes.first() {
                (1u16..=512)
                    .filter_map(|ch| universe.get_channel(ch).ok().filter(|&v| v > 0).map(|v| (ch, v)))
                    .collect()
            } else { vec![] };
            if let Some(c) = app.cue_list.get_cue_mut(idx) {
                if let Some(d) = c.lighting_data_mut() {
                    d.channel_values.clear();
                    for (ch, val) in channel_values {
                        d.set_channel(ch, val);
                    }
                }
            }
            app.ui_state.status_message = format!("Captured stage to cue {:.1}", cue.number);
        }
    });

    // Non-zero channel values (compact list)
    if let Some(data) = cue.lighting_data() {
        if !data.channel_values.is_empty() {
            ui.add_space(6.0);
            ui.collapsing(format!("Channel Values ({})", data.channel_values.len()), |ui| {
                let mut pairs: Vec<(u16, u8)> = data.channel_values.iter().map(|(&k, &v)| (k, v)).collect();
                pairs.sort_by_key(|(ch, _)| *ch);
                egui::Grid::new("lx_cue_ch").num_columns(4).spacing([6.0, 2.0]).show(ui, |ui| {
                    for (i, (ch, val)) in pairs.iter().enumerate() {
                        ui.label(format!("{}: {}", ch, val));
                        if (i + 1) % 4 == 0 { ui.end_row(); }
                    }
                });
            });
        }
    }
}

#[cfg(feature = "audio")]
fn render_audio_cue_properties(ui: &mut Ui, app: &mut EasyCueApp, cue: &crate::cue::Cue, abs_idx: Option<usize>) {
    ui.label(egui::RichText::new(format!("🔊 Cue {:.1}", cue.number)).strong());

    let Some(idx) = abs_idx else { return };

    let (path, volume, fade_in, fade_out) = cue.audio_data()
        .map(|d| (d.audio_path.clone(), d.volume, d.fade_in, d.fade_out))
        .unwrap_or_default();

    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("(none)").to_string();
    let resolved = crate::cue::AudioData::new(path.clone()).resolved_path();
    let file_ok = resolved.exists();

    egui::Grid::new("audio_cue_props")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Label:");
            let mut label = cue.label.clone();
            if ui.add(egui::TextEdit::singleline(&mut label).desired_width(160.0)).changed() {
                if let Some(c) = app.cue_list.get_cue_mut(idx) { c.label = label; }
            }
            ui.end_row();

            ui.label("File:");
            ui.horizontal(|ui| {
                let file_color = if file_ok { ui.style().visuals.text_color() } else { egui::Color32::RED };
                ui.label(egui::RichText::new(&filename).color(file_color));
                if ui.small_button("…").on_hover_text("Choose different file").clicked() {
                    if let Some(new_path) = rfd::FileDialog::new()
                        .add_filter("Audio", &["mp3","wav","flac","ogg","aac","m4a"])
                        .pick_file()
                    {
                        if let Some(c) = app.cue_list.get_cue_mut(idx) {
                            if let Some(d) = c.audio_data_mut() {
                                d.set_path(new_path);
                            }
                        }
                        app.ui_state.audio_file_cache.clear();
                    }
                }
            });
            ui.end_row();

            ui.label("Volume:");
            let mut vol_pct = (volume * 100.0) as i32;
            if ui.add(egui::DragValue::new(&mut vol_pct).speed(1.0).range(0..=100).suffix("%")).changed() {
                if let Some(c) = app.cue_list.get_cue_mut(idx) {
                    if let Some(d) = c.audio_data_mut() { d.volume = vol_pct as f32 / 100.0; }
                }
            }
            ui.end_row();

            ui.label("Fade In:");
            let mut fi = fade_in;
            if ui.add(egui::DragValue::new(&mut fi).speed(0.1).range(0.0..=30.0).suffix("s")).changed() {
                if let Some(c) = app.cue_list.get_cue_mut(idx) {
                    if let Some(d) = c.audio_data_mut() { d.fade_in = fi; }
                }
            }
            ui.end_row();

            ui.label("Fade Out:");
            let mut fo = fade_out;
            if ui.add(egui::DragValue::new(&mut fo).speed(0.1).range(0.0..=30.0).suffix("s")).changed() {
                if let Some(c) = app.cue_list.get_cue_mut(idx) {
                    if let Some(d) = c.audio_data_mut() { d.fade_out = fo; }
                }
            }
            ui.end_row();
        });
}

/// Render properties for a single selected channel
fn render_single_channel_properties(ui: &mut Ui, app: &mut EasyCueApp, channel: u16) {
    // Check if this channel is part of a patched fixture
    // Collect fixture data to avoid borrow conflicts
    let fixture_data: Option<(crate::fixtures::Patch, crate::fixtures::FixtureProfile)> = {
        let channel_counts = app.fixtures.get_channel_counts();
        app.fixtures
            .patch_list()
            .find_patch_at_channel(channel, &channel_counts)
            .and_then(|patch| {
                app.fixtures
                    .get_profile(&patch.profile_id)
                    .map(|profile| (patch.clone(), profile.clone()))
            })
    };
    
    if let Some((patch, profile)) = fixture_data {
        // Channel is part of a fixture - show fixture properties
        render_fixture_properties(ui, app, &patch, &profile, channel);
        return;
    }
    
    // Fall back to raw channel display if not patched
    ui.label(egui::RichText::new(format!("Channel {}", channel)).strong());
    ui.label(egui::RichText::new("(Unpatched)").small().italics());
    
    if let Some(universe) = app.universes.first_mut() {
        let mut value = universe.get_channel(channel).unwrap_or(0);
        
        ui.add_space(6.0);
        
        egui::Grid::new("channel_props")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label("Value:");
                if ui.add(egui::DragValue::new(&mut value).range(0..=100)).changed() {
                    let _ = universe.set_channel(channel, value);
                    // Update base level when manually changed
                    app.ui_state.channel_base_levels.insert(channel, value);
                    app.ui_state.group_master = value;
                }
                ui.end_row();
            });
    }
}

/// Render fixture properties with parameter controls
fn render_fixture_properties(
    ui: &mut Ui,
    app: &mut EasyCueApp,
    patch: &crate::fixtures::Patch,
    profile: &crate::fixtures::FixtureProfile,
    _selected_channel: u16,
) {
    use crate::fixtures::profiles::FixtureParameter;
    
    ui.label(egui::RichText::new(&patch.label).strong());
    ui.label(egui::RichText::new(&profile.name).small().italics());
    
    ui.add_space(10.0);
    
    let Some(universe) = app.universes.first_mut() else {
        return;
    };
    
    // Intensity parameter (if present)
    if let Some(offset) = profile.get_parameter_offset(&FixtureParameter::Intensity) {
        let ch = patch.start_address + offset;
        let mut value = universe.get_channel(ch).unwrap_or(0);
        
        ui.label(egui::RichText::new("Intensity").strong());
        if ui.add(egui::Slider::new(&mut value, 0..=100)).changed() {
            let _ = universe.set_channel(ch, value);
        }
        ui.add_space(8.0);
    } else if profile.is_rgb() {
        // RGB fixture without dedicated intensity channel - use virtual intensity
        ui.label(egui::RichText::new("Virtual Intensity").strong());
        
        // Get current virtual intensity
        let current_intensity = app.virtual_intensity.get_intensity(patch.id)
            .unwrap_or_else(|| {
                app.virtual_intensity.calculate_intensity(patch.id, universe, &patch, &profile)
            });
        
        let mut intensity = current_intensity;
        if ui.add(egui::Slider::new(&mut intensity, 0.0..=1.0)
            .text("%")
            .custom_formatter(|val, _| format!("{:.0}%", val * 100.0))
        ).changed() {
            // Apply virtual intensity
            let patch_clone = patch.clone();
            let profile_clone = profile.clone();
            if let Err(e) = app.virtual_intensity.set_intensity(
                patch.id,
                intensity,
                universe,
                &patch_clone,
                &profile_clone,
            ) {
                log::error!("Failed to set virtual intensity: {}", e);
            }
        }
        
        ui.label(egui::RichText::new("Color-preserving intensity control").small().italics());
        ui.add_space(8.0);
    }
    
    // Color picker for RGB fixtures
    if profile.is_rgb() {
        ui.label(egui::RichText::new("Color").strong());
        
        // Get current RGB values
        let r_offset = profile.get_parameter_offset(&FixtureParameter::Red).unwrap();
        let g_offset = profile.get_parameter_offset(&FixtureParameter::Green).unwrap();
        let b_offset = profile.get_parameter_offset(&FixtureParameter::Blue).unwrap();
        
        let r_ch = patch.start_address + r_offset;
        let g_ch = patch.start_address + g_offset;
        let b_ch = patch.start_address + b_offset;
        
        let r = universe.get_channel(r_ch).unwrap_or(0);
        let g = universe.get_channel(g_ch).unwrap_or(0);
        let b = universe.get_channel(b_ch).unwrap_or(0);
        
        // Convert to egui Color32 (0-100 -> 0-255 range)
        let mut color = egui::Color32::from_rgb(
            ((r as f32 / 100.0) * 255.0) as u8,
            ((g as f32 / 100.0) * 255.0) as u8,
            ((b as f32 / 100.0) * 255.0) as u8,
        );
        
        if ui.color_edit_button_srgba(&mut color).changed() {
            // Convert back to 0-100 range
            let new_r = ((color.r() as f32 / 255.0) * 100.0) as u8;
            let new_g = ((color.g() as f32 / 255.0) * 100.0) as u8;
            let new_b = ((color.b() as f32 / 255.0) * 100.0) as u8;
            
            // Update RGB channels in universe
            let _ = universe.set_channel(r_ch, new_r);
            let _ = universe.set_channel(g_ch, new_g);
            let _ = universe.set_channel(b_ch, new_b);
            
            // For fixtures without dedicated intensity, update ALL color ratios
            // (not just RGB) to preserve other color channels like Amber, White, UV
            if !profile.has_intensity() {
                let mut color_values = std::collections::HashMap::new();
                color_values.insert(FixtureParameter::Red, new_r);
                color_values.insert(FixtureParameter::Green, new_g);
                color_values.insert(FixtureParameter::Blue, new_b);
                
                // Read other color channels from universe to preserve them
                for param_mapping in profile.color_parameters() {
                    if !matches!(param_mapping.parameter, FixtureParameter::Red | FixtureParameter::Green | FixtureParameter::Blue) {
                        let ch = patch.start_address + param_mapping.channel_offset;
                        if let Ok(value) = universe.get_channel(ch) {
                            color_values.insert(param_mapping.parameter.clone(), value);
                        }
                    }
                }
                
                app.virtual_intensity.set_color(patch.id, color_values);
            }
        }
        
        ui.add_space(8.0);
        
        // Individual color sliders
        ui.collapsing("Color Channels", |ui| {
            egui::Grid::new("rgb_sliders")
                .num_columns(2)
                .spacing([10.0, 6.0])
                .show(ui, |ui| {
                    let mut r_val = r;
                    ui.label("Red:");
                    if ui.add(egui::Slider::new(&mut r_val, 0..=100)).changed() {
                        let _ = universe.set_channel(r_ch, r_val);
                        // Update virtual intensity state if applicable
                        if !profile.has_intensity() {
                            let patch_clone = patch.clone();
                            let profile_clone = profile.clone();
                            app.virtual_intensity.update_from_universe(patch.id, universe, &patch_clone, &profile_clone);
                        }
                    }
                    ui.end_row();
                    
                    let mut g_val = g;
                    ui.label("Green:");
                    if ui.add(egui::Slider::new(&mut g_val, 0..=100)).changed() {
                        let _ = universe.set_channel(g_ch, g_val);
                        // Update virtual intensity state if applicable
                        if !profile.has_intensity() {
                            let patch_clone = patch.clone();
                            let profile_clone = profile.clone();
                            app.virtual_intensity.update_from_universe(patch.id, universe, &patch_clone, &profile_clone);
                        }
                    }
                    ui.end_row();
                    
                    let mut b_val = b;
                    ui.label("Blue:");
                    if ui.add(egui::Slider::new(&mut b_val, 0..=100)).changed() {
                        let _ = universe.set_channel(b_ch, b_val);
                        // Update virtual intensity state if applicable
                        if !profile.has_intensity() {
                            let patch_clone = patch.clone();
                            let profile_clone = profile.clone();
                            app.virtual_intensity.update_from_universe(patch.id, universe, &patch_clone, &profile_clone);
                        }
                    }
                    ui.end_row();
                    
                    // Additional color channels (if present)
                    if let Some(offset) = profile.get_parameter_offset(&FixtureParameter::Amber) {
                        let ch = patch.start_address + offset;
                        let mut val = universe.get_channel(ch).unwrap_or(0);
                        ui.label("Amber:");
                        if ui.add(egui::Slider::new(&mut val, 0..=100)).changed() {
                            let _ = universe.set_channel(ch, val);
                            // Update virtual intensity state
                            let patch_clone = patch.clone();
                            let profile_clone = profile.clone();
                            app.virtual_intensity.update_from_universe(patch.id, universe, &patch_clone, &profile_clone);
                        }
                        ui.end_row();
                    }
                    
                    if let Some(offset) = profile.get_parameter_offset(&FixtureParameter::White) {
                        let ch = patch.start_address + offset;
                        let mut val = universe.get_channel(ch).unwrap_or(0);
                        ui.label("White:");
                        if ui.add(egui::Slider::new(&mut val, 0..=100)).changed() {
                            let _ = universe.set_channel(ch, val);
                            // Update virtual intensity state
                            let patch_clone = patch.clone();
                            let profile_clone = profile.clone();
                            app.virtual_intensity.update_from_universe(patch.id, universe, &patch_clone, &profile_clone);
                        }
                        ui.end_row();
                    }
                    
                    if let Some(offset) = profile.get_parameter_offset(&FixtureParameter::Uv) {
                        let ch = patch.start_address + offset;
                        let mut val = universe.get_channel(ch).unwrap_or(0);
                        ui.label("UV:");
                        if ui.add(egui::Slider::new(&mut val, 0..=100)).changed() {
                            let _ = universe.set_channel(ch, val);
                            // Update virtual intensity state
                            let patch_clone = patch.clone();
                            let profile_clone = profile.clone();
                            app.virtual_intensity.update_from_universe(patch.id, universe, &patch_clone, &profile_clone);
                        }
                        ui.end_row();
                    }
                });
        });
    }
    
    ui.add_space(8.0);
    
    // Other parameters
    ui.collapsing("All Parameters", |ui| {
        egui::Grid::new("fixture_params")
            .num_columns(3)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label("Parameter");
                ui.label("Channel");
                ui.label("Value");
                ui.end_row();
                
                for param_map in &profile.parameters {
                    let ch = patch.start_address + param_map.channel_offset;
                    let value = universe.get_channel(ch).unwrap_or(0);
                    
                    ui.label(format!("{:?}", param_map.parameter));
                    ui.label(format!("{}", ch));
                    ui.label(format!("{}", value));
                    ui.end_row();
                }
            });
    });
    
    ui.add_space(10.0);
    ui.label(
        egui::RichText::new(format!(
            "Channels {}-{}",
            patch.start_address,
            patch.start_address + profile.channel_count - 1
        ))
        .small()
        .italics(),
    );
}

/// Render properties for multiple selected channels
fn render_multi_channel_properties(ui: &mut Ui, app: &mut EasyCueApp) {
    let channel_count = app.ui_state.selected_channels.len();
    
    // Check if all selected channels belong to the same fixture
    let fixture_data: Option<(crate::fixtures::Patch, crate::fixtures::FixtureProfile, u16)> = {
        let channel_counts = app.fixtures.get_channel_counts();
        let patch_ids: Vec<_> = app
            .ui_state
            .selected_channels
            .iter()
            .filter_map(|&ch| {
                app.fixtures
                    .patch_list()
                    .find_patch_at_channel(ch, &channel_counts)
                    .map(|p| p.id)
            })
            .collect();
        
        // If all channels belong to the same fixture, collect the data
        if !patch_ids.is_empty()
            && patch_ids.len() == channel_count
            && patch_ids.iter().all(|&id| id == patch_ids[0])
        {
            let fixture_id = patch_ids[0];
            let first_channel = *app.ui_state.selected_channels.iter().next().unwrap();
            
            // Collect patch and profile data
            app.fixtures
                .patch_list()
                .patches()
                .iter()
                .find(|p| p.id == fixture_id)
                .and_then(|patch| {
                    app.fixtures
                        .get_profile(&patch.profile_id)
                        .map(|profile| (patch.clone(), profile.clone(), first_channel))
                })
        } else {
            None
        }
    };
    
    // If we found fixture data, render fixture properties
    if let Some((patch, profile, first_channel)) = fixture_data {
        render_fixture_properties(ui, app, &patch, &profile, first_channel);
        return;
    }
    
    // Fall back to multi-channel display for mixed/unpatched channels
    ui.label(egui::RichText::new(format!("{} Channels Selected", channel_count)).strong());
    
    if let Some(universe) = app.universes.first_mut() {
        // Get all selected channel values
        let mut channel_values: Vec<(u16, u8)> = app.ui_state.selected_channels
            .iter()
            .map(|&ch| (ch, universe.get_channel(ch).unwrap_or(0)))
            .collect();
        channel_values.sort_by_key(|(ch, _)| *ch);
        
        // Calculate statistics
        let max_value = channel_values.iter().map(|(_, v)| *v).max().unwrap_or(0);
        let min_value = channel_values.iter().map(|(_, v)| *v).min().unwrap_or(0);
        let avg_value = if !channel_values.is_empty() {
            channel_values.iter().map(|(_, v)| *v as u32).sum::<u32>() / channel_values.len() as u32
        } else {
            0
        } as u8;
        
        ui.add_space(6.0);
        
        egui::Grid::new("multi_channel_props")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label("Channels:");
                ui.label(format!("{}", channel_count));
                ui.end_row();
                
                ui.label("Range:");
                ui.label(format!("{}-{}", min_value, max_value));
                ui.end_row();
                
                ui.label("Average:");
                ui.label(format!("{}", avg_value));
                ui.end_row();
            });
        
        ui.add_space(10.0);
        
        // Master slider for proportional control using O_i = M * L_i formula
        ui.label(egui::RichText::new("Group Master (Proportional)").strong());
        ui.add_space(4.0);
        
        let mut master_value = app.ui_state.group_master;
        let slider_changed = ui.add(
            egui::Slider::new(&mut master_value, 0..=100)
                .text("M")
        ).changed();
        
        if slider_changed {
            app.ui_state.group_master = master_value;
            
            // Find the max base level for normalization
            let max_base = app.ui_state.channel_base_levels.values().copied().max().unwrap_or(100);
            
            if max_base > 0 {
                // Apply O_i = M * (L_i / L_max) formula to all selected channels
                for &ch in &app.ui_state.selected_channels {
                    if let Some(&base_level) = app.ui_state.channel_base_levels.get(&ch) {
                        // O_i = M * (L_i / L_max)
                        let output = ((master_value as f32) * (base_level as f32) / (max_base as f32)).round() as u8;
                        let _ = universe.set_channel(ch, output.min(100));
                    }
                }
            } else {
                // All base levels are 0, set all to master value
                for &ch in &app.ui_state.selected_channels {
                    let _ = universe.set_channel(ch, master_value);
                }
            }
        }
        
        // Show base levels for reference
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Base Levels (L_i):").small().italics());
        ui.horizontal_wrapped(|ui| {
            let mut sorted_channels: Vec<u16> = app.ui_state.selected_channels.iter().copied().collect();
            sorted_channels.sort();
            for &ch in &sorted_channels {
                if let Some(&base) = app.ui_state.channel_base_levels.get(&ch) {
                    ui.label(format!("{}:{}", ch, base));
                }
            }
        });
    }
}

/// Render properties for a single selected fixture
fn render_selected_fixture_properties(ui: &mut Ui, app: &mut EasyCueApp, fixture_id: usize) {
    // Get the patch and profile for this fixture
    let Some(patch) = app.fixtures.patch_list().get_patch(fixture_id) else {
        ui.label("Fixture not found");
        return;
    };
    let patch = patch.clone();
    
    let Some(profile) = app.fixtures.get_profile(&patch.profile_id) else {
        ui.label(format!("Profile '{}' not found", patch.profile_id));
        return;
    };
    let profile = profile.clone();
    
    // Render full fixture properties
    render_fixture_properties(ui, app, &patch, &profile, patch.start_address);
}

/// Render properties for multiple selected fixtures
fn render_multi_fixture_properties(ui: &mut Ui, app: &mut EasyCueApp) {
    let fixture_count = app.ui_state.selected_fixtures.len();
    
    ui.label(egui::RichText::new(format!("{} Fixtures Selected", fixture_count)).strong());
    
    ui.add_space(10.0);
    
    // Show list of selected fixtures
    ui.collapsing("Selected Fixtures", |ui| {
        let mut sorted_fixtures: Vec<usize> = app.ui_state.selected_fixtures.iter().copied().collect();
        sorted_fixtures.sort();
        
        for fixture_id in sorted_fixtures {
            if let Some(patch) = app.fixtures.patch_list().get_patch(fixture_id) {
                if let Some(profile) = app.fixtures.get_profile(&patch.profile_id) {
                    ui.label(format!("[#{}] {} ({})", fixture_id, patch.label, profile.name));
                }
            }
        }
    });
    
    ui.add_space(10.0);
    ui.label(egui::RichText::new("Tip: Select a single fixture to edit properties").small().italics());
}
