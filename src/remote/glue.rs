//! Per-frame bridge between the remote server and the engine.
//!
//! `service_frame` is called once per egui frame from `App::update()`:
//! it executes queued client commands against the engine, then diffs
//! engine state against what was last published and pushes updates.

use super::protocol::*;
use super::RemoteServer;
use crate::app::EasyCueApp;
use crate::fixtures::profiles::FixtureParameter;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

/// Minimum interval between channel/playback pushes (~20 Hz) so 60 FPS fades
/// don't flood venue wifi.
const PUSH_INTERVAL: Duration = Duration::from_millis(50);
/// How often the (more expensive) structure diff runs.
const STRUCTURE_INTERVAL: Duration = Duration::from_millis(500);

/// Change-detection state. Lives inside `RemoteServer` so stopping the server
/// resets it and a restarted server re-sends everything.
pub struct Shadow {
    channels: HashMap<u16, [u8; 512]>,
    playback: PlaybackState,
    structure_hash: u64,
    structure_json: String,
    last_push: Instant,
    last_structure_check: Instant,
    /// True until the first publish — forces a full initial snapshot.
    dirty: bool,
}

impl Default for Shadow {
    fn default() -> Self {
        Self {
            channels: HashMap::new(),
            playback: PlaybackState::default(),
            structure_hash: 0,
            structure_json: String::new(),
            last_push: Instant::now() - PUSH_INTERVAL,
            last_structure_check: Instant::now() - STRUCTURE_INTERVAL,
            dirty: true,
        }
    }
}

/// Run the remote bridge for this frame. Takes the server out of the app to
/// avoid borrow conflicts while commands mutate app state.
pub fn service_frame(app: &mut EasyCueApp, ctx: &egui::Context) {
    let Some(mut server) = app.remote.take() else {
        return;
    };

    for msg in server.drain_commands() {
        execute(app, &server, msg);
    }
    publish(app, &mut server);

    // Keep state flowing to connected phones even when the desktop is idle.
    if server.client_count() > 0 {
        ctx.request_repaint_after(Duration::from_millis(150));
    }

    app.remote = Some(server);
}

// --- Command execution -----------------------------------------------------

fn execute(app: &mut EasyCueApp, server: &RemoteServer, msg: ClientMessage) {
    match msg {
        ClientMessage::CueGo => {
            app.go_next();
        }
        ClientMessage::CueBack => {
            app.go_back();
        }
        ClientMessage::CueStop => {
            app.playback.stop();
            #[cfg(feature = "audio")]
            app.audio_playback.stop_all();
            app.autofollow_timer = None;
            app.ui_state.status_message = "Stopped (remote)".to_string();
        }
        ClientMessage::CueGoto { number } => {
            app.goto_cue_by_number(number);
        }
        ClientMessage::SetChannels { universe, channels } => {
            let idx = (universe as usize).saturating_sub(1);
            if let Some(uni) = app.universes.get_mut(idx) {
                for cv in channels {
                    if let Err(e) = uni.set_channel(cv.channel, cv.value.min(100)) {
                        log::warn!("[remote] set_channel failed: {}", e);
                    }
                }
            }
        }
        ClientMessage::SetIntensity {
            fixture_ids,
            intensity,
        } => {
            for id in fixture_ids {
                set_fixture_intensity(app, id, intensity.clamp(0.0, 1.0));
            }
        }
        ClientMessage::SetParams { fixture_id, values } => {
            set_fixture_params(app, fixture_id, &values);
        }
        ClientMessage::CommandLine { text, context } => {
            let saved_context = app.ui_state.command_context;
            app.ui_state.command_context = match context {
                RemoteCmdContext::Channel => crate::command::CommandContext::General,
                RemoteCmdContext::Fixture => crate::command::CommandContext::Lighting,
            };
            app.ui_state.command_input = text.clone();
            crate::ui::execute_command_line(app);
            app.ui_state.command_context = saved_context;
            server.publish(envelope(
                "log",
                serde_json::json!({
                    "text": text,
                    "reply": app.ui_state.status_message,
                }),
            ));
        }
        ClientMessage::SetMaster { value } => {
            app.ui_state.lighting_master = value.clamp(0.0, 1.0);
        }
        ClientMessage::SetBlackout { active } => {
            app.ui_state.blackout_active = active;
            app.ui_state.status_message = if active {
                "BLACKOUT (remote)".to_string()
            } else {
                "Blackout released (remote)".to_string()
            };
        }
        ClientMessage::PatchAdd {
            label,
            profile_id,
            universe,
            start_address,
        } => {
            let result = app
                .fixtures
                .add_patch(label.clone(), profile_id, start_address, universe);
            report_patch_result(app, server, result.map(|id| format!("Patched #{} '{}'", id, label)));
        }
        ClientMessage::PatchUpdate {
            id,
            label,
            new_id,
            universe,
            start_address,
        } => {
            let result = update_patch(app, id, label, new_id, universe, start_address);
            report_patch_result(app, server, result.map(|_| format!("Updated fixture #{}", new_id)));
        }
        ClientMessage::PatchRemove { id } => {
            let result = app.fixtures.remove_patch(id);
            report_patch_result(app, server, result.map(|_| format!("Removed fixture #{}", id)));
        }
    }
}

/// Surface a patch operation's outcome on the desktop status bar and as a
/// remote `log` message (the patch page listens for errors there).
fn report_patch_result(app: &mut EasyCueApp, server: &RemoteServer, result: anyhow::Result<String>) {
    let (text, is_error) = match result {
        Ok(msg) => (msg, false),
        Err(e) => (format!("Patch error: {}", e), true),
    };
    app.ui_state.status_message = text.clone();
    server.publish(envelope(
        "log",
        serde_json::json!({ "text": "patch", "reply": text, "error": is_error }),
    ));
}

/// Apply a remote patch edit. Label edits in place; ID/universe/address
/// changes go through remove + re-add so the library's overlap validation
/// runs. On validation failure the original patch is restored.
fn update_patch(
    app: &mut EasyCueApp,
    id: usize,
    label: String,
    new_id: usize,
    universe: u16,
    start_address: u16,
) -> anyhow::Result<()> {
    let Some(original) = app.fixtures.patch_list().get_patch(id).cloned() else {
        anyhow::bail!("fixture #{} not found", id);
    };

    if new_id == original.id && universe == original.universe && start_address == original.start_address
    {
        if let Some(patch) = app.fixtures.patch_list_mut().get_patch_mut(id) {
            patch.label = label;
        }
        return Ok(());
    }

    app.fixtures.remove_patch(id)?;
    match app.fixtures.add_patch_with_id(
        new_id,
        label,
        original.profile_id.clone(),
        start_address,
        universe,
    ) {
        Ok(_) => Ok(()),
        Err(e) => {
            // Roll back so a failed edit never loses the fixture.
            let _ = app.fixtures.add_patch_with_id(
                original.id,
                original.label.clone(),
                original.profile_id.clone(),
                original.start_address,
                original.universe,
            );
            Err(e)
        }
    }
}

/// Set one fixture's intensity through the proper route: dedicated channel if
/// the profile has one, virtual intensity for RGB-only fixtures. Unlike the
/// command-line path this respects the fixture's patched universe.
fn set_fixture_intensity(app: &mut EasyCueApp, fixture_id: usize, intensity: f32) {
    let Some(patch) = app.fixtures.patch_list().get_patch(fixture_id).cloned() else {
        log::warn!("[remote] fixture {} not in patch", fixture_id);
        return;
    };
    let Some(profile) = app.fixtures.get_profile(&patch.profile_id).cloned() else {
        log::warn!(
            "[remote] fixture {}: profile '{}' missing",
            fixture_id,
            patch.profile_id
        );
        return;
    };
    let uni_idx = (patch.universe as usize).saturating_sub(1);
    let Some(universe) = app.universes.get_mut(uni_idx) else {
        return;
    };

    if profile.has_intensity() {
        if let Some(offset) = profile.get_parameter_offset(&FixtureParameter::Intensity) {
            let value = (intensity * 100.0).round() as u8;
            let _ = universe.set_channel(patch.start_address + offset, value.min(100));
        }
    } else if profile.is_rgb() {
        if let Err(e) = app
            .virtual_intensity
            .set_intensity(fixture_id, intensity, universe, &patch, &profile)
        {
            log::warn!(
                "[remote] virtual intensity for fixture {}: {}",
                fixture_id,
                e
            );
        }
    }
}

/// Write raw parameter values (profile channel offset → 0–100) for a fixture,
/// then re-sync virtual intensity ratios if color channels changed on an
/// RGB-only fixture. All color channels are re-read from the universe so
/// non-RGB colors (Amber/White/UV) keep their levels (see CLAUDE.md gotcha).
fn set_fixture_params(app: &mut EasyCueApp, fixture_id: usize, values: &HashMap<u16, u8>) {
    let Some(patch) = app.fixtures.patch_list().get_patch(fixture_id).cloned() else {
        return;
    };
    let Some(profile) = app.fixtures.get_profile(&patch.profile_id).cloned() else {
        return;
    };
    let uni_idx = (patch.universe as usize).saturating_sub(1);
    let Some(universe) = app.universes.get_mut(uni_idx) else {
        return;
    };

    let mut touched_color = false;
    for (&offset, &value) in values {
        if offset >= profile.channel_count {
            continue;
        }
        let param = profile
            .parameters
            .iter()
            .find(|p| p.channel_offset == offset)
            .map(|p| &p.parameter);
        if param.map(|p| p.is_color()).unwrap_or(false) {
            touched_color = true;
        }
        let _ = universe.set_channel(patch.start_address + offset, value.min(100));
    }

    if touched_color && !profile.has_intensity() && profile.is_rgb() {
        let mut color_values = HashMap::new();
        for pm in profile.color_parameters() {
            let ch = patch.start_address + pm.channel_offset;
            let value = universe.get_channel(ch).unwrap_or(0);
            color_values.insert(pm.parameter.clone(), value);
        }
        app.virtual_intensity.set_color(fixture_id, color_values);
    }
}

// --- State publishing --------------------------------------------------------

fn publish(app: &EasyCueApp, server: &mut RemoteServer) {
    let now = Instant::now();
    if now.duration_since(server.shadow.last_push) < PUSH_INTERVAL {
        return;
    }

    let mut anything_sent = false;

    // Structure (cues / patch / profiles / groups) — checked at 2 Hz.
    if server.shadow.dirty
        || now.duration_since(server.shadow.last_structure_check) >= STRUCTURE_INTERVAL
    {
        server.shadow.last_structure_check = now;
        let structure = build_structure(app);
        let json = serde_json::to_value(&structure).unwrap_or_default();
        let json_str = json.to_string();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        json_str.hash(&mut hasher);
        let hash = hasher.finish();
        if hash != server.shadow.structure_hash {
            server.shadow.structure_hash = hash;
            server.shadow.structure_json = json_str;
            server.publish(envelope("structure", json));
            anything_sent = true;
        }
    }

    // Channel levels for active universes — byte-compared, cheap.
    let active = active_universes(app);
    for &uni_id in &active {
        let idx = (uni_id as usize).saturating_sub(1);
        let Some(universe) = app.universes.get(idx) else {
            continue;
        };
        let current = *universe.channels();
        let changed = server
            .shadow
            .channels
            .get(&uni_id)
            .map(|last| *last != current)
            .unwrap_or(true);
        if changed {
            server.shadow.channels.insert(uni_id, current);
            server.publish(envelope(
                "channels",
                serde_json::json!({ "universe": uni_id, "values": current.to_vec() }),
            ));
            anything_sent = true;
        }
    }

    // Playback / masters / status line.
    let playback = build_playback(app);
    if playback != server.shadow.playback {
        server.shadow.playback = playback.clone();
        server.publish(envelope(
            "playback",
            serde_json::to_value(&playback).unwrap_or_default(),
        ));
        anything_sent = true;
    }

    if anything_sent || server.shadow.dirty {
        server.shadow.last_push = now;
        server.shadow.dirty = false;
        rebuild_snapshot(app, server, &active);
    }
}

fn rebuild_snapshot(app: &EasyCueApp, server: &RemoteServer, active: &[u16]) {
    let structure: serde_json::Value =
        serde_json::from_str(&server.shadow.structure_json).unwrap_or_default();
    let universes: Vec<serde_json::Value> = active
        .iter()
        .filter_map(|&uni_id| {
            let idx = (uni_id as usize).saturating_sub(1);
            app.universes
                .get(idx)
                .map(|u| serde_json::json!({ "universe": uni_id, "values": u.channels().to_vec() }))
        })
        .collect();
    let snapshot = envelope(
        "snapshot",
        serde_json::json!({
            "structure": structure,
            "universes": universes,
            "playback": serde_json::to_value(&server.shadow.playback).unwrap_or_default(),
        }),
    );
    server.set_snapshot(snapshot);
}

fn active_universes(app: &EasyCueApp) -> Vec<u16> {
    let mut ids: Vec<u16> = app
        .fixtures
        .patch_list()
        .patches()
        .iter()
        .map(|p| p.universe)
        .collect();
    ids.push(1);
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn build_playback(app: &EasyCueApp) -> PlaybackState {
    PlaybackState {
        current_index: app.cue_list.current_index(),
        next_index: app.cue_list.next_any_index(),
        playing: app.playback.is_playing(),
        progress: app.playback.fade_progress(),
        blackout: app.ui_state.blackout_active,
        master: app.ui_state.lighting_master,
        status: app.ui_state.status_message.clone(),
    }
}

fn build_structure(app: &EasyCueApp) -> Structure {
    let cues = app
        .cue_list
        .cues()
        .iter()
        .map(|cue| {
            let kind = if cue.is_lighting() {
                "lighting"
            } else {
                #[cfg(feature = "audio")]
                {
                    if cue.is_audio() {
                        "audio"
                    } else {
                        "adjust"
                    }
                }
                #[cfg(not(feature = "audio"))]
                {
                    "other"
                }
            };
            CueInfo {
                id: cue.id,
                number: cue.number,
                label: cue.label.clone(),
                kind,
                fade_up: cue.lighting_data().map(|d| d.fade_up),
                fade_down: cue.lighting_data().map(|d| d.fade_down),
                autofollow: cue.autofollow,
            }
        })
        .collect();

    let patch: Vec<PatchInfo> = app
        .fixtures
        .patch_list()
        .patches()
        .iter()
        .map(|p| PatchInfo {
            id: p.id,
            label: p.label.clone(),
            profile_id: p.profile_id.clone(),
            universe: p.universe,
            start_address: p.start_address,
        })
        .collect();

    // Whole profile library, not just patched profiles — the remote patch
    // page needs the full list to offer when adding fixtures.
    let mut profiles = HashMap::new();
    for (profile_id, profile) in app.fixtures.profiles() {
        let parameters = profile
            .parameters
            .iter()
            .map(|pm| ParamInfo {
                key: param_key(&pm.parameter),
                label: pm.parameter.short_label().to_string(),
                offset: pm.channel_offset,
                is_color: pm.parameter.is_color(),
                is_intensity: pm.parameter == FixtureParameter::Intensity,
            })
            .collect();
        profiles.insert(
            profile_id.clone(),
            ProfileInfo {
                name: profile.name.clone(),
                channel_count: profile.channel_count,
                has_intensity: profile.has_intensity(),
                is_rgb: profile.is_rgb(),
                parameters,
            },
        );
    }

    let groups = app
        .groups
        .groups
        .iter()
        .map(|g| GroupInfo {
            id: g.id,
            label: g.label.clone(),
            fixtures: g.fixture_ids.clone(),
        })
        .collect();

    Structure {
        show_title: app.show_title.clone(),
        cues,
        patch,
        profiles,
        groups,
        active_universes: active_universes(app),
    }
}

/// Stable string key for a fixture parameter ("red", "zoom", "custom:Ring").
fn param_key(param: &FixtureParameter) -> String {
    match serde_json::to_value(param) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(serde_json::Value::Object(map)) => map
            .get("custom")
            .and_then(|v| v.as_str())
            .map(|s| format!("custom:{}", s))
            .unwrap_or_else(|| "custom".to_string()),
        _ => "unknown".to_string(),
    }
}
