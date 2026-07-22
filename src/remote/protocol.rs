//! Wire protocol for the phone remote — plain serde types only.
//!
//! Everything here is deliberately decoupled from engine/fixture types (see
//! the dual-crate note in CLAUDE.md): the app layer translates between these
//! messages and real engine state in `glue.rs`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One channel write within a universe (values are 0–100, the internal range).
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelValue {
    pub channel: u16,
    pub value: u8,
}

/// Command line context requested by the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RemoteCmdContext {
    /// EOS "General" context: bare numbers address raw DMX channels.
    Channel,
    /// EOS "Lighting" context: bare numbers address patched fixtures.
    #[default]
    Fixture,
}

/// Client → server commands (WebSocket messages and REST bodies).
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ClientMessage {
    CueGo,
    CueBack,
    CueStop,
    CueGoto {
        number: f32,
    },
    /// Set raw DMX channels in one universe.
    SetChannels {
        universe: u16,
        channels: Vec<ChannelValue>,
    },
    /// Set fixture intensity (dedicated channel or virtual). 0.0–1.0.
    SetIntensity {
        fixture_ids: Vec<usize>,
        intensity: f32,
    },
    /// Set fixture parameters by profile channel offset (offset → 0–100 value).
    /// Color-capable offsets keep virtual intensity ratios in sync app-side.
    SetParams {
        fixture_id: usize,
        values: HashMap<u16, u8>,
    },
    /// Raw command line passthrough; result is echoed back as a `log` message.
    CommandLine {
        text: String,
        #[serde(default)]
        context: RemoteCmdContext,
    },
    /// Grand master 0.0–1.0.
    SetMaster {
        value: f32,
    },
    SetBlackout {
        active: bool,
    },
    /// Patch a new fixture (ID auto-assigned).
    PatchAdd {
        label: String,
        profile_id: String,
        universe: u16,
        start_address: u16,
    },
    /// Edit an existing patch. `new_id` renumbers the fixture; profile changes
    /// are not supported (delete + re-add instead, matching desktop behavior).
    PatchUpdate {
        id: usize,
        label: String,
        new_id: usize,
        universe: u16,
        start_address: u16,
    },
    PatchRemove {
        id: usize,
    },
}

/// REST body for POST /api/channel.
#[derive(Debug, Clone, Deserialize)]
pub struct RestChannelBody {
    #[serde(default = "default_universe")]
    pub universe: u16,
    pub channel: u16,
    pub value: u8,
}

fn default_universe() -> u16 {
    1
}

/// REST body for POST /api/command.
#[derive(Debug, Clone, Deserialize)]
pub struct RestCommandBody {
    pub text: String,
    #[serde(default)]
    pub context: RemoteCmdContext,
}

// --- Server → client payloads -------------------------------------------

/// Live playback / master state. Small and diffed every frame.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct PlaybackState {
    /// Index of the last-fired cue in the cue list, if any.
    pub current_index: Option<usize>,
    /// Index of the on-deck cue (what GO will fire).
    pub next_index: Option<usize>,
    pub playing: bool,
    /// Crossfade progress 0.0–1.0 while a lighting fade runs.
    pub progress: Option<f32>,
    pub blackout: bool,
    /// Grand master 0.0–1.0.
    pub master: f32,
    /// Desktop status line (mirrors the bottom status bar).
    pub status: String,
}

/// One cue row for the remote cue list.
#[derive(Debug, Clone, Serialize)]
pub struct CueInfo {
    pub id: u32,
    pub number: f32,
    pub label: String,
    /// "lighting" | "audio" | "adjust"
    pub kind: &'static str,
    pub fade_up: Option<f32>,
    pub fade_down: Option<f32>,
    pub autofollow: Option<f32>,
}

/// One patched fixture.
#[derive(Debug, Clone, Serialize)]
pub struct PatchInfo {
    pub id: usize,
    pub label: String,
    pub profile_id: String,
    pub universe: u16,
    pub start_address: u16,
}

/// One parameter of a fixture profile, flattened for the client.
#[derive(Debug, Clone, Serialize)]
pub struct ParamInfo {
    /// Stable key, e.g. "red", "zoom", "custom:Ring". Display + routing only.
    pub key: String,
    /// Short slider label from the profile ("R", "Zoom", …).
    pub label: String,
    pub offset: u16,
    pub is_color: bool,
    pub is_intensity: bool,
}

/// Fixture profile summary for the client.
#[derive(Debug, Clone, Serialize)]
pub struct ProfileInfo {
    pub name: String,
    pub channel_count: u16,
    pub has_intensity: bool,
    pub is_rgb: bool,
    pub parameters: Vec<ParamInfo>,
}

/// A fixture group.
#[derive(Debug, Clone, Serialize)]
pub struct GroupInfo {
    pub id: u32,
    pub label: String,
    pub fixtures: Vec<usize>,
}

/// Slow-changing show structure — resent wholesale when anything in it changes.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Structure {
    pub show_title: String,
    pub cues: Vec<CueInfo>,
    pub patch: Vec<PatchInfo>,
    pub profiles: HashMap<String, ProfileInfo>,
    pub groups: Vec<GroupInfo>,
    /// Universe IDs that carry patched fixtures (universe 1 always included).
    pub active_universes: Vec<u16>,
}

/// Envelope for server → client messages.
pub fn envelope(msg_type: &str, payload: serde_json::Value) -> String {
    serde_json::json!({ "type": msg_type, "payload": payload }).to_string()
}
