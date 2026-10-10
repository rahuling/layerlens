//! LayerLens: a G-code viewer for Windows.
//!
//! Start it with a file path to open that file straight away, or drop a file
//! onto the window.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod theme;
mod viewport;

use std::path::PathBuf;

use eframe::egui;

fn main() -> eframe::Result {
    let initial = std::env::args().nth(1).map(PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("LayerLens")
            // Fits a 1536x864 laptop screen at 125% scaling, with room for the taskbar.
            .with_inner_size([1000.0, 640.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "LayerLens",
        options,
        Box::new(move |cc| Ok(Box::new(app::LayerLensApp::new(cc, initial)))),
    )
}
