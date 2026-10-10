//! The window: file loading, the top-down view of one layer, and the summary panel.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use eframe::egui::{self, Align2, FontId, Key, RichText, Sense, Stroke};
use gcode::{Move, MoveKind, Toolpath, parse_reader};

use crate::theme;
use crate::viewport::{Transform, Viewport};

const EXTRUDE_WIDTH: f32 = 1.5;
const TRAVEL_WIDTH: f32 = 1.0;
/// Shown in the summary so long lists of warnings don't fill the panel.
const WARNINGS_SHOWN: usize = 5;

enum Load {
    Empty,
    Loading {
        name: String,
        rx: Receiver<Result<Toolpath, String>>,
    },
    Ready {
        name: String,
        toolpath: Toolpath,
    },
    Failed {
        name: String,
        message: String,
    },
}

pub struct LayerLensApp {
    load: Load,
    /// Index into `Toolpath::layers` of the layer on screen.
    layer: usize,
    show_travel: bool,
    viewport: Viewport,
}

impl LayerLensApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial: Option<PathBuf>) -> Self {
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        let mut app = LayerLensApp {
            load: Load::Empty,
            layer: 0,
            show_travel: false,
            viewport: Viewport::default(),
        };
        if let Some(path) = initial {
            app.start_load(path);
        }
        app
    }

    /// Parses the file on a worker thread so the window stays responsive.
    fn start_load(&mut self, path: PathBuf) {
        let name = file_name(&path);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = File::open(&path)
                .map_err(|e| e.to_string())
                .and_then(|file| parse_reader(BufReader::new(file)).map_err(|e| e.to_string()));
            // If the window has closed in the meantime, nobody is listening.
            let _ = tx.send(result);
        });
        self.load = Load::Loading { name, rx };
        self.layer = 0;
        self.viewport = Viewport::default();
    }

    fn poll_load(&mut self) {
        let Load::Loading { name, rx } = &self.load else {
            return;
        };
        let outcome = match rx.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("loading stopped unexpectedly".to_string()),
        };
        let name = name.clone();
        self.load = match outcome {
            Ok(toolpath) => Load::Ready { name, toolpath },
            Err(message) => Load::Failed { name, message },
        };
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()));
        if let Some(path) = dropped {
            self.start_load(path);
        }

        let steps = ctx.input(|i| {
            let mut steps: isize = 0;
            if i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::ArrowRight) {
                steps += 1;
            }
            if i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::ArrowLeft) {
                steps -= 1;
            }
            if i.key_pressed(Key::PageUp) {
                steps += 10;
            }
            if i.key_pressed(Key::PageDown) {
                steps -= 10;
            }
            steps
        });
        if steps != 0 {
            let last = match &self.load {
                Load::Ready { toolpath, .. } => toolpath.layers.len().saturating_sub(1),
                _ => 0,
            };
            self.layer = (self.layer as isize + steps).clamp(0, last as isize) as usize;
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Open…").clicked() {
                let picked = rfd::FileDialog::new()
                    .add_filter("G-code", &["gcode", "gco", "g"])
                    .pick_file();
                if let Some(path) = picked {
                    self.start_load(path);
                }
            }
            if ui.button("Reset view").clicked() {
                self.viewport = Viewport::default();
            }
            ui.checkbox(&mut self.show_travel, "Show travel");
            ui.separator();
            ui.label(self.title());
        });
    }

    fn title(&self) -> String {
        match &self.load {
            Load::Empty => "Drop a .gcode file here, or click Open…".to_string(),
            Load::Loading { name, .. } => format!("Reading {name}…"),
            Load::Ready { name, .. } => name.clone(),
            Load::Failed { name, .. } => format!("{name} could not be read"),
        }
    }

    fn summary(&self, ui: &mut egui::Ui) {
        match &self.load {
            Load::Empty => {
                ui.label("No file open.");
            }
            Load::Loading { .. } => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Reading the file…");
                });
            }
            Load::Failed { message, .. } => {
                ui.colored_label(theme::ERROR, message);
            }
            Load::Ready { toolpath, .. } => self.ready_summary(ui, toolpath),
        }
    }

    fn ready_summary(&self, ui: &mut egui::Ui, toolpath: &Toolpath) {
        ui.heading("Print");
        egui::Grid::new("print_stats")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                stat(ui, "Layers", toolpath.layers.len().to_string());
                if let Some(top) = toolpath.layers.last() {
                    stat(ui, "Height", format!("{:.2} mm", top.z));
                }
                if let Some(b) = toolpath.bounds {
                    stat(
                        ui,
                        "Footprint",
                        format!("{:.1} × {:.1} mm", b.max[0] - b.min[0], b.max[1] - b.min[1]),
                    );
                }
                stat(
                    ui,
                    "Filament",
                    format!("{:.2} m", toolpath.filament_mm / 1000.0),
                );
                stat(
                    ui,
                    "Time",
                    format!("{} (estimated)", format_duration(toolpath.time_s)),
                );
                stat(ui, "Lines read", toolpath.report.lines.to_string());
            });

        let warnings = &toolpath.report.warnings;
        if !warnings.is_empty() {
            ui.add_space(8.0);
            ui.colored_label(theme::MUTED, format!("{} lines skipped:", warnings.len()));
            for warning in warnings.iter().take(WARNINGS_SHOWN) {
                ui.colored_label(theme::MUTED, RichText::new(warning).small());
            }
        }

        ui.add_space(16.0);
        ui.heading("This layer");
        match toolpath.layers.get(self.layer) {
            Some(layer) => {
                let moves = toolpath.layer_moves(self.layer);
                let extruding = moves.iter().filter(|m| m.kind == MoveKind::Extrude).count();
                let filament: f32 = moves.iter().map(|m| m.extrusion_mm).sum();
                let time: f32 = moves.iter().map(|m| m.time_s).sum();
                egui::Grid::new("layer_stats")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        stat(ui, "Height", format!("{:.2} mm", layer.z));
                        stat(ui, "Extruding moves", extruding.to_string());
                        stat(ui, "Filament", format!("{filament:.1} mm"));
                        stat(ui, "Time", format_duration(f64::from(time)));
                    });
            }
            None => {
                ui.label("No layers in this file.");
            }
        }
    }

    fn layer_controls(&mut self, ui: &mut egui::Ui) {
        let Load::Ready { toolpath, .. } = &self.load else {
            return;
        };
        let count = toolpath.layers.len();
        if count == 0 {
            ui.label("No layers to show.");
            return;
        }
        ui.horizontal(|ui| {
            ui.label("Layer");
            ui.add(egui::Slider::new(&mut self.layer, 0..=count - 1).show_value(false));
            ui.label(format!("{} of {}", self.layer + 1, count));
        });
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, theme::CANVAS);

        let Load::Ready { toolpath, .. } = &self.load else {
            let hint = match &self.load {
                Load::Loading { .. } => "Reading the file…",
                Load::Failed { .. } => "This file could not be read.",
                _ => "Drop a .gcode file here, or click Open…",
            };
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                hint,
                FontId::proportional(18.0),
                theme::MUTED,
            );
            return;
        };
        let Some(bounds) = toolpath.bounds else {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "This file never moves the nozzle.",
                FontId::proportional(18.0),
                theme::MUTED,
            );
            return;
        };
        if toolpath.layers.is_empty() {
            return;
        }

        if response.dragged() {
            self.viewport.pan += response.drag_delta();
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.viewport.zoom_by((scroll * 0.002).exp());
            }
        }

        let transform = self.viewport.transform(
            [bounds.min[0], bounds.min[1]],
            [bounds.max[0], bounds.max[1]],
            rect,
        );
        // The layer below is drawn faintly so you can see how this one sits on it.
        if self.layer > 0 {
            draw_moves(
                &painter,
                toolpath.layer_moves(self.layer - 1),
                &transform,
                false,
                true,
            );
        }
        draw_moves(
            &painter,
            toolpath.layer_moves(self.layer),
            &transform,
            self.show_travel,
            false,
        );
    }
}

impl eframe::App for LayerLensApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_load();
        self.handle_input(&ctx);
        if matches!(self.load, Load::Loading { .. }) {
            ctx.request_repaint();
        }

        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        egui::Panel::right("summary")
            .resizable(false)
            .default_size(280.0)
            .show(ui, |ui| self.summary(ui));
        egui::Panel::bottom("layer_controls").show(ui, |ui| self.layer_controls(ui));
        egui::CentralPanel::default().show(ui, |ui| self.canvas(ui));
    }
}

fn draw_moves(
    painter: &egui::Painter,
    moves: &[Move],
    transform: &Transform,
    show_travel: bool,
    faded: bool,
) {
    for m in moves {
        let (color, width) = match (m.kind, faded) {
            (_, true) if m.kind == MoveKind::Travel => continue,
            (_, true) => (theme::BELOW, 1.0),
            (MoveKind::Extrude, false) => (theme::feature_color(m.feature), EXTRUDE_WIDTH),
            (MoveKind::Travel, false) if show_travel => (theme::TRAVEL, TRAVEL_WIDTH),
            (MoveKind::Travel, false) => continue,
        };
        let from = transform.to_screen([m.from[0], m.from[1]]);
        let to = transform.to_screen([m.to[0], m.to[1]]);
        painter.line_segment([from, to], Stroke::new(width, color));
    }
}

fn stat(ui: &mut egui::Ui, label: &str, value: String) {
    ui.label(RichText::new(label).color(theme::MUTED));
    ui.label(value);
    ui.end_row();
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn format_duration(seconds: f64) -> String {
    let total = seconds.round() as u64;
    format!(
        "{}h {:02}m {:02}s",
        total / 3600,
        total % 3600 / 60,
        total % 60
    )
}
