//! G-code parsing for LayerLens.
//!
//! [`parse_reader`] and [`parse_str`] turn G-code into a [`Toolpath`]: the list
//! of moves, the layers, and summary totals. Nothing here depends on a GUI.

pub mod parser;
pub mod toolpath;

pub use parser::{ParseError, parse_reader, parse_str};
pub use toolpath::{Bounds, Feature, Layer, Move, MoveKind, ParseReport, Toolpath};
