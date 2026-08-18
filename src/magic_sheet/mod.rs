//! Magic sheet — freeform fixture-layout canvas
//!
//! Serialisable data structures stored inside the show file.
//! UI rendering lives in `src/ui/magic_sheet.rs`.

use serde::{Deserialize, Serialize};

/// Visual shape type. Add new variants here; the UI will pick them up automatically
/// once a renderer arm is added in `ui/magic_sheet.rs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ShapeKind {
    #[default]
    Rectangle,
    Circle,
    Diamond,
    Arrow,
}

impl std::fmt::Display for ShapeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShapeKind::Rectangle => write!(f, "Rect"),
            ShapeKind::Circle => write!(f, "Circle"),
            ShapeKind::Diamond => write!(f, "Diamond"),
            ShapeKind::Arrow => write!(f, "Arrow"),
        }
    }
}

/// All available shape kinds, in palette order.
pub const ALL_SHAPE_KINDS: &[ShapeKind] = &[
    ShapeKind::Rectangle,
    ShapeKind::Circle,
    ShapeKind::Diamond,
    ShapeKind::Arrow,
];

/// A single shape placed on the magic sheet canvas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagicSheetShape {
    /// Unique within this sheet; never reused after deletion.
    pub id: u32,
    pub kind: ShapeKind,
    /// Centre of the shape in canvas space (logical pixels, origin = canvas top-left).
    pub pos: [f32; 2],
    /// Size multiplier — 1.0 means the default base size (~80 × 60 px).
    pub scale: f32,
    /// Rotation in degrees, clockwise. 0 = not rotated.
    #[serde(default)]
    pub rotation_deg: f32,
    /// Background fill colour [R, G, B, A].
    pub bg_color: [u8; 4],
    /// Outline / border colour [R, G, B, A].
    pub outline_color: [u8; 4],
    /// Linked fixture (matches `Patch::id`). `None` = unassigned.
    pub fixture_id: Option<usize>,
    /// Linked group (matches `Group::id`). Mutually exclusive with `fixture_id`.
    /// When set, clicking this shape in live mode selects all fixtures in the group.
    #[serde(default)]
    pub group_id: Option<u32>,
    /// When true, this shape is treated as a group shape (even if no group is assigned yet).
    /// Persists the mode selection in properties without requiring a group to be chosen first.
    #[serde(default)]
    pub is_group: bool,
    /// In live mode, mirror the linked fixture's RGB colour into the fill.
    #[serde(default)]
    pub link_color: bool,
    /// In live mode, modulate fill brightness by the linked fixture's intensity.
    #[serde(default)]
    pub link_intensity: bool,
    /// When set, this is a command shape: clicking it in live mode runs this
    /// command-line command (e.g. "go", "back", "stop", "goto5", "4a33").
    #[serde(default)]
    pub command: Option<String>,
}

impl MagicSheetShape {
    pub fn new(id: u32, kind: ShapeKind, pos: [f32; 2]) -> Self {
        Self {
            id,
            kind,
            pos,
            scale: 1.0,
            rotation_deg: 0.0,
            bg_color: [30, 50, 75, 255],
            outline_color: [100, 150, 200, 255],
            fixture_id: None,
            group_id: None,
            is_group: false,
            link_color: false,
            link_intensity: false,
            command: None,
        }
    }

    /// True when this shape is a command shape (clicking it runs `command`).
    pub fn is_command(&self) -> bool {
        self.command.is_some()
    }

    /// Create a copy of this shape with a fresh ID, offset by `offset` in canvas space.
    /// Preserves every attribute so copy/paste keeps links and commands intact.
    pub fn duplicate(&self, id: u32, offset: [f32; 2]) -> Self {
        Self {
            id,
            kind: self.kind.clone(),
            pos: [self.pos[0] + offset[0], self.pos[1] + offset[1]],
            scale: self.scale,
            rotation_deg: self.rotation_deg,
            bg_color: self.bg_color,
            outline_color: self.outline_color,
            fixture_id: self.fixture_id,
            group_id: self.group_id,
            is_group: self.is_group,
            link_color: self.link_color,
            link_intensity: self.link_intensity,
            command: self.command.clone(),
        }
    }
}

/// Complete magic sheet layout — embedded in `ShowFile`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MagicSheet {
    /// All shapes on the canvas.
    pub shapes: Vec<MagicSheetShape>,
    /// Monotonically increasing; never reused.
    #[serde(default = "default_next_id")]
    pub next_shape_id: u32,
    /// Canvas pan offset [x, y] in logical pixels (persisted with show file).
    #[serde(default)]
    pub canvas_offset: [f32; 2],
    /// Canvas zoom level, 1.0 = 100% (persisted with show file).
    #[serde(default = "default_zoom")]
    pub canvas_zoom: f32,
}

fn default_zoom() -> f32 {
    1.0
}

fn default_next_id() -> u32 {
    1
}

impl MagicSheet {
    /// Add a default shape and return its new ID.
    pub fn add_shape(&mut self, kind: ShapeKind, pos: [f32; 2]) -> u32 {
        let id = self.next_shape_id.max(1);
        self.next_shape_id = id + 1;
        self.shapes.push(MagicSheetShape::new(id, kind, pos));
        id
    }

    /// Add a command shape with a sensible default command ("go").
    pub fn add_command_shape(&mut self, pos: [f32; 2]) -> u32 {
        let id = self.next_shape_id.max(1);
        self.next_shape_id = id + 1;
        let mut shape = MagicSheetShape::new(id, ShapeKind::Rectangle, pos);
        shape.command = Some("go".to_string());
        shape.bg_color = [70, 45, 20, 255];
        shape.outline_color = [220, 170, 50, 255];
        self.shapes.push(shape);
        id
    }

    /// Add a copy of `source` offset by `offset` (used for paste), returning the new ID.
    pub fn add_shape_clone(&mut self, source: &MagicSheetShape, offset: [f32; 2]) -> u32 {
        let id = self.next_shape_id.max(1);
        self.next_shape_id = id + 1;
        self.shapes.push(source.duplicate(id, offset));
        id
    }

    pub fn remove_shape(&mut self, id: u32) {
        self.shapes.retain(|s| s.id != id);
    }

    pub fn get_shape_mut(&mut self, id: u32) -> Option<&mut MagicSheetShape> {
        self.shapes.iter_mut().find(|s| s.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_shape_serde_round_trip_and_default() {
        let mut sheet = MagicSheet::default();
        let id = sheet.add_command_shape([10.0, 20.0]);
        let shape = sheet.shapes.iter().find(|s| s.id == id).unwrap();
        assert_eq!(shape.command.as_deref(), Some("go"));
        assert!(shape.is_command());

        let json = serde_json::to_string(&sheet).unwrap();
        let back: MagicSheet = serde_json::from_str(&json).unwrap();
        let shape = back.shapes.iter().find(|s| s.id == id).unwrap();
        assert_eq!(shape.command.as_deref(), Some("go"));

        // Older show files without the field still deserialize (None = fixture shape).
        let legacy = r#"{"shapes":[{"id":1,"kind":"Rectangle","pos":[0.0,0.0],"scale":1.0,"bg_color":[30,50,75,255],"outline_color":[100,150,200,255],"fixture_id":null}],"next_shape_id":2}"#;
        let old: MagicSheet = serde_json::from_str(legacy).unwrap();
        assert!(!old.shapes[0].is_command());
    }

    #[test]
    fn duplicate_preserves_command_and_group() {
        let mut sheet = MagicSheet::default();
        let id = sheet.add_command_shape([0.0, 0.0]);
        let mut src = sheet.shapes.iter().find(|s| s.id == id).unwrap().clone();
        src.rotation_deg = 45.0;
        let new_id = sheet.add_shape_clone(&src, [5.0, 5.0]);
        let dup = sheet.shapes.iter().find(|s| s.id == new_id).unwrap();
        assert_eq!(dup.command.as_deref(), Some("go"));
        assert_eq!(dup.pos, [5.0, 5.0]);
        assert_eq!(dup.rotation_deg, 45.0);
    }
}
