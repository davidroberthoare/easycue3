//! Lighting groups panel UI
//!
//! Displays all groups and lets the operator create, rename, delete, and edit
//! the fixture membership of each group.  The fixture list is entered as a
//! plain comma-separated string (e.g. "1, 2, 3") for speed.

use crate::app::EasyCueApp;
use crate::groups::Group;
use egui::{Color32, RichText, ScrollArea};
use egui_phosphor::regular as ph;

/// Ephemeral state for the groups panel (not saved to disk).
#[derive(Default)]
pub struct GroupsPanelState {
    /// Inline edit buffer: maps group id → current text in the fixtures field.
    pub fixture_inputs: std::collections::HashMap<u32, String>,
}

/// Render the Groups panel.
pub fn render_groups_panel(ui: &mut egui::Ui, app: &mut EasyCueApp, state: &mut GroupsPanelState) {
    ui.heading("Lighting Groups");

    ui.horizontal(|ui| {
        if ui.button(format!("{} Add Group", ph::PLUS)).clicked() {
            app.groups.add_group();
        }

        ui.separator();

        ui.label(egui::RichText::new(
            format!("{} group{}", app.groups.groups.len(),
                if app.groups.groups.len() == 1 { "" } else { "s" })
        ).color(Color32::GRAY));
    });

    ui.separator();

    // Ensure the fixture input buffers are in sync with the group list.
    // Add missing entries; stale entries from deleted groups are harmless.
    for group in &app.groups.groups {
        state.fixture_inputs.entry(group.id).or_insert_with(|| {
            Group::fixtures_to_string(&group.fixture_ids)
        });
    }

    let mut to_remove: Option<u32> = None;
    let mut to_select: Option<Vec<usize>> = None;

    ScrollArea::vertical().show(ui, |ui| {
        egui_extras::TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(egui_extras::Column::exact(45.0))           // Group #
            .column(egui_extras::Column::initial(120.0).at_least(80.0)) // Label
            .column(egui_extras::Column::remainder().at_least(120.0))   // Fixtures
            .column(egui_extras::Column::exact(80.0))           // Actions
            .header(20.0, |mut header| {
                header.col(|ui| { ui.strong("Group #"); });
                header.col(|ui| { ui.strong("Label"); });
                header.col(|ui| { ui.strong("Fixtures (comma-separated)"); });
                header.col(|ui| { ui.strong(""); });
            })
            .body(|mut body| {
                // Snapshot IDs so we can mutate inside the loop.
                let group_ids: Vec<u32> = app.groups.groups.iter().map(|g| g.id).collect();

                for gid in group_ids {
                    body.row(26.0, |mut row| {
                        // ── Group # ───────────────────────────────────────────
                        row.col(|ui| {
                            ui.label(format!("G{}", gid));
                        });

                        // ── Label (inline edit) ────────────────────────────────
                        row.col(|ui| {
                            if let Some(group) = app.groups.get_group_mut(gid) {
                                let response = ui.add(
                                    egui::TextEdit::singleline(&mut group.label)
                                        .desired_width(ui.available_width())
                                        .hint_text("(label)")
                                );
                                let _ = response;
                            }
                        });

                        // ── Fixture IDs text field ─────────────────────────────
                        row.col(|ui| {
                            let buf = state.fixture_inputs.entry(gid).or_default();
                            let response = ui.add(
                                egui::TextEdit::singleline(buf)
                                    .desired_width(ui.available_width())
                                    .hint_text("e.g. 1, 2, 3")
                                    .code_editor()
                            );
                            if response.changed() || response.lost_focus() {
                                // Commit parsed IDs back into the group.
                                let parsed = Group::parse_fixtures_string(buf);
                                if let Some(group) = app.groups.get_group_mut(gid) {
                                    group.fixture_ids = parsed;
                                }
                                // Re-format so the field stays canonical.
                                if response.lost_focus() {
                                    if let Some(group) = app.groups.get_group(gid) {
                                        *buf = Group::fixtures_to_string(&group.fixture_ids);
                                    }
                                }
                            }
                        });

                        // ── Actions ───────────────────────────────────────────
                        row.col(|ui| {
                            ui.horizontal(|ui| {
                                let fixture_ids = app.groups.get_group(gid)
                                    .map(|g| g.fixture_ids.clone())
                                    .unwrap_or_default();

                                let has_fixtures = !fixture_ids.is_empty();
                                if ui
                                    .add_enabled(has_fixtures, egui::Button::new("Select").small())
                                    .on_hover_text("Select these fixtures")
                                    .clicked()
                                {
                                    to_select = Some(fixture_ids);
                                }

                                if ui
                                    .add(egui::Button::new(
                                        RichText::new(ph::TRASH).color(Color32::from_rgb(200, 80, 80))
                                    ).small())
                                    .on_hover_text("Delete group")
                                    .clicked()
                                {
                                    to_remove = Some(gid);
                                }
                            });
                        });
                    });
                }
            });
    });

    // ── Deferred mutations ─────────────────────────────────────────────────────
    if let Some(gid) = to_remove {
        app.groups.remove_group(gid);
        state.fixture_inputs.remove(&gid);
        app.ui_state.status_message = format!("Deleted group G{}", gid);
    }

    if let Some(fixture_ids) = to_select {
        app.ui_state.selected_fixtures.clear();
        for fid in &fixture_ids {
            app.ui_state.selected_fixtures.insert(*fid);
        }
        app.ui_state.status_message = format!("Selected {} fixture{}", fixture_ids.len(),
            if fixture_ids.len() == 1 { "" } else { "s" });
    }

    // ── Usage hint ─────────────────────────────────────────────────────────────
    if app.groups.groups.is_empty() {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("No groups yet.").color(Color32::GRAY));
            ui.add_space(4.0);
            ui.label(RichText::new("Click \"+ Add Group\" to create one.").color(Color32::GRAY).small());
            ui.add_space(4.0);
            ui.label(RichText::new("In the Channels or Magic Sheet panel, type \"g1@50\" to set Group 1 to 50%.").color(Color32::GRAY).small());
        });
    }
}
