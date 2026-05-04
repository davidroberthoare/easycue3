//! Unified cue list panel — lighting and audio cues in one table

use egui::Ui;
use egui_extras::{TableBuilder, Column};
use crate::app::EasyCueApp;
use egui_phosphor::regular as ph;

const COLOR_ACTIVE:          egui::Color32 = egui::Color32::from_rgb(40, 110, 40);
const COLOR_FADING:          egui::Color32 = egui::Color32::from_rgb(120, 90, 20);
const COLOR_SELECTED:        egui::Color32 = egui::Color32::from_rgb(60, 60, 120);
const COLOR_ACTIVE_SELECTED: egui::Color32 = egui::Color32::from_rgb(60, 110, 130);
const COLOR_NEXT:            egui::Color32 = egui::Color32::from_rgb(140, 100, 20);
// Idle row tints by cue type (subtle — state colours override these)
const COLOR_ROW_LX:          egui::Color32 = egui::Color32::from_rgba_premultiplied(30, 60, 90, 30);
const COLOR_ROW_AUDIO:       egui::Color32 = egui::Color32::from_rgba_premultiplied(20, 80, 40, 30);

pub fn render_cues_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    // ── Toolbar ──────────────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        // ── Transport ───────────────────────────────────────────────────────

        // Resolve the on-deck target: typed cue number overrides the default next cue.
        let next_idx = app.cue_list.next_any_index();
        let go_target_idx: Option<usize> = {
            let input = app.ui_state.go_cue_input.trim();
            if input.is_empty() {
                next_idx
            } else {
                input.parse::<f32>().ok().and_then(|num| {
                    app.cue_list.cues().iter()
                        .position(|c| (c.number - num).abs() < 0.005)
                })
            }
        };
        let next_hint = next_idx
            .and_then(|i| app.cue_list.get_cue(i))
            .map(|c| format!("{:.1}", c.number))
            .unwrap_or_default();

        let go_enabled = go_target_idx.is_some();
        let back_enabled = app.cue_list.previous_any_index().is_some();

        // On-deck number box
        let ondeck_resp = ui.add(
            egui::TextEdit::singleline(&mut app.ui_state.go_cue_input)
                .desired_width(45.0)
                .hint_text(&next_hint)
                .font(egui::TextStyle::Monospace),
        );
        let enter_in_box = ondeck_resp.lost_focus()
            && ui.input(|i| i.key_pressed(egui::Key::Enter));

        let go_btn = egui::Button::new(format!("{} GO", ph::PLAY))
            .fill(if go_enabled { egui::Color32::from_rgb(50, 120, 50) } else { egui::Color32::from_rgb(30, 60, 30) });
        if ui.add_enabled(go_enabled, go_btn).clicked() || (go_enabled && enter_in_box) {
            if app.ui_state.go_cue_input.trim().is_empty() {
                if app.go_next() {
                    let label = app.cue_list.current_index()
                        .and_then(|i| app.cue_list.get_cue(i))
                        .map(|c| format!("Q{:.1} {}", c.number, c.label))
                        .unwrap_or_default();
                    app.ui_state.status_message = format!("GO → {}", label);
                }
            } else if let Some(abs_idx) = go_target_idx {
                let label_str = app.cue_list.get_cue(abs_idx)
                    .map(|c| format!("Q{:.1} {}", c.number, c.label))
                    .unwrap_or_default();
                if app.go_to_cue(abs_idx) {
                    app.ui_state.go_cue_input.clear();
                    app.ui_state.status_message = format!("GO → {}", label_str);
                }
            }
        }

        let back_btn = egui::Button::new(format!("{} BACK", ph::SKIP_BACK))
            .fill(if back_enabled { egui::Color32::from_rgb(50, 80, 120) } else { egui::Color32::from_rgb(30, 40, 60) });
        if ui.add_enabled(back_enabled, back_btn).clicked() {
            app.go_back();
            app.ui_state.status_message = "BACK".to_string();
        }

        if ui.button(format!("{} STOP", ph::STOP)).clicked() {
            app.playback.stop();
            #[cfg(feature = "audio")]
            app.audio_playback.stop_all();
            app.ui_state.status_message = "STOP".to_string();
        }

        ui.separator();

        // Edit actions
        if ui.button(format!("{} Record LX", ph::RECORD)).clicked() {
            let id = app.record_cue();
            app.ui_state.selected_cue_id = Some(id);
            app.ui_state.selected_lighting_cue_id = Some(id);
        }

        #[cfg(feature = "audio")]
        if ui.button(format!("{} Adjust", ph::PLUS)).on_hover_text("Add a sound adjust cue (volume ramp / stop)").clicked() {
            let next_number = app.cue_list.cues().iter()
                .last()
                .map(|c| c.number.floor() + 1.0)
                .unwrap_or(1.0);
            let cue = crate::cue::Cue::new_adjust(next_number);
            let id = app.cue_list.next_id();
            app.cue_list.add_cue(cue);
            app.ui_state.selected_cue_id = Some(id);
            app.ui_state.status_message = format!("Added adjust cue {:.0}", next_number);
        }

        #[cfg(feature = "audio")]
        if ui.button(format!("{} Audio", ph::PLUS)).clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Audio Files", &["mp3", "wav", "flac", "ogg", "aac", "m4a"])
                .set_title("Select Audio File")
                .pick_file()
            {
                let next_number = app.cue_list.cues().iter()
                    .last()
                    .map(|c| c.number.floor() + 1.0)
                    .unwrap_or(1.0);
                let filename = path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Audio")
                    .to_string();
                let mut cue = crate::cue::Cue::new_audio(next_number, path);
                cue.label = format!("{}", filename);
                let id = app.cue_list.next_id();
                app.cue_list.add_cue(cue);
                app.ui_state.selected_cue_id = Some(id);
                app.ui_state.status_message = format!("Added audio cue {:.0}", next_number);
                app.ui_state.audio_file_cache.clear();
            }
        }

        if ui.button(format!("{} Delete", ph::TRASH)).clicked() {
            if let Some(sel_id) = app.ui_state.selected_cue_id {
                if let Some(abs_idx) = app.cue_list.cues().iter().position(|c| c.id == sel_id) {
                    let num = app.cue_list.get_cue(abs_idx).map(|c| c.number).unwrap_or(0.0);
                    if app.cue_list.remove_cue(abs_idx).is_ok() {
                        app.ui_state.selected_cue_id = None;
                        app.ui_state.selected_lighting_cue_id = None;
                        app.ui_state.selected_audio_cue_id = None;
                        app.ui_state.status_message = format!("Deleted cue {:.1}", num);
                        #[cfg(feature = "audio")]
                        app.ui_state.audio_file_cache.clear();
                    }
                }
            } else {
                app.ui_state.status_message = "Select a cue first".to_string();
            }
        }

        ui.separator();

        // Masters — compact
        let bo_text = if app.ui_state.blackout_active { "●" } else { ph::LIGHTBULB };
        let bo_fill = if app.ui_state.blackout_active { egui::Color32::from_rgb(80, 40, 40) } else { egui::Color32::from_rgb(50, 50, 50) };
        if ui.add(egui::Button::new(bo_text).fill(bo_fill).min_size(egui::vec2(26.0, 20.0))).clicked() {
            if app.ui_state.blackout_active {
                app.ui_state.lighting_master = app.ui_state.previous_lighting_master;
                app.ui_state.blackout_active = false;
            } else {
                app.ui_state.previous_lighting_master = app.ui_state.lighting_master;
                app.ui_state.lighting_master = 0.0;
                app.ui_state.blackout_active = true;
            }
        }
        let mut lx_pct = (app.ui_state.lighting_master * 100.0) as i32;
        if ui.add(egui::DragValue::new(&mut lx_pct).speed(1.0).range(0..=100).suffix("%").prefix("LX ")).changed() {
            app.ui_state.lighting_master = lx_pct as f32 / 100.0;
            app.ui_state.blackout_active = false;
        }

        #[cfg(feature = "audio")]
        {
            let mute_text = if app.ui_state.audio_mute_active { ph::SPEAKER_SLASH } else { ph::SPEAKER_HIGH };
            let mute_fill = if app.ui_state.audio_mute_active { egui::Color32::from_rgb(80, 40, 40) } else { egui::Color32::from_rgb(50, 50, 50) };
            if ui.add(egui::Button::new(mute_text).fill(mute_fill).min_size(egui::vec2(26.0, 20.0))).clicked() {
                if app.ui_state.audio_mute_active {
                    app.ui_state.sound_master = app.ui_state.previous_sound_master;
                    app.ui_state.audio_mute_active = false;
                } else {
                    app.ui_state.previous_sound_master = app.ui_state.sound_master;
                    app.ui_state.sound_master = 0.0;
                    app.ui_state.audio_mute_active = true;
                }
            }
            let mut snd_pct = (app.ui_state.sound_master * 100.0) as i32;
            if ui.add(egui::DragValue::new(&mut snd_pct).speed(1.0).range(0..=100).suffix("%").prefix("SND ")).changed() {
                app.ui_state.sound_master = snd_pct as f32 / 100.0;
                app.ui_state.audio_mute_active = false;
            }
        }
    });

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    // ── Pre-compute display state ─────────────────────────────────────────────
    let footer_reserved = 52.0;
    let available_height = (ui.available_height() - footer_reserved).max(0.0);

    let selected_id       = app.ui_state.selected_cue_id;
    let next_any_idx      = app.cue_list.next_any_index();
    let lx_active_id      = app.playback.current_cue_id();
    let lx_fade           = app.playback.fade_progress();
    #[cfg(feature = "audio")]
    let audio_active_set: std::collections::HashSet<u32> =
        app.audio_playback.active_cue_ids().into_iter().collect();
    #[cfg(not(feature = "audio"))]
    let audio_active_set: std::collections::HashSet<u32> = std::collections::HashSet::new();

    // Pre-compute global sound-fade state so we can highlight the triggering Adjust cue row.
    #[cfg(feature = "audio")]
    let sound_fade_trigger: Option<u32> = app.sound_fade.as_ref().map(|sf| sf.trigger_cue_id);
    #[cfg(feature = "audio")]
    let sound_fade_progress: f32 = app.sound_fade.as_ref().map(|sf| {
        if sf.fade_time > 0.0 {
            (sf.start.elapsed().as_secs_f32() / sf.fade_time).clamp(0.0, 1.0)
        } else { 1.0 }
    }).unwrap_or(0.0);

    let cue_count = app.cue_list.len();

    if cue_count == 0 {
        ui.vertical_centered(|ui| {
            ui.add_space(30.0);
            ui.label(egui::RichText::new("No Cues").color(egui::Color32::GRAY));
            ui.add_space(10.0);
            ui.label("Press 'Record LX' or Ctrl+R to create your first lighting cue");
            #[cfg(feature = "audio")]
            ui.label("Press 'Add Audio' to add a sound cue");
        });
        render_footer(ui, app);
        return;
    }

    // ── Table ─────────────────────────────────────────────────────────────────
    let mut clicked_id:     Option<u32>   = None;
    let mut go_to_abs_idx:  Option<usize> = None;

    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::exact(24.0))   // play button
        .column(Column::exact(22.0))   // type icon
        .column(Column::initial(55.0).at_least(40.0))  // Q#
        .column(Column::remainder().at_least(120.0))   // label
        .column(Column::initial(130.0).at_least(80.0)) // info
        .column(Column::initial(55.0).at_least(40.0))  // status
        .min_scrolled_height(0.0)
        .max_scroll_height(available_height)
        .header(20.0, |mut h| {
            h.col(|ui| { ui.strong(""); });
            h.col(|ui| { ui.strong(""); });
            h.col(|ui| { ui.strong("Q#"); });
            h.col(|ui| { ui.strong("Label"); });
            h.col(|ui| { ui.strong("Info"); });
            h.col(|ui| { ui.strong("State"); });
        })
        .body(|body| {
            body.rows(22.0, cue_count, |mut row| {
                let abs_idx = row.index();
                let cue = app.cue_list.get_cue(abs_idx).unwrap();
                let cue_id     = cue.id;
                let cue_number = cue.number;
                let cue_label  = cue.label.clone();
                let is_lighting = cue.is_lighting();
                #[cfg(feature = "audio")]
                let is_audio = cue.is_audio();
                #[cfg(not(feature = "audio"))]
                let is_audio = false;
                #[cfg(feature = "audio")]
                let is_adjust = cue.is_adjust();
                #[cfg(not(feature = "audio"))]
                let is_adjust = false;

                // Info column text (contextual)
                let info_text = if is_lighting {
                    let fade = cue.lighting_data().map(|d| d.fade_up).unwrap_or(0.0);
                    if fade > 0.0 { format!("{} {:.1}s", ph::CLOCK, fade) } else { "instant".to_string() }
                } else if is_adjust {
                    #[cfg(feature = "audio")]
                    {
                        cue.adjust_data()
                            .map(|d| {
                                let stop = if d.stop_when_complete { "+stop" } else { "" };
                                let target = d.target_audio_cue
                                    .map(|n| format!("Q{:.1} ", n))
                                    .unwrap_or_else(|| "master ".to_string());
                                if d.fade_time > 0.0 {
                                    format!("{}→{:.0}% {:.1}s{}", target, d.volume * 100.0, d.fade_time, stop)
                                } else {
                                    format!("{}→{:.0}% snap{}", target, d.volume * 100.0, stop)
                                }
                            })
                            .unwrap_or_default()
                    }
                    #[cfg(not(feature = "audio"))]
                    String::new()
                } else {
                    #[cfg(feature = "audio")]
                    {
                        cue.audio_data()
                            .map(|d| d.audio_path.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("?")
                                .to_string())
                            .unwrap_or_default()
                    }
                    #[cfg(not(feature = "audio"))]
                    String::new()
                };

                // Trigger indicator — look up the target cue number from its stable ID
                let trigger_label = if is_lighting {
                    cue.lighting_data()
                        .and_then(|d| d.triggers_audio_cue)
                        .and_then(|id| app.cue_list.find_by_id(id))
                        .map(|t| format!("→{}{:.1}", ph::SPEAKER_HIGH, t.number))
                } else {
                    #[cfg(feature = "audio")]
                    {
                        cue.audio_data()
                            .and_then(|d| d.triggers_lighting_cue)
                            .and_then(|id| app.cue_list.find_by_id(id))
                            .map(|t| format!("→{}{:.1}", ph::LIGHTBULB, t.number))
                    }
                    #[cfg(not(feature = "audio"))]
                    None
                };

                // Row state flags
                let is_lx_active   = lx_active_id == Some(cue_id) && is_lighting;
                let is_lx_fading   = is_lx_active && lx_fade.is_some();
                let is_audio_active = is_audio && audio_active_set.contains(&cue_id);
                #[cfg(feature = "audio")]
                let row_audio_state = if is_audio_active {
                    app.audio_playback.stream_state(cue_id)
                } else {
                    None
                };
                #[cfg(feature = "audio")]
                let is_audio_fading = is_audio_active && matches!(
                    row_audio_state,
                    Some(crate::audio::AudioCueState::FadingIn { .. } | crate::audio::AudioCueState::FadingOut { .. })
                );
                #[cfg(not(feature = "audio"))]
                let is_audio_fading = false;

                // Adjust cue: active while its targeted stream has a volume-adjust in progress,
                // or while the global sound fade it triggered is still running.
                #[cfg(feature = "audio")]
                let adjust_progress: Option<f32> = if is_adjust {
                    cue.adjust_data().and_then(|d| {
                        if let Some(target_num) = d.target_audio_cue {
                            let target_id = app.cue_list.cues().iter()
                                .find(|c| (c.number - target_num).abs() < 0.005)
                                .map(|c| c.id);
                            target_id.and_then(|tid| app.audio_playback.volume_adjust_progress(tid))
                        } else if sound_fade_trigger == Some(cue_id) {
                            Some(sound_fade_progress)
                        } else {
                            None
                        }
                    })
                } else {
                    None
                };
                #[cfg(not(feature = "audio"))]
                let adjust_progress: Option<f32> = None;
                let is_adjust_active = adjust_progress.is_some();

                let is_active  = is_lx_active   || is_audio_active || is_adjust_active;
                let is_fading  = is_lx_fading   || is_audio_fading || is_adjust_active;
                let is_selected = selected_id   == Some(cue_id);
                let is_next    = next_any_idx   == Some(abs_idx);

                if is_selected { row.set_selected(true); }

                let bg_color = if is_active && is_selected {
                    COLOR_ACTIVE_SELECTED
                } else if is_fading {
                    COLOR_FADING
                } else if is_active {
                    COLOR_ACTIVE
                } else if is_selected {
                    COLOR_SELECTED
                } else if is_next {
                    COLOR_NEXT
                } else if is_lighting {
                    COLOR_ROW_LX
                } else {
                    COLOR_ROW_AUDIO
                };

                let paint_bg = |ui: &mut egui::Ui| {
                    ui.painter().rect_filled(ui.max_rect(), 0.0, bg_color);
                };

                let mut row_responses = Vec::new();

                // Col 0: play button
                row.col(|ui| {
                    paint_bg(ui);
                    let btn_text = if is_next { ph::CARET_RIGHT } else { ph::PLAY };
                    if ui.small_button(btn_text).on_hover_text("Fire this cue").clicked() {
                        go_to_abs_idx = Some(abs_idx);
                    }
                });

                // Col 1: type icon
                row.col(|ui| {
                    paint_bg(ui);
                    let icon = if is_lighting { ph::LIGHTBULB } else if is_adjust { ph::SLIDERS } else { ph::SPEAKER_HIGH };
                    ui.label(egui::RichText::new(icon).size(13.0));
                });

                // Col 2: cue number
                row.col(|ui| {
                    paint_bg(ui);
                    let (rect, resp) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click());
                    ui.painter().text(
                        rect.left_center() + egui::vec2(4.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        format!("{:.1}", cue_number),
                        egui::FontId::default(),
                        ui.style().visuals.text_color(),
                    );
                    row_responses.push(resp);
                });

                // Col 3: label (editable inline)
                row.col(|ui| {
                    paint_bg(ui);
                    let mut label = cue_label.clone();
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut label).desired_width(ui.available_width())
                    );
                    if resp.changed() {
                        if let Some(c) = app.cue_list.get_cue_mut(abs_idx) {
                            c.label = label;
                        }
                    }
                    if resp.clicked() { clicked_id = Some(cue_id); }
                    row_responses.push(resp);
                });

                // Col 4: info
                row.col(|ui| {
                    paint_bg(ui);
                    let text = if let Some(t) = &trigger_label {
                        format!("{} {}", info_text, t)
                    } else {
                        info_text
                    };
                    let (rect, resp) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click());
                    ui.painter().text(
                        rect.left_center() + egui::vec2(4.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        text,
                        egui::FontId::proportional(11.0),
                        egui::Color32::from_rgb(160, 160, 160),
                    );
                    row_responses.push(resp);
                });

                // Col 5: playback state
                row.col(|ui| {
                    paint_bg(ui);
                    let state_str = if is_lx_fading {
                        format!("{}{:.0}%", ph::PLAY, lx_fade.unwrap_or(0.0) * 100.0)
                    } else if is_lx_active {
                        ph::PAUSE.to_string()
                    } else if is_adjust_active {
                        #[cfg(feature = "audio")]
                        {
                            let pct = (adjust_progress.unwrap_or(0.0) * 100.0) as u32;
                            format!("{}{}%", ph::PLAY, pct)
                        }
                        #[cfg(not(feature = "audio"))]
                        String::new()
                    } else if is_audio_active {
                        #[cfg(feature = "audio")]
                        {
                            match row_audio_state {
                                Some(crate::audio::AudioCueState::FadingIn { progress }) =>
                                    format!("{}{:.0}%", ph::PLAY, progress * 100.0),
                                Some(crate::audio::AudioCueState::FadingOut { progress }) =>
                                    format!("{}{:.0}%", ph::PAUSE, (1.0 - progress) * 100.0),
                                Some(crate::audio::AudioCueState::Playing) => ph::PLAY.to_string(),
                                _ => String::new(),
                            }
                        }
                        #[cfg(not(feature = "audio"))]
                        String::new()
                    } else {
                        String::new()
                    };
                    let (rect, resp) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click());
                    if !state_str.is_empty() {
                        ui.painter().text(
                            rect.left_center() + egui::vec2(4.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            state_str,
                            egui::FontId::proportional(11.0),
                            egui::Color32::from_rgb(200, 200, 100),
                        );
                    }
                    row_responses.push(resp);
                });

                // Row click → selection
                if row_responses.iter().any(|r| r.clicked()) {
                    clicked_id = Some(cue_id);
                }

                // Context menu
                if let Some(first) = row_responses.first() {
                    let combined = row_responses.iter().skip(1).fold(first.clone(), |a, r| a.union(r.clone()));
                    combined.context_menu(|ui| {
                        if ui.button("Fire (Go To)").clicked() {
                            go_to_abs_idx = Some(abs_idx);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Edit in Properties").clicked() {
                            app.ui_state.selected_cue_id = Some(cue_id);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Delete").clicked() {
                            if app.cue_list.remove_cue(abs_idx).is_ok() {
                                app.ui_state.selected_cue_id = None;
                                app.ui_state.status_message = format!("Deleted cue {:.1}", cue_number);
                            }
                            ui.close_menu();
                        }
                    });
                }
            });
        });

    // Deferred actions (can't mutate app inside body closure)
    if let Some(id) = clicked_id {
        if selected_id == Some(id) {
            app.ui_state.selected_cue_id = None;
        } else {
            app.ui_state.selected_cue_id = Some(id);
            // Keep legacy fields in sync for properties panel
            if let Some(cue) = app.cue_list.find_by_id(id) {
                if cue.is_lighting() {
                    app.ui_state.selected_lighting_cue_id = Some(id);
                    app.ui_state.selected_audio_cue_id = None;
                } else {
                    app.ui_state.selected_audio_cue_id = Some(id);
                    app.ui_state.selected_lighting_cue_id = None;
                }
            }
        }
    }

    if let Some(abs_idx) = go_to_abs_idx {
        let num = app.cue_list.get_cue(abs_idx).map(|c| c.number).unwrap_or(0.0);
        app.go_to_cue(abs_idx);
        app.ui_state.status_message = format!("→ Q{:.1}", num);
    }

    render_footer(ui, app);
}

fn render_footer(ui: &mut Ui, app: &mut EasyCueApp) {
    let max_rect = ui.max_rect();
    let footer_height = 48.0;
    let footer_rect = egui::Rect::from_min_max(
        egui::pos2(max_rect.left(), max_rect.bottom() - footer_height),
        egui::pos2(max_rect.right(), max_rect.bottom()),
    );
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(footer_rect), |ui| {
        ui.separator();
        egui::Frame::new()
            .fill(ui.style().visuals.extreme_bg_color)
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Lighting state
                    let lx_str = match app.playback.fade_progress() {
                        Some(p) => format!("{}{}{:.0}%", ph::LIGHTBULB, ph::PLAY, p * 100.0),
                        None if app.playback.is_playing() => format!("{}{}", ph::LIGHTBULB, ph::PAUSE),
                        _ => format!("{}{}", ph::LIGHTBULB, ph::STOP),
                    };
                    ui.label(egui::RichText::new(lx_str).strong());
                    if let Some(id) = app.playback.current_cue_id() {
                        if let Some(c) = app.cue_list.find_by_id(id) {
                            ui.label(format!("Q{:.1} {}", c.number, c.label));
                        }
                    }

                    #[cfg(feature = "audio")]
                    {
                        ui.separator();
                        let count = app.audio_playback.active_count();
                        let multi = if count > 1 { format!(" ×{}", count) } else { String::new() };
                        let snd_str = match app.audio_playback.state() {
                            crate::audio::AudioCueState::Stopped =>
                                format!("{}{}", ph::SPEAKER_HIGH, ph::STOP),
                            crate::audio::AudioCueState::FadingIn { progress } =>
                                format!("{}{}{:.0}%{}", ph::SPEAKER_HIGH, ph::PLAY, progress * 100.0, multi),
                            crate::audio::AudioCueState::Playing =>
                                format!("{}{}{}", ph::SPEAKER_HIGH, ph::PLAY, multi),
                            crate::audio::AudioCueState::FadingOut { progress } =>
                                format!("{}{}{:.0}%{}", ph::SPEAKER_HIGH, ph::PAUSE, (1.0 - progress) * 100.0, multi),
                        };
                        ui.label(egui::RichText::new(snd_str).strong());
                        if let Some(id) = app.audio_playback.current_cue_id() {
                            if let Some(c) = app.cue_list.find_by_id(id) {
                                ui.label(format!("Q{:.1} {}", c.number, c.label));
                            }
                        }
                    }

                    ui.separator();

                    // Command line
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center).with_main_justify(true), |ui| {
                        ui.horizontal(|ui| {
                            let ctx_icon = match app.ui_state.command_context {
                                crate::command::CommandContext::Lighting => ph::LIGHTBULB,
                                crate::command::CommandContext::Sound => ph::SPEAKER_HIGH,
                                _ => ph::KEYBOARD,
                            };
                            ui.label(egui::RichText::new(ctx_icon).size(16.0));
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut app.ui_state.command_input)
                                    .desired_width(ui.available_width() - 80.0)
                                    .hint_text("Click channels...")
                                    .font(egui::TextStyle::Monospace)
                            );
                            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                crate::ui::execute_command_line(app);
                            }
                            if ui.button(ph::ARROW_BEND_DOWN_LEFT).clicked() { crate::ui::execute_command_line(app); }
                            if ui.button("✖").clicked() {
                                app.ui_state.command_input.clear();
                            }
                        });
                    });
                });
            });
    });
}
