//! Colours used by the viewer, taken from the design canvas.

use eframe::egui::Color32;
use gcode::Feature;

pub const CANVAS: Color32 = Color32::from_rgb(0x14, 0x17, 0x1C);
pub const MUTED: Color32 = Color32::from_rgb(0x8C, 0x94, 0xA3);
pub const TRAVEL: Color32 = Color32::from_rgb(0x5C, 0x65, 0x75);
/// The layer just below the current one, drawn faintly for context.
pub const BELOW: Color32 = Color32::from_rgb(0x3A, 0x41, 0x4D);
pub const ERROR: Color32 = Color32::from_rgb(0xE5, 0x48, 0x4D);

pub fn feature_color(feature: Feature) -> Color32 {
    match feature {
        Feature::OuterWall => Color32::from_rgb(0xFF, 0x7A, 0x3D),
        Feature::InnerWall => Color32::from_rgb(0xF2, 0xC1, 0x4E),
        Feature::Infill => Color32::from_rgb(0x4F, 0x8C, 0xF0),
        Feature::Support => Color32::from_rgb(0xB4, 0x7C, 0xFF),
        Feature::Skirt => Color32::from_rgb(0x3C, 0xC3, 0xA0),
        Feature::Other | Feature::Unknown => MUTED,
    }
}
