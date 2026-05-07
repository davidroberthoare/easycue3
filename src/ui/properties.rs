//! Properties panels — cue properties and instrument properties

use egui::Ui;
use crate::app::EasyCueApp;
use egui_phosphor::regular as ph;

/// Render cue properties for the selected cue.
pub fn render_cue_properties_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    if let Some(sel_id) = app.ui_state.selected_cue_id {
        let cue = app.cue_list.find_by_id(sel_id).cloned();
        if let Some(cue) = cue {
            let abs_idx = app.cue_list.cues().iter().position(|c| c.id == sel_id);
            if cue.is_lighting() {
                render_lighting_cue_properties(ui, app, &cue, abs_idx);
            } else {
                #[cfg(feature = "audio")]
                {
                    if cue.is_adjust() {
                        render_adjust_cue_properties(ui, app, &cue, abs_idx);
                    } else {
                        render_audio_cue_properties(ui, app, &cue, abs_idx);
                    }
                }
                #[cfg(not(feature = "audio"))]
                ui.label("(audio feature not enabled)");
            }
            return;
        }
    }
    ui.vertical_centered(|ui| {
        ui.add_space(20.0);
        ui.label(egui::RichText::new("No Cue Selected").color(egui::Color32::GRAY));
        ui.add_space(10.0);
        ui.label("Select a cue to view its properties");
    });
}

/// Render instrument/channel properties for the current selection.
pub fn render_instrument_properties_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    let has_channels = !app.ui_state.selected_channels.is_empty();
    let has_fixtures = !app.ui_state.selected_fixtures.is_empty();

    if has_fixtures {
        if app.ui_state.selected_fixtures.len() == 1 {
            let fixture_id = *app.ui_state.selected_fixtures.iter().next().unwrap();
            render_selected_fixture_properties(ui, app, fixture_id);
        } else {
            render_multi_fixture_properties(ui, app);
        }
    } else if has_channels {
        egui::ScrollArea::vertical().show(ui, |ui| {
            if app.ui_state.selected_channels.len() == 1 {
                let channel = *app.ui_state.selected_channels.iter().next().unwrap();
                render_single_channel_properties(ui, app, channel);
            } else {
                render_multi_channel_properties(ui, app);
            }
        });
    } else {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(egui::RichText::new("No Selection").color(egui::Color32::GRAY));
            ui.add_space(10.0);
            ui.label("Select a channel or fixture to view properties");
        });
    }
}

// ── Cue properties ────────────────────────────────────────────────────────────

/// Render the editable cue-number row inside a 2-column grid.
/// Returns Some(new_number) if the user committed a valid change, None otherwise.
fn cue_number_row(ui: &mut egui::Ui, cue: &crate::cue::Cue, cue_list: &crate::cue::CueList) -> Option<f32> {
    ui.label("Number:");
    let mut num = cue.number;
    let resp = ui.add(
        egui::DragValue::new(&mut num)
            .speed(0.1)
            .range(0.01..=9999.0)
            .custom_formatter(|n, _| format!("{:.1}", n))
            .custom_parser(|s| s.parse::<f64>().ok()),
    );
    if resp.changed() && (num - cue.number).abs() > 0.001 {
        let duplicate = cue_list.cues().iter().any(|c| c.id != cue.id && (c.number - num).abs() < 0.005);
        if !duplicate {
            return Some(num);
        }
    }
    None
}

fn render_lighting_cue_properties(ui: &mut Ui, app: &mut EasyCueApp, cue: &crate::cue::Cue, abs_idx: Option<usize>) {
    ui.label(egui::RichText::new(format!("{} Cue {:.1}", ph::LIGHTBULB, cue.number)).strong());

    let Some(idx) = abs_idx else { return };

    egui::Grid::new("lx_cue_props")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            // Number (editable)
            if let Some(new_num) = cue_number_row(ui, cue, &app.cue_list) {
                let _ = app.cue_list.renumber_cue(cue.id, new_num);
            }
            ui.end_row();

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

            // ui.label("Fade ↑:");
            ui.label(format!("Fade {}:", ph::ARROW_UP));
            let mut fu = fade_up;
            if ui.add(egui::DragValue::new(&mut fu).speed(0.1).range(0.0..=30.0).suffix("s")).changed() {
                if let Some(c) = app.cue_list.get_cue_mut(idx) {
                    if let Some(d) = c.lighting_data_mut() { d.fade_up = fu; }
                }
            }
            ui.end_row();

            ui.label(format!("Fade {}:", ph::ARROW_DOWN));
            let mut fd = fade_down;
            if ui.add(egui::DragValue::new(&mut fd).speed(0.1).range(0.0..=30.0).suffix("s")).changed() {
                if let Some(c) = app.cue_list.get_cue_mut(idx) {
                    if let Some(d) = c.lighting_data_mut() { d.fade_down = fd; }
                }
            }
            ui.end_row();

            // Auto-follow
            ui.label("Auto-follow:");
            let mut af_enabled = cue.autofollow.is_some();
            let mut af_delay = cue.autofollow.unwrap_or(2.0_f32).max(0.1);
            ui.horizontal(|ui| {
                if ui.checkbox(&mut af_enabled, "").changed() {
                    if let Some(c) = app.cue_list.get_cue_mut(idx) {
                        c.autofollow = if af_enabled { Some(af_delay) } else { None };
                    }
                }
                if af_enabled {
                    if ui.add(egui::DragValue::new(&mut af_delay).speed(0.1).range(0.1..=300.0).suffix("s")).changed() {
                        if let Some(c) = app.cue_list.get_cue_mut(idx) {
                            c.autofollow = Some(af_delay);
                        }
                    }
                } else {
                    ui.label(egui::RichText::new("off").color(egui::Color32::GRAY));
                }
            });
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

        if ui.button("Update From Stage").on_hover_text("Overwrite cue with current live levels").clicked() {
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
    ui.label(egui::RichText::new(format!("{} Cue {:.1}", ph::SPEAKER_HIGH, cue.number)).strong());

    let Some(idx) = abs_idx else { return };

    let (path, volume, fade_in, fade_out, length) = cue.audio_data()
        .map(|d| (d.audio_path.clone(), d.volume, d.fade_in, d.fade_out, d.length))
        .unwrap_or_default();

    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("(none)").to_string();
    let resolved = crate::cue::AudioData::new(path.clone()).resolved_path();
    let file_ok = resolved.exists();

    egui::Grid::new("audio_cue_props")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            if let Some(new_num) = cue_number_row(ui, cue, &app.cue_list) {
                let _ = app.cue_list.renumber_cue(cue.id, new_num);
            }
            ui.end_row();

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

            // Length (optional auto-stop timer)
            ui.label("Length:");
            let mut len_enabled = length.is_some();
            let mut len_val = length.unwrap_or(10.0_f32).max(0.1);
            ui.horizontal(|ui| {
                if ui.checkbox(&mut len_enabled, "").changed() {
                    if let Some(c) = app.cue_list.get_cue_mut(idx) {
                        if let Some(d) = c.audio_data_mut() {
                            d.length = if len_enabled { Some(len_val) } else { None };
                        }
                    }
                }
                if len_enabled {
                    if ui.add(egui::DragValue::new(&mut len_val).speed(0.5).range(0.1..=3600.0).suffix("s")).changed() {
                        if let Some(c) = app.cue_list.get_cue_mut(idx) {
                            if let Some(d) = c.audio_data_mut() { d.length = Some(len_val); }
                        }
                    }
                } else {
                    ui.label(egui::RichText::new("file end").color(egui::Color32::GRAY));
                }
            });
            ui.end_row();

            // Auto-follow
            ui.label("Auto-follow:");
            let mut af_enabled = cue.autofollow.is_some();
            let mut af_delay = cue.autofollow.unwrap_or(2.0_f32).max(0.1);
            ui.horizontal(|ui| {
                if ui.checkbox(&mut af_enabled, "").changed() {
                    if let Some(c) = app.cue_list.get_cue_mut(idx) {
                        c.autofollow = if af_enabled { Some(af_delay) } else { None };
                    }
                }
                if af_enabled {
                    if ui.add(egui::DragValue::new(&mut af_delay).speed(0.1).range(0.1..=300.0).suffix("s")).changed() {
                        if let Some(c) = app.cue_list.get_cue_mut(idx) {
                            c.autofollow = Some(af_delay);
                        }
                    }
                } else {
                    ui.label(egui::RichText::new("off").color(egui::Color32::GRAY));
                }
            });
            ui.end_row();
        });
}

/// Render properties for an Adjust cue (sound master ramp + optional stop)
#[cfg(feature = "audio")]
fn render_adjust_cue_properties(ui: &mut Ui, app: &mut EasyCueApp, cue: &crate::cue::Cue, abs_idx: Option<usize>) {
    ui.label(egui::RichText::new(format!("{} Cue {:.1}", ph::SLIDERS, cue.number)).strong());

    let Some(idx) = abs_idx else { return };

    let (target_audio_cue, volume, fade_time, stop_when_complete) = cue.adjust_data()
        .map(|d| (d.target_audio_cue, d.volume, d.fade_time, d.stop_when_complete))
        .unwrap_or((None, 0.8, 2.0, false));

    egui::Grid::new("adjust_cue_props")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            if let Some(new_num) = cue_number_row(ui, cue, &app.cue_list) {
                let _ = app.cue_list.renumber_cue(cue.id, new_num);
            }
            ui.end_row();

            ui.label("Label:");
            let mut label = cue.label.clone();
            if ui.add(egui::TextEdit::singleline(&mut label).desired_width(160.0)).changed() {
                if let Some(c) = app.cue_list.get_cue_mut(idx) { c.label = label; }
            }
            ui.end_row();

            // Target cue: which audio cue to affect (None = global master)
            ui.label("Target Cue:");
            let target_id = ui.id().with("adjust_target_cue");
            // Only sync from storage when the field is not actively being edited,
            // so the user's in-progress typing isn't overwritten each frame.
            if !ui.memory(|m| m.has_focus(target_id)) {
                app.ui_state.adjust_target_edit = target_audio_cue
                    .map(|n| format!("{:.1}", n))
                    .unwrap_or_default();
            }
            let target_resp = ui.add(
                egui::TextEdit::singleline(&mut app.ui_state.adjust_target_edit)
                    .id(target_id)
                    .desired_width(80.0)
                    .hint_text("all (master)"),
            );
            if target_resp.lost_focus() {
                let parsed = app.ui_state.adjust_target_edit.trim().parse::<f32>().ok();
                if let Some(c) = app.cue_list.get_cue_mut(idx) {
                    if let Some(d) = c.adjust_data_mut() { d.target_audio_cue = parsed; }
                }
            }
            ui.end_row();

            ui.label("Target Vol:");
            let mut vol_pct = (volume * 100.0) as i32;
            if ui.add(egui::DragValue::new(&mut vol_pct).speed(1.0).range(0..=100).suffix("%")).changed() {
                if let Some(c) = app.cue_list.get_cue_mut(idx) {
                    if let Some(d) = c.adjust_data_mut() { d.volume = vol_pct as f32 / 100.0; }
                }
            }
            ui.end_row();

            ui.label("Fade Time:");
            let mut ft = fade_time;
            ui.horizontal(|ui| {
                if ui.add(egui::DragValue::new(&mut ft).speed(0.1).range(0.0..=60.0).suffix("s")).changed() {
                    if let Some(c) = app.cue_list.get_cue_mut(idx) {
                        if let Some(d) = c.adjust_data_mut() { d.fade_time = ft; }
                    }
                }
                if ft == 0.0 {
                    ui.label(egui::RichText::new("(instant)").color(egui::Color32::GRAY));
                }
            });
            ui.end_row();

            ui.label("Stop when done:");
            let mut stop = stop_when_complete;
            if ui.checkbox(&mut stop, "").changed() {
                if let Some(c) = app.cue_list.get_cue_mut(idx) {
                    if let Some(d) = c.adjust_data_mut() { d.stop_when_complete = stop; }
                }
            }
            ui.end_row();

            // Auto-follow
            ui.label("Auto-follow:");
            let mut af_enabled = cue.autofollow.is_some();
            let mut af_delay = cue.autofollow.unwrap_or(2.0_f32).max(0.1);
            ui.horizontal(|ui| {
                if ui.checkbox(&mut af_enabled, "").changed() {
                    if let Some(c) = app.cue_list.get_cue_mut(idx) {
                        c.autofollow = if af_enabled { Some(af_delay) } else { None };
                    }
                }
                if af_enabled {
                    if ui.add(egui::DragValue::new(&mut af_delay).speed(0.1).range(0.1..=300.0).suffix("s")).changed() {
                        if let Some(c) = app.cue_list.get_cue_mut(idx) {
                            c.autofollow = Some(af_delay);
                        }
                    }
                } else {
                    ui.label(egui::RichText::new("off").color(egui::Color32::GRAY));
                }
            });
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

/// Render fixture properties with parameter controls.
///
/// Layout: header label, then a horizontally-scrolling row of vertical sliders —
/// intensity first, colour wheel second, individual colour channels after.
fn render_fixture_properties(
    ui: &mut Ui,
    app: &mut EasyCueApp,
    patch: &crate::fixtures::Patch,
    profile: &crate::fixtures::FixtureProfile,
    _selected_channel: u16,
) {
    use crate::fixtures::profiles::FixtureParameter;

    // Single header line — no instrument-type subtitle.
    ui.label(egui::RichText::new(&patch.label).strong());

    let Some(universe) = app.universes.first_mut() else { return };

    // ── Collect channel addresses + current values (immutable reads) ──────────
    let has_int    = profile.get_parameter_offset(&FixtureParameter::Intensity).is_some();
    let int_ch     = profile.get_parameter_offset(&FixtureParameter::Intensity)
                        .map(|off| patch.start_address + off);
    let int_raw    = int_ch.and_then(|ch| universe.get_channel(ch).ok()).unwrap_or(0);
    let vi_val     = if !has_int && profile.is_rgb() {
        app.virtual_intensity.get_intensity(patch.id)
            .unwrap_or_else(|| app.virtual_intensity.calculate_intensity(patch.id, universe, patch, profile))
    } else { 1.0 };

    let is_rgb    = profile.is_rgb();
    let r_ch      = profile.get_parameter_offset(&FixtureParameter::Red  ).map(|o| patch.start_address + o);
    let g_ch      = profile.get_parameter_offset(&FixtureParameter::Green).map(|o| patch.start_address + o);
    let b_ch      = profile.get_parameter_offset(&FixtureParameter::Blue ).map(|o| patch.start_address + o);
    let amber_ch  = profile.get_parameter_offset(&FixtureParameter::Amber).map(|o| patch.start_address + o);
    let white_ch  = profile.get_parameter_offset(&FixtureParameter::White).map(|o| patch.start_address + o);
    let uv_ch     = profile.get_parameter_offset(&FixtureParameter::Uv   ).map(|o| patch.start_address + o);

    let r     = r_ch    .and_then(|ch| universe.get_channel(ch).ok()).unwrap_or(0);
    let g     = g_ch    .and_then(|ch| universe.get_channel(ch).ok()).unwrap_or(0);
    let b     = b_ch    .and_then(|ch| universe.get_channel(ch).ok()).unwrap_or(0);
    let amber = amber_ch.and_then(|ch| universe.get_channel(ch).ok()).unwrap_or(0);
    let white = white_ch.and_then(|ch| universe.get_channel(ch).ok()).unwrap_or(0);
    let uv    = uv_ch   .and_then(|ch| universe.get_channel(ch).ok()).unwrap_or(0);

    // Extra channels: any parameter not already handled above.
    // Intensity is always shown; colour parameters are shown in the colour section
    // when is_rgb — otherwise they fall through here so nothing is silently dropped.
    let extra_channels: Vec<(String, u16, u8)> = profile.parameters.iter()
        .filter(|pm| {
            if matches!(pm.parameter, FixtureParameter::Intensity) { return false; }
            if is_rgb && pm.parameter.is_color()                   { return false; }
            true
        })
        .map(|pm| {
            let ch  = patch.start_address + pm.channel_offset;
            let val = universe.get_channel(ch).unwrap_or(0);
            (pm.parameter.short_label().to_string(), ch, val)
        })
        .collect();

    // Sync wheel from current RGB every frame so slider/cue changes are reflected.
    // last_wheel_fixture_id is still written so the multi-fixture path can detect
    // when to reset the wheel on selection changes.
    if is_rgb {
        app.ui_state.color_wheel.set_from_srgb_100(r, g, b);
        app.ui_state.last_wheel_fixture_id = Some(patch.id);
    }

    // ── Pending changes collected during rendering, applied after ─────────────
    let mut apply_int_raw : Option<u8>  = None;
    let mut apply_int_vi  : Option<f32> = None;
    let mut apply_wheel   : bool        = false;
    let mut apply_channels: Vec<(u16, u8)> = Vec::new();

    // ── Horizontal scroll area ────────────────────────────────────────────────
    egui::ScrollArea::horizontal()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let avail_h  = ui.available_height().max(60.0);
            let slider_h = (avail_h - 24.0).max(40.0);
            let wheel_size = slider_h.min(220.0);

            ui.horizontal(|ui| {
                // Intensity
                ui.vertical(|ui| {
                    ui.label("Int");
                    if has_int {
                        let mut v = int_raw;
                        if ui.add_sized([30.0, slider_h], egui::Slider::new(&mut v, 0..=100).vertical()).changed() {
                            apply_int_raw = Some(v);
                        }
                    } else if is_rgb {
                        let mut v = vi_val;
                        if ui.add_sized(
                            [30.0, slider_h],
                            egui::Slider::new(&mut v, 0.0..=1.0)
                                .vertical()
                                .custom_formatter(|val, _| format!("{:.0}", val * 100.0)),
                        ).changed() {
                            apply_int_vi = Some(v);
                        }
                    }
                });

                // Colour wheel + per-channel sliders (RGB fixtures only)
                if is_rgb {
                    ui.separator();

                    ui.vertical(|ui| {
                        ui.label("Color");
                        if app.ui_state.color_wheel.show(ui, wheel_size) {
                            apply_wheel = true;
                        }
                    });

                    ui.separator();

                    // One vertical slider per colour channel
                    let needs_vi = !has_int;
                    macro_rules! col_slider {
                        ($label:expr, $ch_opt:expr, $init:expr) => {
                            if let Some(ch) = $ch_opt {
                                ui.vertical(|ui| {
                                    ui.label($label);
                                    let mut v = $init;
                                    if ui.add_sized(
                                        [30.0, slider_h],
                                        egui::Slider::new(&mut v, 0..=100).vertical(),
                                    ).changed() {
                                        apply_channels.push((ch, v));
                                        let _ = needs_vi;
                                    }
                                });
                            }
                        };
                    }
                    col_slider!("R",  r_ch,     r    );
                    col_slider!("G",  g_ch,     g    );
                    col_slider!("B",  b_ch,     b    );
                    col_slider!("A",  amber_ch, amber);
                    col_slider!("W",  white_ch, white);
                    col_slider!("UV", uv_ch,    uv   );
                }

                // Extra channels — Strobe, Pan, Tilt, Focus, Zoom, Gobo, Custom, etc.
                // Rendered for every parameter not already covered above.
                if !extra_channels.is_empty() {
                    ui.separator();
                    for (label, ch, init_val) in &extra_channels {
                        ui.vertical(|ui| {
                            ui.label(label.as_str());
                            let mut v = *init_val;
                            if ui.add_sized(
                                [30.0, slider_h],
                                egui::Slider::new(&mut v, 0..=100).vertical(),
                            ).changed() {
                                apply_channels.push((*ch, v));
                            }
                        });
                    }
                }
            });
        });

    // ── Apply changes (universe now exclusively available) ────────────────────
    if let Some(v) = apply_int_raw {
        if let Some(ch) = int_ch { let _ = universe.set_channel(ch, v); }
    }
    if let Some(v) = apply_int_vi {
        let p = patch.clone(); let pr = profile.clone();
        if let Err(e) = app.virtual_intensity.set_intensity(patch.id, v, universe, &p, &pr) {
            log::error!("Failed to set virtual intensity: {}", e);
        }
    }
    if apply_wheel {
        let (fr, fg, fb) = app.ui_state.color_wheel.selected_color();
        let intensity = if has_int { 1.0_f32 } else {
            app.virtual_intensity.get_intensity(patch.id).unwrap_or(1.0)
        };
        let new_r = (fr * intensity * 100.0).round().clamp(0.0, 100.0) as u8;
        let new_g = (fg * intensity * 100.0).round().clamp(0.0, 100.0) as u8;
        let new_b = (fb * intensity * 100.0).round().clamp(0.0, 100.0) as u8;
        if let (Some(rc), Some(gc), Some(bc)) = (r_ch, g_ch, b_ch) {
            let _ = universe.set_channel(rc, new_r);
            let _ = universe.set_channel(gc, new_g);
            let _ = universe.set_channel(bc, new_b);
        }
        if !has_int {
            let mut cv = std::collections::HashMap::new();
            cv.insert(FixtureParameter::Red,   new_r);
            cv.insert(FixtureParameter::Green, new_g);
            cv.insert(FixtureParameter::Blue,  new_b);
            for pm in profile.color_parameters() {
                if !matches!(pm.parameter, FixtureParameter::Red | FixtureParameter::Green | FixtureParameter::Blue) {
                    let ch = patch.start_address + pm.channel_offset;
                    if let Ok(val) = universe.get_channel(ch) { cv.insert(pm.parameter.clone(), val); }
                }
            }
            app.virtual_intensity.set_color(patch.id, cv);
        }
    }
    if !apply_channels.is_empty() {
        let needs_vi = !has_int;
        for (ch, v) in apply_channels {
            let _ = universe.set_channel(ch, v);
        }
        if needs_vi {
            let p = patch.clone(); let pr = profile.clone();
            app.virtual_intensity.update_from_universe(patch.id, universe, &p, &pr);
        }
    }
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

/// Render shared properties for multiple selected fixtures (ETC-style multi-edit).
fn render_multi_fixture_properties(ui: &mut Ui, app: &mut EasyCueApp) {
    use crate::fixtures::profiles::FixtureParameter;

    // ── Collect fixture metadata ──────────────────────────────────────────────
    let mut sorted_ids: Vec<usize> = app.ui_state.selected_fixtures.iter().copied().collect();
    sorted_ids.sort();

    struct FxInfo {
        id: usize,
        label: String,
        profile_name: String,
        patch: crate::fixtures::Patch,
        profile: crate::fixtures::FixtureProfile,
        has_intensity: bool,
        intensity_ch: u16,
        is_rgb_only: bool,
        r_ch: Option<u16>,
        g_ch: Option<u16>,
        b_ch: Option<u16>,
        amber_ch: Option<u16>,
        white_ch: Option<u16>,
        uv_ch: Option<u16>,
    }

    let fix_infos: Vec<FxInfo> = sorted_ids
        .iter()
        .filter_map(|&id| {
            let patch = app.fixtures.patch_list().get_patch(id)?.clone();
            let profile = app.fixtures.get_profile(&patch.profile_id)?.clone();
            let addr = patch.start_address;
            let has_intensity = profile.has_parameter(&FixtureParameter::Intensity);
            let intensity_ch = profile
                .get_parameter_offset(&FixtureParameter::Intensity)
                .map(|off| addr + off)
                .unwrap_or(0);
            let r_ch = profile.get_parameter_offset(&FixtureParameter::Red).map(|off| addr + off);
            let g_ch = profile.get_parameter_offset(&FixtureParameter::Green).map(|off| addr + off);
            let b_ch = profile.get_parameter_offset(&FixtureParameter::Blue).map(|off| addr + off);
            let amber_ch = profile.get_parameter_offset(&FixtureParameter::Amber).map(|off| addr + off);
            let white_ch = profile.get_parameter_offset(&FixtureParameter::White).map(|off| addr + off);
            let uv_ch = profile.get_parameter_offset(&FixtureParameter::Uv).map(|off| addr + off);
            let is_rgb_only = profile.is_rgb() && !has_intensity;
            Some(FxInfo {
                id,
                label: patch.label.clone(),
                profile_name: profile.name.clone(),
                patch,
                profile,
                has_intensity,
                intensity_ch,
                is_rgb_only,
                r_ch,
                g_ch,
                b_ch,
                amber_ch,
                white_ch,
                uv_ch,
            })
        })
        .collect();

    if fix_infos.is_empty() {
        ui.label("No valid fixtures found");
        return;
    }

    // Parameters present in every selected fixture that aren't Intensity or standard
    // colour channels (those are handled above).  Vec of (label, per-fixture channel).
    let extra_common: Vec<(String, Vec<u16>)> = fix_infos[0].profile.parameters.iter()
        .filter(|pm| {
            !matches!(
                pm.parameter,
                FixtureParameter::Intensity
                    | FixtureParameter::Red
                    | FixtureParameter::Green
                    | FixtureParameter::Blue
                    | FixtureParameter::Amber
                    | FixtureParameter::White
                    | FixtureParameter::Uv
            ) && fix_infos[1..].iter().all(|fi| fi.profile.has_parameter(&pm.parameter))
        })
        .map(|pm| {
            let channels: Vec<u16> = fix_infos.iter()
                .map(|fi| {
                    fi.patch.start_address
                        + fi.profile.get_parameter_offset(&pm.parameter).unwrap_or(0)
                })
                .collect();
            (pm.parameter.short_label().to_string(), channels)
        })
        .collect();

    // ── Read current values from universe (immutable) ─────────────────────────
    let intensities: Vec<u8>;
    let rs: Vec<u8>;
    let gs: Vec<u8>;
    let bs: Vec<u8>;
    let ambers: Vec<u8>;
    let whites: Vec<u8>;
    let uvs: Vec<u8>;
    let all_rgb: bool;
    let all_amber: bool;
    let all_white: bool;
    let all_uv: bool;
    let extra_vals: Vec<Vec<u8>>;

    {
        let universe = app.universes.first();
        let get = |ch: u16| -> u8 {
            universe.and_then(|u| u.get_channel(ch).ok()).unwrap_or(0)
        };

        intensities = fix_infos
            .iter()
            .map(|fi| {
                if fi.has_intensity {
                    get(fi.intensity_ch)
                } else if fi.is_rgb_only {
                    let vi = app.virtual_intensity.get_intensity(fi.id).unwrap_or_else(|| {
                        match (fi.r_ch, fi.g_ch, fi.b_ch) {
                            (Some(r), Some(g), Some(b)) => {
                                (get(r).max(get(g)).max(get(b)) as f32) / 100.0
                            }
                            _ => 0.0,
                        }
                    });
                    (vi * 100.0).round() as u8
                } else {
                    0
                }
            })
            .collect();

        all_rgb = fix_infos
            .iter()
            .all(|fi| fi.r_ch.is_some() && fi.g_ch.is_some() && fi.b_ch.is_some());
        all_amber = all_rgb && fix_infos.iter().all(|fi| fi.amber_ch.is_some());
        all_white = all_rgb && fix_infos.iter().all(|fi| fi.white_ch.is_some());
        all_uv = all_rgb && fix_infos.iter().all(|fi| fi.uv_ch.is_some());

        if all_rgb {
            rs = fix_infos.iter().map(|fi| fi.r_ch.map(get).unwrap_or(0)).collect();
            gs = fix_infos.iter().map(|fi| fi.g_ch.map(get).unwrap_or(0)).collect();
            bs = fix_infos.iter().map(|fi| fi.b_ch.map(get).unwrap_or(0)).collect();
            ambers = fix_infos.iter().map(|fi| fi.amber_ch.map(get).unwrap_or(0)).collect();
            whites = fix_infos.iter().map(|fi| fi.white_ch.map(get).unwrap_or(0)).collect();
            uvs = fix_infos.iter().map(|fi| fi.uv_ch.map(get).unwrap_or(0)).collect();
        } else {
            rs = vec![];
            gs = vec![];
            bs = vec![];
            ambers = vec![];
            whites = vec![];
            uvs = vec![];
        }

        extra_vals = extra_common.iter()
            .map(|(_, channels)| {
                channels.iter()
                    .map(|&ch| universe.and_then(|u| u.get_channel(ch).ok()).unwrap_or(0))
                    .collect()
            })
            .collect();
    }

    // ── Helpers ───────────────────────────────────────────────────────────────
    let is_uniform = |vals: &[u8]| vals.windows(2).all(|w| w[0] == w[1]);
    let mixed_col = egui::Color32::from_rgb(220, 160, 40);

    // Gray slider helper: dims inactive widget colours so "mixed" looks inactive.
    // Used inside ui.scope() so the style change is scoped to the child Ui.
    let gray_visuals = |ui: &mut egui::Ui| {
        ui.visuals_mut().widgets.inactive.bg_fill = egui::Color32::from_gray(52);
        ui.visuals_mut().widgets.inactive.fg_stroke.color = egui::Color32::from_gray(140);
        ui.visuals_mut().widgets.hovered.bg_fill = egui::Color32::from_gray(68);
    };

    // ── Changes to apply after rendering ─────────────────────────────────────
    let mut apply_intensity: Option<u8> = None;
    let mut apply_wheel_color: bool = false;
    let mut apply_r: Option<u8> = None;
    let mut apply_g: Option<u8> = None;
    let mut apply_b: Option<u8> = None;
    let mut apply_amber: Option<u8> = None;
    let mut apply_white: Option<u8> = None;
    let mut apply_uv: Option<u8> = None;
    let mut apply_extra: Vec<Option<u8>> = vec![None; extra_common.len()];

    // ── Render header ─────────────────────────────────────────────────────────
    ui.label(egui::RichText::new(format!("{} Fixtures Selected", fix_infos.len())).strong());

    // Multi-select: clear last-synced ID so switching back to single re-syncs.
    app.ui_state.last_wheel_fixture_id = None;

    if all_rgb {
        let color_uniform = is_uniform(&rs) && is_uniform(&gs) && is_uniform(&bs);
        if color_uniform {
            app.ui_state.color_wheel.set_from_srgb_100(rs[0], gs[0], bs[0]);
        }
    }

    // ── Horizontal scroll area ────────────────────────────────────────────────
    egui::ScrollArea::horizontal()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let avail_h  = ui.available_height().max(60.0);
            let slider_h = (avail_h - 24.0).max(40.0);
            let wheel_size = slider_h.min(220.0);

            ui.horizontal(|ui| {
                // ── Intensity column ──────────────────────────────────────────
                let int_uniform = is_uniform(&intensities);
                let mut int_val = intensities[0];
                ui.vertical(|ui| {
                    ui.label("Int");
                    let resp = if int_uniform {
                        ui.add_sized([30.0, slider_h], egui::Slider::new(&mut int_val, 0..=100).vertical())
                    } else {
                        ui.scope(|ui| {
                            gray_visuals(ui);
                            ui.add_sized([30.0, slider_h], egui::Slider::new(&mut int_val, 0..=100).vertical())
                        }).inner
                    };
                    if resp.changed() { apply_intensity = Some(int_val); }
                    if !int_uniform { ui.colored_label(mixed_col, "≠"); }
                });

                // ── Colour wheel + channel sliders (all-RGB only) ─────────────
                if all_rgb {
                    ui.separator();

                    let color_uniform = is_uniform(&rs) && is_uniform(&gs) && is_uniform(&bs);
                    ui.vertical(|ui| {
                        ui.label("Color");
                        if app.ui_state.color_wheel.show(ui, wheel_size) {
                            apply_wheel_color = true;
                        }
                        if !color_uniform { ui.colored_label(mixed_col, "≠"); }
                    });

                    ui.separator();

                    macro_rules! ch_slider {
                        ($label:expr, $vals:expr, $apply:expr) => {{
                            let uniform = is_uniform(&$vals);
                            let mut val = $vals[0];
                            ui.vertical(|ui| {
                                ui.label($label);
                                let resp = if uniform {
                                    ui.add_sized([30.0, slider_h], egui::Slider::new(&mut val, 0..=100).vertical())
                                } else {
                                    ui.scope(|ui| {
                                        gray_visuals(ui);
                                        ui.add_sized([30.0, slider_h], egui::Slider::new(&mut val, 0..=100).vertical())
                                    }).inner
                                };
                                if resp.changed() { $apply = Some(val); }
                                if !uniform { ui.colored_label(mixed_col, "≠"); }
                            });
                        }};
                    }

                    ch_slider!("R",  rs,     apply_r    );
                    ch_slider!("G",  gs,     apply_g    );
                    ch_slider!("B",  bs,     apply_b    );
                    if all_amber { ch_slider!("A",  ambers, apply_amber); }
                    if all_white { ch_slider!("W",  whites, apply_white); }
                    if all_uv    { ch_slider!("UV", uvs,    apply_uv   ); }
                }

                // Extra channels shared by all fixtures (Strobe, Pan, Tilt, Gobo, etc.)
                if !extra_common.is_empty() {
                    ui.separator();
                    for (i, (label, _)) in extra_common.iter().enumerate() {
                        let vals = &extra_vals[i];
                        let uniform = is_uniform(vals);
                        let mut val = vals[0];
                        ui.vertical(|ui| {
                            ui.label(label.as_str());
                            let resp = if uniform {
                                ui.add_sized([30.0, slider_h], egui::Slider::new(&mut val, 0..=100).vertical())
                            } else {
                                ui.scope(|ui| {
                                    gray_visuals(ui);
                                    ui.add_sized([30.0, slider_h], egui::Slider::new(&mut val, 0..=100).vertical())
                                }).inner
                            };
                            if resp.changed() { apply_extra[i] = Some(val); }
                            if !uniform { ui.colored_label(mixed_col, "≠"); }
                        });
                    }
                }
            });
        });

    // ── Apply changes ─────────────────────────────────────────────────────────

    // Intensity
    if let Some(new_val) = apply_intensity {
        for fi in &fix_infos {
            if fi.has_intensity {
                if let Some(u) = app.universes.first_mut() {
                    let _ = u.set_channel(fi.intensity_ch, new_val);
                }
            } else if fi.is_rgb_only {
                if let Some(u) = app.universes.first_mut() {
                    let _ = app.virtual_intensity.set_intensity(
                        fi.id, new_val as f32 / 100.0, u, &fi.patch, &fi.profile,
                    );
                }
            }
        }
    }

    // Wheel colour pick — applies to all fixtures, preserving each one's intensity.
    if apply_wheel_color {
        let (fr, fg, fb) = app.ui_state.color_wheel.selected_color();
        // Collect per-fixture intensities before mutably borrowing universes.
        let intensities: Vec<f32> = fix_infos.iter().map(|fi| {
            if fi.has_intensity {
                1.0_f32
            } else {
                app.virtual_intensity.get_intensity(fi.id).unwrap_or(1.0)
            }
        }).collect();
        for (fi, intensity) in fix_infos.iter().zip(intensities.iter()) {
            let nr = (fr * intensity * 100.0).round().clamp(0.0, 100.0) as u8;
            let ng = (fg * intensity * 100.0).round().clamp(0.0, 100.0) as u8;
            let nb = (fb * intensity * 100.0).round().clamp(0.0, 100.0) as u8;
            if let Some(u) = app.universes.first_mut() {
                if let (Some(rc), Some(gc), Some(bc)) = (fi.r_ch, fi.g_ch, fi.b_ch) {
                    let _ = u.set_channel(rc, nr);
                    let _ = u.set_channel(gc, ng);
                    let _ = u.set_channel(bc, nb);
                }
                if !fi.has_intensity {
                    let mut cv = std::collections::HashMap::new();
                    cv.insert(FixtureParameter::Red, nr);
                    cv.insert(FixtureParameter::Green, ng);
                    cv.insert(FixtureParameter::Blue, nb);
                    if let Some(ac) = fi.amber_ch { cv.insert(FixtureParameter::Amber, u.get_channel(ac).unwrap_or(0)); }
                    if let Some(wc) = fi.white_ch { cv.insert(FixtureParameter::White, u.get_channel(wc).unwrap_or(0)); }
                    if let Some(uc) = fi.uv_ch   { cv.insert(FixtureParameter::Uv,    u.get_channel(uc).unwrap_or(0)); }
                    app.virtual_intensity.set_color(fi.id, cv);
                }
            }
        }
    }

    // Individual colour channel sliders
    let has_ch_change = apply_r.is_some() || apply_g.is_some() || apply_b.is_some()
        || apply_amber.is_some() || apply_white.is_some() || apply_uv.is_some();
    if has_ch_change {
        for fi in &fix_infos {
            if let Some(u) = app.universes.first_mut() {
                if let Some(v) = apply_r     { if let Some(ch) = fi.r_ch     { let _ = u.set_channel(ch, v); } }
                if let Some(v) = apply_g     { if let Some(ch) = fi.g_ch     { let _ = u.set_channel(ch, v); } }
                if let Some(v) = apply_b     { if let Some(ch) = fi.b_ch     { let _ = u.set_channel(ch, v); } }
                if let Some(v) = apply_amber { if let Some(ch) = fi.amber_ch { let _ = u.set_channel(ch, v); } }
                if let Some(v) = apply_white { if let Some(ch) = fi.white_ch { let _ = u.set_channel(ch, v); } }
                if let Some(v) = apply_uv   { if let Some(ch) = fi.uv_ch    { let _ = u.set_channel(ch, v); } }
                if !fi.has_intensity {
                    let p = fi.patch.clone();
                    let pr = fi.profile.clone();
                    app.virtual_intensity.update_from_universe(fi.id, u, &p, &pr);
                }
            }
        }
    }

    // Extra channels (Strobe, Pan, Tilt, etc.) — applied to each fixture's channel
    for (i, (_, channels)) in extra_common.iter().enumerate() {
        if let Some(v) = apply_extra[i] {
            if let Some(u) = app.universes.first_mut() {
                for &ch in channels {
                    let _ = u.set_channel(ch, v);
                }
            }
        }
    }
}
