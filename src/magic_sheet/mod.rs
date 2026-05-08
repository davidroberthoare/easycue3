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
}

impl std::fmt::Display for ShapeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShapeKind::Rectangle => write!(f, "Rect"),
            ShapeKind::Circle    => write!(f, "Circle"),
            ShapeKind::Diamond   => write!(f, "Diamond"),
        }
    }
}

/// All available shape kinds, in palette order.
pub const ALL_SHAPE_KINDS: &[ShapeKind] = &[
    ShapeKind::Rectangle,
    ShapeKind::Circle,
    ShapeKind::Diamond,
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
    /// Background fill colour [R, G, B, A].
    pub bg_color: [u8; 4],
    /// Outline / border colour [R, G, B, A].
    pub outline_color: [u8; 4],
    /// Linked fixture (matches `Patch::id`). `None` = unassigned.
    pub fixture_id: Option<usize>,
    /// In live mode, mirror the linked fixture's RGB colour into the fill.
    #[serde(default)]
    pub link_color: bool,
    /// In live mode, modulate fill brightness by the linked fixture's intensity.
    #[serde(default)]
    pub link_intensity: bool,
}

impl MagicSheetShape {
    pub fn new(id: u32, kind: ShapeKind, pos: [f32; 2]) -> Self {
        Self {
            id,
            kind,
            pos,
            scale: 1.0,
            bg_color: [30, 50, 75, 255],
            outline_color: [100, 150, 200, 255],
            fixture_id: None,
            link_color: false,
            link_intensity: false,
        }
    }

    pub fn new_full(
        id: u32,
        kind: ShapeKind,
        pos: [f32; 2],
        scale: f32,
        bg_color: [u8; 4],
        outline_color: [u8; 4],
        fixture_id: Option<usize>,
    ) -> Self {
        Self { id, kind, pos, scale, bg_color, outline_color, fixture_id, link_color: false, link_intensity: false }
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

fn default_zoom() -> f32 { 1.0 }

fn default_next_id() -> u32 { 1 }

impl MagicSheet {
    /// Add a default shape and return its new ID.
    pub fn add_shape(&mut self, kind: ShapeKind, pos: [f32; 2]) -> u32 {
        let id = self.next_shape_id.max(1);
        self.next_shape_id = id + 1;
        self.shapes.push(MagicSheetShape::new(id, kind, pos));
        id
    }

    /// Add a shape with full attribute set (used for paste with offset).
    pub fn add_shape_at(
        &mut self,
        kind: ShapeKind,
        pos: [f32; 2],
        scale: f32,
        bg_color: [u8; 4],
        outline_color: [u8; 4],
        fixture_id: Option<usize>,
    ) -> u32 {
        let id = self.next_shape_id.max(1);
        self.next_shape_id = id + 1;
        self.shapes.push(MagicSheetShape::new_full(id, kind, pos, scale, bg_color, outline_color, fixture_id));
        id
    }

    pub fn remove_shape(&mut self, id: u32) {
        self.shapes.retain(|s| s.id != id);
    }

    pub fn get_shape_mut(&mut self, id: u32) -> Option<&mut MagicSheetShape> {
        self.shapes.iter_mut().find(|s| s.id == id)
    }
}
