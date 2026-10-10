//! Parses the synthetic samples in `testdata/synthetic` and checks the results
//! against `expected.json`, which the generator writes from the moves it emitted.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;

use gcode::parse_reader;

#[derive(serde::Deserialize)]
struct Expected {
    layers: usize,
    z_max: f32,
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
    filament_mm: f64,
    time_s: f64,
}

fn samples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/synthetic")
}

/// Arcs are drawn as chords, so the drawn bounds can sit slightly inside the true ones.
const BOUNDS_TOLERANCE_MM: f32 = 0.05;

#[test]
fn synthetic_samples_match_expected_values() {
    let dir = samples_dir();
    let manifest: BTreeMap<String, Expected> =
        serde_json::from_str(&fs::read_to_string(dir.join("expected.json")).unwrap()).unwrap();

    for (name, expected) in manifest {
        let file = File::open(dir.join(&name)).unwrap();
        let toolpath = parse_reader(BufReader::new(file)).unwrap();

        assert_eq!(
            toolpath.layers.len(),
            expected.layers,
            "{name}: layer count"
        );
        assert!(
            toolpath.report.warnings.is_empty(),
            "{name}: warnings {:?}",
            toolpath.report.warnings
        );

        let bounds = toolpath.bounds.expect("bounds");
        let axes = bounds
            .min
            .iter()
            .zip(&expected.bounds_min)
            .zip(bounds.max.iter().zip(&expected.bounds_max));
        for (axis, ((&min, &want_min), (&max, &want_max))) in axes.enumerate() {
            assert!(
                (min - want_min).abs() <= BOUNDS_TOLERANCE_MM,
                "{name}: min[{axis}] was {min}, expected {want_min}"
            );
            assert!(
                (max - want_max).abs() <= BOUNDS_TOLERANCE_MM,
                "{name}: max[{axis}] was {max}, expected {want_max}"
            );
        }
        assert!(
            (bounds.max[2] - expected.z_max).abs() < 1e-3,
            "{name}: z_max"
        );

        assert!(
            (toolpath.filament_mm - expected.filament_mm).abs() < 0.01,
            "{name}: filament {} vs expected {}",
            toolpath.filament_mm,
            expected.filament_mm
        );
        assert!(
            (toolpath.time_s - expected.time_s).abs() < 1.0,
            "{name}: time {} vs expected {}",
            toolpath.time_s,
            expected.time_s
        );
    }
}
