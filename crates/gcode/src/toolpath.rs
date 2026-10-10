//! The parsed print: moves, layers and summary totals.

use std::ops::Range;

/// What the nozzle is doing during a move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveKind {
    /// Moves while depositing filament.
    Extrude,
    /// Moves without depositing filament.
    Travel,
}

/// The part of the print a move belongs to, from slicer `;TYPE:` comments.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    OuterWall,
    InnerWall,
    Infill,
    Support,
    Skirt,
    /// A label we don't recognise yet.
    Other,
    /// No `;TYPE:` comment seen before this move.
    #[default]
    Unknown,
}

impl Feature {
    /// Maps a slicer's feature label to a feature. Matching is case-insensitive
    /// and covers PrusaSlicer, Bambu Studio, OrcaSlicer and Cura wording.
    pub fn from_label(label: &str) -> Feature {
        let label = label.trim().to_ascii_lowercase();
        if label.contains("outer") || label.contains("external") {
            Feature::OuterWall
        } else if label.contains("infill") || label == "fill" {
            Feature::Infill
        } else if label.contains("inner") || label.contains("perimeter") || label.contains("wall") {
            Feature::InnerWall
        } else if label.contains("support") {
            Feature::Support
        } else if label.contains("skirt") || label.contains("brim") {
            Feature::Skirt
        } else {
            Feature::Other
        }
    }
}

/// One straight segment of the toolpath, in millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Move {
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub kind: MoveKind,
    pub feature: Feature,
    /// Filament length deposited by this move (0 for travel).
    pub extrusion_mm: f32,
    /// Time this move takes at the feedrate in effect, in seconds.
    pub time_s: f32,
    /// 1-based line number in the source file.
    pub line: u32,
}

/// A group of moves printed at one height.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub z: f32,
    /// Index range into [`Toolpath::moves`].
    pub moves: Range<usize>,
}

/// Axis-aligned box around every point the nozzle visits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Bounds {
    pub fn at(point: [f32; 3]) -> Bounds {
        Bounds {
            min: point,
            max: point,
        }
    }

    pub fn include(&mut self, point: [f32; 3]) {
        for (axis, &value) in point.iter().enumerate() {
            self.min[axis] = self.min[axis].min(value);
            self.max[axis] = self.max[axis].max(value);
        }
    }
}

/// What the parser noticed while reading the file.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ParseReport {
    /// Number of lines read.
    pub lines: u32,
    /// Commands the parser doesn't act on (e.g. `M104`), with how often each appeared.
    pub ignored_commands: std::collections::BTreeMap<String, u32>,
    /// Problems that were skipped, each prefixed with its line number.
    pub warnings: Vec<String>,
}

/// A parsed G-code file.
#[derive(Debug, Clone, PartialEq)]
pub struct Toolpath {
    pub moves: Vec<Move>,
    pub layers: Vec<Layer>,
    /// `None` if the file never moved the nozzle.
    pub bounds: Option<Bounds>,
    /// Net filament length in mm. Retractions cancel out, so this matches the slicer's figure.
    pub filament_mm: f64,
    /// Estimated print time in seconds, from the feedrates in the file.
    pub time_s: f64,
    pub report: ParseReport,
}

impl Toolpath {
    /// The moves belonging to one layer.
    pub fn layer_moves(&self, index: usize) -> &[Move] {
        &self.moves[self.layers[index].moves.clone()]
    }
}
