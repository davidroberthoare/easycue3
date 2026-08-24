//! Custom fixture profile editor popup

use crate::app::EasyCueApp;
use crate::fixtures::profiles::{FixtureParameter, FixtureProfile, ParameterMapping};
use egui::Context;
use egui_phosphor::regular as ph;

/// Editable row for one parameter mapping
#[derive(Clone)]
pub struct EditableParam {
    pub parameter: FixtureParameter,
    /// Text buffer for the custom name when parameter == Other/Custom
    pub custom_name: String,
    pub offset_str: String,
    pub default_str: String,
}

impl EditableParam {
    fn new(param: FixtureParameter, offset: u16) -> Self {
        let custom_name = if let FixtureParameter::Custom(ref s) = param {
            s.clone()
        } else {
            String::new()
        };
        Self {
            parameter: param,
            custom_name,
            offset_str: offset.to_string(),
            default_str: String::new(),
        }
    }

    fn from_mapping(m: &ParameterMapping) -> Self {
        let custom_name = if let FixtureParameter::Custom(ref s) = m.parameter {
            s.clone()
        } else {
            String::new()
        };
        Self {
            parameter: m.parameter.clone(),
            custom_name,
            offset_str: m.channel_offset.to_string(),
            default_str: m.default_value.map(|v| v.to_string()).unwrap_or_default(),
        }
    }
}

/// Transient state for the fixture editor window
pub struct FixtureEditorState {
    pub selected_id: Option<String>,
    pub is_new: bool,
    /// Original ID before any edits (to detect renames)
    pub original_id: Option<String>,

    pub edit_name: String,
    pub edit_id: String,
    pub edit_manufacturer: String,
    pub edit_notes: String,
    pub edit_params: Vec<EditableParam>,

    /// Feedback message shown inside the editor: (text, is_error)
    pub message: Option<(String, bool)>,
    /// Suppress ID auto-fill after the first name keystroke
    pub name_changed: bool,

    /// Profiles loaded from an Import that haven't been committed yet.
    pub pending_imports: Vec<FixtureProfile>,
    /// Collision found while importing; a modal asks replace-or-skip.
    pub import_collision: Option<ImportCollision>,
}

/// A name/id collision encountered while importing a profile. The profile to
/// import is kept here (removed from `pending_imports`) until the user decides.
pub struct ImportCollision {
    pub profile: FixtureProfile,
    /// Name of the existing profile it collides with.
    pub existing_name: String,
    /// Whether the existing profile is a user profile (vs bundled).
    pub existing_is_user: bool,
}

impl Default for FixtureEditorState {
    fn default() -> Self {
        Self {
            selected_id: None,
            is_new: false,
            original_id: None,
            edit_name: String::new(),
            edit_id: String::new(),
            edit_manufacturer: String::new(),
            edit_notes: String::new(),
            edit_params: Vec::new(),
            message: None,
            name_changed: false,
            pending_imports: Vec::new(),
            import_collision: None,
        }
    }
}

impl FixtureEditorState {
    fn load_profile(&mut self, profile: &FixtureProfile) {
        self.edit_name = profile.name.clone();
        self.edit_id = profile.id.clone();
        self.edit_manufacturer = profile.manufacturer.clone().unwrap_or_default();
        self.edit_notes = profile.notes.clone().unwrap_or_default();
        self.edit_params = profile
            .parameters
            .iter()
            .map(EditableParam::from_mapping)
            .collect();
        self.original_id = Some(profile.id.clone());
        self.is_new = false;
        self.message = None;
        self.name_changed = false;
    }

    fn blank_new(&mut self) {
        self.edit_name = String::new();
        self.edit_id = String::new();
        self.edit_manufacturer = String::new();
        self.edit_notes = String::new();
        self.edit_params = Vec::new();
        self.original_id = None;
        self.is_new = true;
        self.selected_id = None;
        self.message = None;
        self.name_changed = false;
    }

    fn slug_from_name(name: &str) -> String {
        name.to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>()
            .trim_matches('_')
            .to_string()
    }

    /// Channel count = highest offset + 1, minimum 1.
    fn computed_channel_count(&self) -> u16 {
        self.edit_params
            .iter()
            .filter_map(|ep| ep.offset_str.trim().parse::<u16>().ok())
            .max()
            .map(|m| m + 1)
            .unwrap_or(1)
    }

    fn build_profile(&self) -> Result<FixtureProfile, String> {
        let id = self.edit_id.trim().to_string();
        if id.is_empty() {
            return Err("Profile ID cannot be empty".to_string());
        }
        let name = self.edit_name.trim().to_string();
        if name.is_empty() {
            return Err("Profile name cannot be empty".to_string());
        }

        let channel_count = self.computed_channel_count();

        let mut parameters = Vec::new();
        for (i, ep) in self.edit_params.iter().enumerate() {
            let offset: u16 = ep
                .offset_str
                .trim()
                .parse()
                .map_err(|_| format!("Row {}: offset must be a number", i + 1))?;
            if offset >= channel_count {
                return Err(format!(
                    "Row {}: offset {} is out of range (channel count is {})",
                    i + 1,
                    offset,
                    channel_count
                ));
            }
            let default_value: Option<u8> = if ep.default_str.trim().is_empty() {
                None
            } else {
                Some(
                    ep.default_str
                        .trim()
                        .parse()
                        .map_err(|_| format!("Row {}: default must be 0–100 or blank", i + 1))?,
                )
            };
            // Resolve Custom parameter with the live custom_name text
            let parameter = if matches!(ep.parameter, FixtureParameter::Custom(_)) {
                let label = ep.custom_name.trim().to_string();
                if label.is_empty() {
                    return Err(format!("Row {}: Other parameter needs a name", i + 1));
                }
                FixtureParameter::Custom(label)
            } else {
                ep.parameter.clone()
            };
            parameters.push(ParameterMapping {
                parameter,
                channel_offset: offset,
                default_value,
            });
        }

        Ok(FixtureProfile {
            id,
            name,
            manufacturer: if self.edit_manufacturer.trim().is_empty() {
                None
            } else {
                Some(self.edit_manufacturer.trim().to_string())
            },
            channel_count,
            parameters,
            notes: if self.edit_notes.trim().is_empty() {
                None
            } else {
                Some(self.edit_notes.trim().to_string())
            },
        })
    }
}

/// Standard parameter types shown in the dropdown (Custom handled separately as "Other")
const STANDARD_PARAMETERS: &[FixtureParameter] = &[
    FixtureParameter::Intensity,
    FixtureParameter::Red,
    FixtureParameter::Green,
    FixtureParameter::Blue,
    FixtureParameter::Amber,
    FixtureParameter::White,
    FixtureParameter::Uv,
    FixtureParameter::Strobe,
    FixtureParameter::Pan,
    FixtureParameter::PanFine,
    FixtureParameter::Tilt,
    FixtureParameter::TiltFine,
    FixtureParameter::Iris,
    FixtureParameter::Focus,
    FixtureParameter::Zoom,
    FixtureParameter::Prism,
    FixtureParameter::Frost,
    FixtureParameter::Gobo,
];

fn param_label(p: &FixtureParameter) -> &'static str {
    match p {
        FixtureParameter::Intensity => "Intensity",
        FixtureParameter::Red => "Red",
        FixtureParameter::Green => "Green",
        FixtureParameter::Blue => "Blue",
        FixtureParameter::Amber => "Amber",
        FixtureParameter::White => "White",
        FixtureParameter::Uv => "UV",
        FixtureParameter::Strobe => "Strobe",
        FixtureParameter::Pan => "Pan",
        FixtureParameter::PanFine => "Pan Fine",
        FixtureParameter::Tilt => "Tilt",
        FixtureParameter::TiltFine => "Tilt Fine",
        FixtureParameter::Iris => "Iris",
        FixtureParameter::Focus => "Focus",
        FixtureParameter::Zoom => "Zoom",
        FixtureParameter::Prism => "Prism",
        FixtureParameter::Frost => "Frost",
        FixtureParameter::Gobo => "Gobo",
        FixtureParameter::Custom(_) => "Other",
    }
}

/// Write a profile to the user profiles directory and select it in the editor.
fn commit_import(app: &mut EasyCueApp, profile: FixtureProfile) {
    let id = profile.id.clone();
    match app.fixtures.save_user_profile(profile, None) {
        Ok(_) => {
            app.ui_state.status_message = format!("Imported fixture profile '{}'", id);
            let profile = app.fixtures.get_profile(&id).cloned();
            if let Some(profile) = profile {
                app.fixture_editor.load_profile(&profile);
                app.fixture_editor.selected_id = Some(id.clone());
                app.fixture_editor.is_new = false;
            }
        }
        Err(e) => {
            app.ui_state.status_message = format!("Import of '{}' failed: {}", id, e);
        }
    }
}

/// Find an existing profile (bundled or user) that `profile` collides with —
/// same ID, or a name match ignoring case. Returns (existing id, name, is_user).
fn find_collision(
    fixtures: &crate::fixtures::FixtureLibrary,
    profile: &FixtureProfile,
) -> Option<(String, String, bool)> {
    for (id, existing) in fixtures.profiles() {
        if *id == profile.id || existing.name.eq_ignore_ascii_case(&profile.name) {
            return Some((
                id.clone(),
                existing.name.clone(),
                fixtures.is_user_profile(id),
            ));
        }
    }
    None
}

/// Process pending imports: commit any that don't collide with an existing
/// profile; stop at the first collision and surface it via `import_collision`.
fn advance_imports(app: &mut EasyCueApp) {
    while !app.fixture_editor.pending_imports.is_empty() {
        let profile = app.fixture_editor.pending_imports.remove(0);
        match find_collision(&app.fixtures, &profile) {
            Some((_, existing_name, is_user)) => {
                app.fixture_editor.import_collision = Some(ImportCollision {
                    profile,
                    existing_name,
                    existing_is_user: is_user,
                });
                return;
            }
            None => commit_import(app, profile),
        }
    }
    app.fixture_editor.import_collision = None;
}

/// How the user resolved an import collision.
enum ImportDecision {
    Replace,
    Skip,
    ReplaceAll,
    SkipAll,
}

pub fn render_fixture_editor(ctx: &Context, app: &mut EasyCueApp) {
    if !app.ui_state.show_fixture_editor {
        return;
    }

    let mut open = true;
    let mut save_profile: Option<(FixtureProfile, Option<String>)> = None;
    let mut delete_id: Option<String> = None;
    let mut select_id: Option<String> = None;
    let mut create_new = false;
    let mut export_profile: Option<FixtureProfile> = None;
    let mut import_paths: Vec<std::path::PathBuf> = Vec::new();
    let mut import_decision: Option<ImportDecision> = None;

    egui::Window::new("Custom Fixture Profiles")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([700.0, 480.0])
        .min_size([560.0, 360.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let state = &mut app.fixture_editor;
            let fixtures = &app.fixtures;

            ui.horizontal_top(|ui| {
                // ── Left panel: profile list ──────────────────────────────
                ui.vertical(|ui| {
                    ui.set_min_width(160.0);
                    ui.set_max_width(160.0);

                    ui.strong("Your Profiles");
                    ui.add_space(4.0);

                    let user_ids = fixtures.user_profile_ids();

                    if user_ids.is_empty() {
                        ui.label(
                            egui::RichText::new("No custom profiles yet")
                                .italics()
                                .color(egui::Color32::GRAY)
                                .small(),
                        );
                    } else {
                        egui::ScrollArea::vertical()
                            .id_salt("fixture_profile_list")
                            .max_height(360.0)
                            .show(ui, |ui| {
                                for id in &user_ids {
                                    let is_sel = state.selected_id.as_deref() == Some(id.as_str());
                                    let profile_name = fixtures
                                        .get_profile(id)
                                        .map(|p| p.name.as_str())
                                        .unwrap_or(id.as_str());
                                    if ui.selectable_label(is_sel, profile_name).clicked() {
                                        select_id = Some(id.clone());
                                    }
                                }
                            });
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    if ui.button(format!("{} New Profile", ph::PLUS)).clicked() {
                        create_new = true;
                    }

                    ui.add_space(4.0);
                    if ui
                        .button(format!("{} Import…", ph::DOWNLOAD_SIMPLE))
                        .on_hover_text("Import profile JSON file(s) from disk")
                        .clicked()
                    {
                        if let Some(paths) = rfd::FileDialog::new()
                            .set_title("Import Fixture Profiles")
                            .add_filter("Fixture Profile", &["json"])
                            .pick_files()
                        {
                            import_paths = paths;
                        }
                    }

                    if let Some(sel) = &state.selected_id {
                        ui.add_space(4.0);
                        if ui
                            .button(format!("{} Export…", ph::UPLOAD_SIMPLE))
                            .on_hover_text(
                                "Save this profile as a JSON file to move to another machine",
                            )
                            .clicked()
                        {
                            if let Some(p) = fixtures.get_profile(sel).cloned() {
                                export_profile = Some(p);
                            }
                        }
                    }

                    if let Some(sel) = &state.selected_id {
                        ui.add_space(4.0);
                        let del_btn = egui::Button::new(
                            egui::RichText::new(format!("{} Delete", ph::TRASH))
                                .color(egui::Color32::from_rgb(220, 80, 80)),
                        );
                        if ui.add(del_btn).clicked() {
                            delete_id = Some(sel.clone());
                        }
                    }
                });

                ui.separator();

                // ── Right panel: editor ───────────────────────────────────
                ui.vertical(|ui| {
                    if !state.is_new && state.selected_id.is_none() {
                        ui.add_space(16.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new(
                                    "Select a profile to edit, or create a new one.",
                                )
                                .italics()
                                .color(egui::Color32::GRAY),
                            );
                        });
                        return;
                    }

                    // Metadata fields
                    egui::Grid::new("fixture_meta_grid")
                        .num_columns(2)
                        .spacing([8.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("Name:");
                            let name_resp = ui.add(
                                egui::TextEdit::singleline(&mut state.edit_name)
                                    .desired_width(220.0),
                            );
                            if name_resp.changed() && !state.name_changed {
                                state.edit_id =
                                    FixtureEditorState::slug_from_name(&state.edit_name);
                            }
                            if name_resp.changed() {
                                state.name_changed = true;
                            }
                            ui.end_row();

                            ui.label("ID:");
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut state.edit_id)
                                        .desired_width(160.0),
                                );
                                ui.label(
                                    egui::RichText::new("(unique, no spaces)")
                                        .small()
                                        .color(egui::Color32::GRAY),
                                );
                            });
                            ui.end_row();

                            ui.label("Manufacturer:");
                            ui.add(
                                egui::TextEdit::singleline(&mut state.edit_manufacturer)
                                    .desired_width(220.0)
                                    .hint_text("optional"),
                            );
                            ui.end_row();

                            ui.label("Notes:");
                            ui.add(
                                egui::TextEdit::singleline(&mut state.edit_notes)
                                    .desired_width(220.0)
                                    .hint_text("optional"),
                            );
                            ui.end_row();

                            ui.label("Channel count:");
                            let ch = state.computed_channel_count();
                            ui.label(
                                egui::RichText::new(format!("{} (auto)", ch))
                                    .color(egui::Color32::GRAY),
                            );
                            ui.end_row();
                        });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.strong("Parameters");
                    ui.add_space(4.0);

                    // Parameters table — 5 columns when an "Other" row is present,
                    // otherwise 4. We always render 5 to keep alignment consistent.
                    let mut remove_idx: Option<usize> = None;
                    egui::Grid::new("fixture_params_grid")
                        .num_columns(5)
                        .spacing([6.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.strong("Parameter");
                            ui.strong("Name"); // custom name column — blank for standard params
                            ui.strong("Offset");
                            ui.strong("Default");
                            ui.strong(""); // remove button
                            ui.end_row();

                            for (i, ep) in state.edit_params.iter_mut().enumerate() {
                                let is_custom = matches!(ep.parameter, FixtureParameter::Custom(_));
                                let display = param_label(&ep.parameter);

                                // Type dropdown
                                egui::ComboBox::from_id_salt(format!("param_type_{}", i))
                                    .selected_text(display)
                                    .width(100.0)
                                    .show_ui(ui, |ui| {
                                        for p in STANDARD_PARAMETERS {
                                            let selected = &ep.parameter == p;
                                            if ui
                                                .selectable_label(selected, param_label(p))
                                                .clicked()
                                            {
                                                ep.parameter = p.clone();
                                            }
                                        }
                                        ui.separator();
                                        if ui.selectable_label(is_custom, "Other…").clicked() {
                                            ep.parameter =
                                                FixtureParameter::Custom(ep.custom_name.clone());
                                        }
                                    });

                                // Custom name field (only shown for Other rows; blank label otherwise)
                                if is_custom {
                                    let resp = ui.add(
                                        egui::TextEdit::singleline(&mut ep.custom_name)
                                            .desired_width(80.0)
                                            .hint_text("e.g. Speed"),
                                    );
                                    if resp.changed() {
                                        ep.parameter =
                                            FixtureParameter::Custom(ep.custom_name.clone());
                                    }
                                } else {
                                    ui.label("");
                                }

                                ui.add(
                                    egui::TextEdit::singleline(&mut ep.offset_str)
                                        .desired_width(40.0),
                                );
                                ui.add(
                                    egui::TextEdit::singleline(&mut ep.default_str)
                                        .desired_width(40.0)
                                        .hint_text("–"),
                                );
                                if ui.small_button(ph::X).clicked() {
                                    remove_idx = Some(i);
                                }
                                ui.end_row();
                            }
                        });

                    if let Some(idx) = remove_idx {
                        state.edit_params.remove(idx);
                    }

                    ui.add_space(4.0);
                    if ui
                        .small_button(format!("{} Add Parameter", ph::PLUS))
                        .clicked()
                    {
                        let next_offset = state.edit_params.len() as u16;
                        state
                            .edit_params
                            .push(EditableParam::new(FixtureParameter::Intensity, next_offset));
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    if let Some((msg, is_err)) = &state.message {
                        let color = if *is_err {
                            egui::Color32::from_rgb(220, 80, 80)
                        } else {
                            egui::Color32::from_rgb(80, 200, 120)
                        };
                        ui.label(egui::RichText::new(msg).color(color).small());
                        ui.add_space(4.0);
                    }

                    ui.horizontal(|ui| {
                        if ui.button("  Save  ").clicked() {
                            match state.build_profile() {
                                Ok(profile) => {
                                    let old_id = state.original_id.clone();
                                    save_profile = Some((profile, old_id));
                                }
                                Err(e) => {
                                    state.message = Some((e, true));
                                }
                            }
                        }
                        if ui.button("Revert").clicked() {
                            if let Some(id) = &state.selected_id.clone() {
                                if let Some(p) = fixtures.get_profile(id) {
                                    let p = p.clone();
                                    state.load_profile(&p);
                                }
                            } else if state.is_new {
                                state.blank_new();
                            }
                        }
                    });
                });
            });
        });

    // ── Import collision modal: replace or skip? ───────────────────────────
    if let Some(collision) = &app.fixture_editor.import_collision {
        egui::Modal::new(egui::Id::new("import_collision_modal")).show(ctx, |ui| {
            ui.set_min_width(400.0);
            ui.heading("Profile Already Exists");
            ui.add_space(6.0);
            ui.label(format!(
                "Import \"{}\" (id: {})?",
                collision.profile.name, collision.profile.id
            ));
            let source = if collision.existing_is_user {
                "your custom profiles"
            } else {
                "the bundled profiles"
            };
            ui.label(
                egui::RichText::new(format!(
                    "A profile named \"{}\" already exists in {}.",
                    collision.existing_name, source
                ))
                .color(egui::Color32::from_rgb(200, 140, 60)),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let replace_btn = egui::Button::new(egui::RichText::new("Replace").strong());
                if ui.add(replace_btn).clicked() {
                    import_decision = Some(ImportDecision::Replace);
                }
                if ui.button("Skip").clicked() {
                    import_decision = Some(ImportDecision::Skip);
                }
            });
            ui.horizontal(|ui| {
                if ui.small_button("Replace all remaining").clicked() {
                    import_decision = Some(ImportDecision::ReplaceAll);
                }
                if ui.small_button("Skip all remaining").clicked() {
                    import_decision = Some(ImportDecision::SkipAll);
                }
            });
        });
    }

    // Apply the import decision (one per frame).
    match import_decision {
        Some(ImportDecision::Replace) => {
            if let Some(collision) = app.fixture_editor.import_collision.take() {
                commit_import(app, collision.profile);
            }
            advance_imports(app);
        }
        Some(ImportDecision::Skip) => {
            app.fixture_editor.import_collision = None;
            advance_imports(app);
        }
        Some(ImportDecision::ReplaceAll) => {
            let mut remaining = Vec::new();
            if let Some(collision) = app.fixture_editor.import_collision.take() {
                remaining.push(collision.profile);
            }
            remaining.append(&mut app.fixture_editor.pending_imports);
            for profile in remaining {
                commit_import(app, profile);
            }
        }
        Some(ImportDecision::SkipAll) => {
            app.fixture_editor.import_collision = None;
            app.fixture_editor.pending_imports.clear();
        }
        None => {}
    }

    // ── Export ─────────────────────────────────────────────────────────────
    if let Some(profile) = export_profile {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Export Fixture Profile")
            .add_filter("Fixture Profile", &["json"])
            .set_file_name(format!("{}.json", profile.id))
            .save_file()
        {
            match serde_json::to_string_pretty(&profile) {
                Ok(json) => match std::fs::write(&path, json) {
                    Ok(_) => {
                        app.ui_state.status_message = format!(
                            "Exported fixture profile '{}' to {}",
                            profile.id,
                            path.display()
                        );
                    }
                    Err(e) => {
                        app.ui_state.status_message = format!("Export failed: {}", e);
                    }
                },
                Err(e) => {
                    app.ui_state.status_message = format!("Export failed: {}", e);
                }
            }
        }
    }

    // ── Import ─────────────────────────────────────────────────────────────
    if !import_paths.is_empty() {
        let mut skipped = Vec::new();
        for path in import_paths {
            let label = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            match FixtureProfile::load_from_file(&path) {
                Ok(profile) => app.fixture_editor.pending_imports.push(profile),
                Err(e) => skipped.push(format!("{}: {}", label, e)),
            }
        }
        if !skipped.is_empty() {
            app.ui_state.status_message = format!(
                "Import: {} file(s) skipped — {}",
                skipped.len(),
                skipped.join("; ")
            );
        }
        advance_imports(app);
    }

    if create_new {
        app.fixture_editor.blank_new();
    }

    if let Some(id) = select_id {
        if let Some(profile) = app.fixtures.get_profile(&id).cloned() {
            app.fixture_editor.load_profile(&profile);
            app.fixture_editor.selected_id = Some(id);
        }
    }

    if let Some((profile, old_id)) = save_profile {
        let new_id = profile.id.clone();
        match app.fixtures.save_user_profile(profile, old_id.as_deref()) {
            Ok(_) => {
                app.fixture_editor.selected_id = Some(new_id.clone());
                app.fixture_editor.original_id = Some(new_id.clone());
                app.fixture_editor.is_new = false;
                app.fixture_editor.message = Some(("Saved.".to_string(), false));
                app.ui_state.status_message = format!("Saved fixture profile '{}'", new_id);
            }
            Err(e) => {
                app.fixture_editor.message = Some((format!("Save failed: {}", e), true));
            }
        }
    }

    if let Some(id) = delete_id {
        match app.fixtures.delete_user_profile(&id) {
            Ok(_) => {
                app.fixture_editor.selected_id = None;
                app.fixture_editor.is_new = false;
                app.fixture_editor.message = None;
                app.fixture_editor.edit_name.clear();
                app.fixture_editor.edit_id.clear();
                app.fixture_editor.edit_params.clear();
                app.ui_state.status_message = format!("Deleted fixture profile '{}'", id);
            }
            Err(e) => {
                app.fixture_editor.message = Some((format!("Delete failed: {}", e), true));
            }
        }
    }

    if !open {
        app.ui_state.show_fixture_editor = false;
    }
}
