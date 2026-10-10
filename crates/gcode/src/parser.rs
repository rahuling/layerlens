//! Turns G-code text into a [`Toolpath`].
//!
//! The parser tracks a little machine state (position, extrusion mode,
//! feedrate, current feature) and turns each motion into a [`Move`]. It is
//! tolerant on purpose: unknown commands are counted rather than rejected, and
//! malformed lines are reported and skipped.

use std::f64::consts::TAU;
use std::io::BufRead;

use crate::toolpath::{Bounds, Feature, Layer, Move, MoveKind, ParseReport, Toolpath};

/// Longest chord (mm) used to approximate an arc.
const ARC_SEGMENT_MM: f64 = 0.5;
/// Feedrate (mm/min) used until the file sets one with `F`.
const DEFAULT_FEED_MM_MIN: f64 = 1800.0;
/// Motions shorter than this (mm) count as no motion.
const EPSILON: f64 = 1e-9;
/// Z difference (mm) that starts a new layer.
const LAYER_EPSILON: f32 = 1e-4;
const AXES: [char; 3] = ['X', 'Y', 'Z'];

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("could not read G-code: {0}")]
    Io(#[from] std::io::Error),
}

/// Parses G-code from a string.
pub fn parse_str(text: &str) -> Result<Toolpath, ParseError> {
    parse_reader(text.as_bytes())
}

/// Parses G-code from any buffered reader, one line at a time.
pub fn parse_reader<R: BufRead>(reader: R) -> Result<Toolpath, ParseError> {
    let mut parser = Parser::new();
    for (index, line) in reader.lines().enumerate() {
        parser.line(index as u32 + 1, &line?);
    }
    Ok(parser.finish())
}

/// Words on one line: the command (e.g. `G1`) and its parameters (`X`, `E`, ...).
#[derive(Debug, Default)]
struct Line {
    command: Option<(char, u32)>,
    params: [Option<f64>; 26],
}

impl Line {
    fn get(&self, letter: char) -> Option<f64> {
        self.params[usize::from(letter as u8 - b'A')]
    }
}

fn parse_line(code: &str) -> Result<Line, String> {
    let mut line = Line::default();
    for token in code.split_whitespace() {
        let letter = token.as_bytes()[0].to_ascii_uppercase() as char;
        if !letter.is_ascii_uppercase() {
            return Err(format!("unexpected '{token}'"));
        }
        if letter == 'N' {
            continue; // line number prefix
        }
        let value: f64 = token[1..]
            .parse()
            .map_err(|_| format!("bad number in '{token}'"))?;
        if matches!(letter, 'G' | 'M' | 'T') && line.command.is_none() {
            if value < 0.0 || value.fract() != 0.0 {
                return Err(format!("bad command '{token}'"));
            }
            let code = value as u32;
            line.command = Some((letter, code));
            if letter == 'M' && matches!(code, 117 | 118) {
                break; // free-text messages: ignore the rest of the line
            }
        } else {
            line.params[usize::from(letter as u8 - b'A')] = Some(value);
        }
    }
    Ok(line)
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

fn to_f32(point: [f64; 3]) -> [f32; 3] {
    point.map(|v| v as f32)
}

#[derive(Debug)]
struct Parser {
    pos: [f64; 3],
    e_pos: f64,
    absolute_xyz: bool,
    absolute_e: bool,
    feed_mm_min: f64,
    feature: Feature,
    moves: Vec<Move>,
    layers: Vec<Layer>,
    bounds: Option<Bounds>,
    filament_mm: f64,
    time_s: f64,
    report: ParseReport,
}

impl Parser {
    fn new() -> Parser {
        Parser {
            pos: [0.0; 3],
            e_pos: 0.0,
            absolute_xyz: true,
            absolute_e: true,
            feed_mm_min: DEFAULT_FEED_MM_MIN,
            feature: Feature::Unknown,
            moves: Vec::new(),
            layers: Vec::new(),
            bounds: None,
            filament_mm: 0.0,
            time_s: 0.0,
            report: ParseReport::default(),
        }
    }

    fn line(&mut self, number: u32, raw: &str) {
        self.report.lines = number;
        let (code, comment) = match raw.split_once(';') {
            Some((code, comment)) => (code, Some(comment)),
            None => (raw, None),
        };
        if let Some(label) = comment.and_then(|c| c.trim().strip_prefix("TYPE:")) {
            self.feature = Feature::from_label(label);
        }
        match parse_line(code) {
            Ok(line) => self.execute(number, &line),
            Err(message) => self
                .report
                .warnings
                .push(format!("line {number}: {message}")),
        }
    }

    fn execute(&mut self, number: u32, line: &Line) {
        let Some((letter, code)) = line.command else {
            return;
        };
        match (letter, code) {
            ('G', 0 | 1) => self.linear(number, line),
            ('G', 2) => self.arc(number, line, true),
            ('G', 3) => self.arc(number, line, false),
            ('G', 21) => {}
            ('G', 28) => self.home(line),
            ('G', 90) => self.absolute_xyz = true,
            ('G', 91) => self.absolute_xyz = false,
            ('G', 92) => self.set_position(line),
            ('M', 82) => self.absolute_e = true,
            ('M', 83) => self.absolute_e = false,
            _ => {
                *self
                    .report
                    .ignored_commands
                    .entry(format!("{letter}{code}"))
                    .or_insert(0) += 1
            }
        }
    }

    fn set_feed(&mut self, line: &Line) {
        if let Some(feed) = line.get('F').filter(|&f| f > 0.0) {
            self.feed_mm_min = feed;
        }
    }

    /// The end point of a move, and whether the line named any axis.
    fn target(&self, line: &Line) -> ([f64; 3], bool) {
        let mut target = self.pos;
        let mut any = false;
        for (i, &axis) in AXES.iter().enumerate() {
            if let Some(v) = line.get(axis) {
                any = true;
                target[i] = if self.absolute_xyz {
                    v
                } else {
                    self.pos[i] + v
                };
            }
        }
        (target, any)
    }

    /// Filament change for this line, in mm. Keeps the extruder position up to date.
    fn extrusion(&mut self, line: &Line) -> f64 {
        match line.get('E') {
            None => 0.0,
            Some(v) if self.absolute_e => {
                let delta = v - self.e_pos;
                self.e_pos = v;
                delta
            }
            Some(v) => {
                self.e_pos += v;
                v
            }
        }
    }

    fn linear(&mut self, number: u32, line: &Line) {
        self.set_feed(line);
        let (target, has_axes) = self.target(line);
        let de = self.extrusion(line);
        let path = distance(self.pos, target);
        self.motion(number, target, de, path, has_axes);
    }

    fn arc(&mut self, number: u32, line: &Line, clockwise: bool) {
        self.set_feed(line);
        let (target, has_axes) = self.target(line);
        let de = self.extrusion(line);
        let start = self.pos;

        if line.get('I').is_none() && line.get('J').is_none() {
            self.report.warnings.push(format!(
                "line {number}: arc without I/J offsets drawn as a straight line"
            ));
            let path = distance(start, target);
            self.motion(number, target, de, path, has_axes);
            return;
        }

        // I and J are offsets from the start point to the centre.
        let (i, j) = (line.get('I').unwrap_or(0.0), line.get('J').unwrap_or(0.0));
        let radius = i.hypot(j);
        let (cx, cy) = (start[0] + i, start[1] + j);
        let a_start = (start[1] - cy).atan2(start[0] - cx);
        let a_end = (target[1] - cy).atan2(target[0] - cx);
        let chord = (target[0] - start[0]).hypot(target[1] - start[1]);
        // Equal start and end points mean a full circle.
        let sweep = match (chord < 1e-6, clockwise) {
            (true, true) => -TAU,
            (true, false) => TAU,
            (false, true) => -(a_start - a_end).rem_euclid(TAU),
            (false, false) => (a_end - a_start).rem_euclid(TAU),
        };

        let segments = ((sweep.abs() * radius / ARC_SEGMENT_MM).ceil() as usize).max(1);
        let n = segments as f64;
        let seg_path = (radius * sweep.abs() / n).hypot((target[2] - start[2]) / n);
        for k in 1..=segments {
            let point = if k == segments {
                target
            } else {
                let frac = k as f64 / n;
                let angle = a_start + sweep * frac;
                [
                    cx + radius * angle.cos(),
                    cy + radius * angle.sin(),
                    start[2] + (target[2] - start[2]) * frac,
                ]
            };
            self.motion(number, point, de / n, seg_path, has_axes);
        }
    }

    fn home(&mut self, line: &Line) {
        let all = AXES.iter().all(|&a| line.get(a).is_none());
        for (i, &axis) in AXES.iter().enumerate() {
            if all || line.get(axis).is_some() {
                self.pos[i] = 0.0;
            }
        }
    }

    fn set_position(&mut self, line: &Line) {
        let none = AXES.iter().all(|&a| line.get(a).is_none()) && line.get('E').is_none();
        for (i, &axis) in AXES.iter().enumerate() {
            if let Some(v) = line.get(axis) {
                self.pos[i] = v;
            } else if none {
                self.pos[i] = 0.0;
            }
        }
        if let Some(e) = line.get('E') {
            self.e_pos = e;
        } else if none {
            self.e_pos = 0.0;
        }
    }

    /// Applies one straight motion: updates position, totals, and the move list.
    fn motion(&mut self, number: u32, target: [f64; 3], de: f64, path: f64, track: bool) {
        let start = self.pos;
        let xy = distance([start[0], start[1], 0.0], [target[0], target[1], 0.0]);
        self.pos = target;
        self.filament_mm += de;
        if track {
            self.include(target);
        }

        let time = if path > EPSILON {
            path / (self.feed_mm_min / 60.0)
        } else {
            0.0
        };
        self.time_s += time;

        if xy <= EPSILON {
            return; // Z-only or retraction-only: nothing to draw.
        }
        let kind = if de > EPSILON {
            MoveKind::Extrude
        } else {
            MoveKind::Travel
        };
        if kind == MoveKind::Extrude {
            self.open_layer(target[2]);
        }
        self.moves.push(Move {
            from: to_f32(start),
            to: to_f32(target),
            kind,
            feature: self.feature,
            extrusion_mm: if kind == MoveKind::Extrude {
                de as f32
            } else {
                0.0
            },
            time_s: time as f32,
            line: number,
        });
        if let Some(layer) = self.layers.last_mut() {
            layer.moves.end = self.moves.len();
        }
    }

    /// Starts a new layer when extrusion happens at a new height.
    fn open_layer(&mut self, z: f64) {
        let z = z as f32;
        let is_new = self
            .layers
            .last()
            .is_none_or(|layer| (layer.z - z).abs() > LAYER_EPSILON);
        if is_new {
            let at = self.moves.len();
            self.layers.push(Layer { z, moves: at..at });
        }
    }

    fn include(&mut self, point: [f64; 3]) {
        let point = to_f32(point);
        if let Some(bounds) = &mut self.bounds {
            bounds.include(point);
        } else {
            self.bounds = Some(Bounds::at(point));
        }
    }

    fn finish(self) -> Toolpath {
        Toolpath {
            moves: self.moves,
            layers: self.layers,
            bounds: self.bounds,
            filament_mm: self.filament_mm,
            time_s: self.time_s,
            report: self.report,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Toolpath {
        parse_str(text).unwrap()
    }

    #[test]
    fn classifies_travel_and_extrusion() {
        let tp = parse("G28\nG1 Z0.2 F720\nG0 X10 Y0 F9000\nG1 X20 Y0 E1.5 F1800\n");
        assert_eq!(tp.moves.len(), 2);
        assert_eq!(tp.moves[0].kind, MoveKind::Travel);
        assert_eq!(tp.moves[1].kind, MoveKind::Extrude);
        assert!((tp.moves[1].extrusion_mm - 1.5).abs() < 1e-6);
        assert!((tp.filament_mm - 1.5).abs() < 1e-9);
    }

    #[test]
    fn relative_positioning_and_extrusion() {
        let tp = parse("G91\nM83\nG1 X1 Y0 E0.5\nG1 X1 Y0 E0.5\nM82\n");
        assert_eq!(tp.moves.len(), 2);
        assert_eq!(tp.moves[1].to[0], 2.0);
        assert!((tp.filament_mm - 1.0).abs() < 1e-9);
    }

    #[test]
    fn g92_resets_extruder_position() {
        let tp = parse("G1 X1 E2.0\nG92 E0\nG1 X2 E0.5\n");
        assert!((tp.filament_mm - 2.5).abs() < 1e-9);
        assert!((tp.moves[1].extrusion_mm - 0.5).abs() < 1e-6);
    }

    #[test]
    fn arc_is_split_into_segments_ending_on_target() {
        // Clockwise semicircle from (0,0) to (2,0) with centre (1,0), passing through (1,1).
        let tp = parse("G1 X0 Y0 F1800\nG2 X2 Y0 I1 J0 E0.3\n");
        assert!(tp.moves.len() > 2);
        let last = tp.moves.last().unwrap();
        assert_eq!(last.to, [2.0, 0.0, 0.0]);
        let max_y = tp.moves.iter().map(|m| m.to[1]).fold(f32::MIN, f32::max);
        assert!((max_y - 1.0).abs() < 0.05, "max_y was {max_y}");
        assert!((tp.filament_mm - 0.3).abs() < 1e-6);
    }

    #[test]
    fn unknown_commands_are_counted_not_rejected() {
        let tp = parse("M104 S215\nM104 S200\nT0\nM117 Layer 1 of 2\n");
        assert_eq!(tp.report.ignored_commands["M104"], 2);
        assert_eq!(tp.report.ignored_commands["T0"], 1);
        assert_eq!(tp.report.ignored_commands["M117"], 1);
        assert!(tp.report.warnings.is_empty());
    }

    #[test]
    fn malformed_lines_are_reported_and_skipped() {
        let tp = parse("G1 Xabc Y1\nG1 X5 Y0 E1\n");
        assert_eq!(tp.report.warnings.len(), 1);
        assert!(tp.report.warnings[0].starts_with("line 1:"));
        assert_eq!(tp.moves.len(), 1);
    }

    #[test]
    fn layers_start_at_new_extrusion_height() {
        let text = "G1 Z0.2\n;TYPE:Outer wall\nG1 X10 E1\nG1 Z1.6\nG0 X0 Y5\nG1 Z0.4\n;TYPE:Infill\nG1 X5 Y5 E2\n";
        let tp = parse(text);
        assert_eq!(tp.layers.len(), 2);
        assert!((tp.layers[0].z - 0.2).abs() < 1e-6);
        assert!((tp.layers[1].z - 0.4).abs() < 1e-6);
        // The z-hop travel stays in the first layer's range.
        assert_eq!(tp.layers[0].moves, 0..2);
        assert_eq!(tp.layers[1].moves, 2..3);
        assert_eq!(tp.moves[0].feature, Feature::OuterWall);
        assert_eq!(tp.moves[2].feature, Feature::Infill);
    }

    #[test]
    fn feature_labels_map_across_slicers() {
        assert_eq!(
            Feature::from_label("External perimeter"),
            Feature::OuterWall
        );
        assert_eq!(Feature::from_label("Perimeter"), Feature::InnerWall);
        assert_eq!(Feature::from_label("WALL-OUTER"), Feature::OuterWall);
        assert_eq!(Feature::from_label("Inner wall"), Feature::InnerWall);
        assert_eq!(Feature::from_label("Internal infill"), Feature::Infill);
        assert_eq!(Feature::from_label("FILL"), Feature::Infill);
        assert_eq!(Feature::from_label("Support material"), Feature::Support);
        assert_eq!(Feature::from_label("Custom ironing"), Feature::Other);
    }
}
