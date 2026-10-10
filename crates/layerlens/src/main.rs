//! Command-line entry point. For now it prints a summary of a G-code file;
//! the window comes in milestone 2.

use std::fs::File;
use std::io::BufReader;
use std::process::ExitCode;

use gcode::{Toolpath, parse_reader};

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: layerlens <file.gcode>");
        return ExitCode::FAILURE;
    };
    match run(&path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let toolpath = parse_reader(BufReader::new(File::open(path)?))?;
    print_summary(&toolpath);
    Ok(())
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

fn print_summary(toolpath: &Toolpath) {
    let extrude_moves = toolpath
        .moves
        .iter()
        .filter(|m| m.kind == gcode::MoveKind::Extrude)
        .count();
    println!("layers:    {}", toolpath.layers.len());
    if let (Some(first), Some(last)) = (toolpath.layers.first(), toolpath.layers.last()) {
        println!(
            "height:    {:.2} mm (first layer {:.2} mm)",
            last.z, first.z
        );
    }
    if let Some(bounds) = toolpath.bounds {
        println!(
            "bounds:    x {:.1}..{:.1}  y {:.1}..{:.1}  z {:.2}..{:.2} mm",
            bounds.min[0],
            bounds.max[0],
            bounds.min[1],
            bounds.max[1],
            bounds.min[2],
            bounds.max[2]
        );
    }
    println!("filament:  {:.2} mm", toolpath.filament_mm);
    println!(
        "time:      {} (estimated from feedrates)",
        format_duration(toolpath.time_s)
    );
    println!(
        "moves:     {} total, {} extruding",
        toolpath.moves.len(),
        extrude_moves
    );

    let report = &toolpath.report;
    println!("lines:     {}", report.lines);
    if !report.ignored_commands.is_empty() {
        let list: Vec<String> = report
            .ignored_commands
            .iter()
            .map(|(cmd, n)| format!("{cmd} x{n}"))
            .collect();
        println!("ignored:   {}", list.join(", "));
    }
    if !report.warnings.is_empty() {
        println!("warnings:  {}", report.warnings.len());
        for warning in report.warnings.iter().take(5) {
            println!("  {warning}");
        }
    }
}
