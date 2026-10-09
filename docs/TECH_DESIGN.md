# LayerLens — Technical Design

**Status:** Draft v0.1
**Related:** [PRD.md](PRD.md), [ROADMAP.md](ROADMAP.md)

---

## 1. Technology choices

| Concern | Choice | Why |
|---|---|---|
| Language | Rust (stable) | Fast, memory-safe, a good way to learn the language on a real project |
| GUI framework | `eframe` / `egui` | Pure Rust, immediate-mode UI, easy to iterate on; good enough for panels and sliders |
| 3D rendering | `wgpu` via egui's custom paint callback | Cross-platform GPU API; handles large line buffers well |
| File dialogs | `rfd` | Native Windows open/save dialogs |
| Parsing | Hand-written line parser | G-code is simple line-based text; a parser combinator library would add complexity without benefit |
| Errors | `thiserror` for library errors, `anyhow` at the app boundary | Standard Rust pattern |
| Testing | `cargo test`, plus snapshot tests with `insta` | Catch regressions in parsed output |
| Benchmarks | `criterion` | Track load-time targets |
| Packaging | `cargo build --release`; GitHub Actions builds the `.exe` on tag | Single file, no installer |

Alternatives considered:

- **Tauri (web UI in Rust):** good for UI, but 3D toolpath rendering would still need WebGL, and the web layer adds a second language to learn.
- **Bevy:** powerful, but a game engine is heavy for a viewer.
- **Qt / C++:** mature, but the goal includes learning Rust.

## 2. Architecture

```
┌────────────────────┐      ┌──────────────────────┐      ┌──────────────────┐
│ File loader        │ ───▶ │ Parser (streaming)   │ ───▶ │ Toolpath model   │
│ (mmap / buffered)  │      │ G-code → Moves       │      │ layers, segments │
└────────────────────┘      └──────────────────────┘      └────────┬─────────┘
                                                                  │
                    ┌─────────────────────────────────────────────┼─────────────┐
                    ▼                                             ▼             ▼
          ┌──────────────────┐                         ┌──────────────────┐ ┌──────────────┐
          │ Stats engine     │                         │ Renderer (wgpu)  │ │ UI (egui)    │
          │ time, mass, len  │                         │ lines per layer  │ │ panels, slider│
          └──────────────────┘                         └──────────────────┘ └──────────────┘
```

### Crate layout

The project is a single Cargo workspace with two crates, so the parser can be tested without a GUI.

```
layerlens/
├── Cargo.toml                 # workspace
├── crates/
│   ├── gcode/                 # library: parsing, toolpath model, stats (no GUI deps)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── parser.rs      # line → Command
│   │   │   ├── state.rs       # modal state: absolute/relative, G90/G91, M82/M83, feedrate, position
│   │   │   ├── toolpath.rs    # Move, Layer, FeatureType, Toolpath
│   │   │   ├── slicer/        # per-slicer comment dialects (Prusa, Bambu, Orca, Cura, ...)
│   │   │   ├── stats.rs       # time, filament length, mass, travel
│   │   │   └── diff.rs        # compare two toolpaths (v1.x)
│   │   └── tests/
│   └── layerlens/             # binary: egui app
│       ├── src/
│       │   ├── main.rs
│       │   ├── app.rs         # top-level state, panels
│       │   ├── view3d.rs      # wgpu renderer, camera
│       │   ├── playback.rs    # animation clock
│       │   └── ui/            # summary panel, layer slider, settings
├── testdata/                  # small licence-clean G-code samples per slicer
├── docs/
└── .github/workflows/         # CI: test, clippy, fmt, release build
```

## 3. Data model

```rust
pub struct Move {
    pub from: [f32; 3],        // start position (mm)
    pub to: [f32; 3],          // end position (mm)
    pub kind: MoveKind,        // Extrude or Travel
    pub feature: Feature,      // from slicer comments, else Unknown
    pub feedrate: f32,         // mm/min
    pub extrusion_mm: f32,     // filament length for this move (0 for travel)
    pub line: u32,             // source line number, for jumping in the text view
}

pub struct Layer {
    pub z: f32,
    pub moves: std::ops::Range<usize>, // index range into Toolpath::moves
    pub time_s: f32,
    pub filament_mm: f32,
}

pub struct Toolpath {
    pub moves: Vec<Move>,
    pub layers: Vec<Layer>,
    pub bounds: Aabb,
    pub report: ParseReport,   // unknown commands, warnings, dialect detected
}
```

Design notes:

- A flat `Vec<Move>` with per-layer index ranges keeps memory compact and lets the renderer upload one buffer.
- `f32` is enough for millimetre positions and keeps the GPU buffer half the size of `f64`.
- `line` keeps a link back to the source, so a click on the 3D view can jump to the G-code text.

## 4. Parsing

1. **Read** the file in chunks with `BufReader` (or memory-map with `memmap2` for very large files). Do not read the whole file into a `String`.
2. **Tokenize** each line: strip comments (`;` to end of line), capture slicer comments separately, parse the command letter and numeric parameters.
3. **Apply modal state:** G90/G91 positioning, M82/M83 extrusion mode, G92 position resets, and the last feedrate (`F`) carried forward.
4. **Classify moves:**
   - `G0`, or `G1` with no `E` change → travel.
   - `G1` with positive `E` delta → extrude.
   - `G1` with negative `E` delta → retraction (counted but not drawn).
   - `G2` / `G3` arcs → approximate as short line segments (default 0.5 mm chord, configurable).
5. **Detect layers:** use slicer markers when present (`;LAYER_CHANGE`, `;LAYER:`, `;Z:`), otherwise start a new layer when Z increases.
6. **Detect features:** read `;TYPE:` (PrusaSlicer, Orca, Bambu) and `;FEATURE:` (Cura) comments; map them to `Feature`. Unknown labels become `Feature::Other(String)`.
7. **Build the report:** count unknown commands, note any firmware dialect hints, and list warnings (see the PRD's S3 requirements).

Dialect handling: each slicer's comment conventions live in `slicer/` as a small detector plus a mapping table. Adding a slicer should mean adding one module and a test file.

## 5. Statistics

- **Time:** use the slicer's `;TIME:` or `; estimated printing time` comment if present, and label it "from slicer". Otherwise estimate as distance ÷ feedrate, plus a note that acceleration is ignored.
- **Filament length:** net sum of all E movement. Retractions and unretractions cancel out, so this matches the slicer's own filament figure.
- **Mass:** length × filament cross-section area × density. Diameter defaults to 1.75 mm (2.85 mm supported).
- **Travel:** sum of travel move lengths.

Every stat shows whether it came from the slicer or was calculated, so users can trust it.

## 6. Rendering

- Each visible layer range becomes one vertex buffer of line segments, generated once on load and stored per feature type.
- The layer slider sets a draw range, so changing layers does not re-upload data.
- Colors per feature come from a palette in `ui/palette.rs`, and the palette is colorblind-checked.
- Playback: a clock advances a "current move" index according to `time_s` scaled by a speed factor. The renderer draws moves up to that index.
- Camera: orbit (left drag), pan (right drag or Shift+drag), zoom (wheel). Keep the camera math in one module with unit tests.

## 7. Performance plan

| Target | Approach |
|---|---|
| Open 200 MB in under 3 s | Streaming parse; parse on a worker thread; show progress in the status bar |
| Smooth scrubbing | Pre-built per-layer buffers; no re-parse on slider change |
| Memory under 1 GB for 500 MB files | Compact `Move` struct (≈40 bytes); avoid storing raw text |
| Responsive UI during load | Parser runs on a thread; results sent over a channel |

Benchmarks live in `crates/gcode/benches/` and run with `criterion`. CI tracks the parse time on a fixed sample file.

## 8. Testing strategy

- **Unit tests:** parser lines, modal state transitions (G91 after G90, M83 with negative E), stat math.
- **Golden tests:** `testdata/` holds small G-code files per slicer. Expected output (layers, totals, feature counts) is stored as `insta` snapshots.
- **Accuracy tests:** compare total extrusion and time to the slicer's own footer comments on each sample, within 1%.
- **Fuzzing (later):** `cargo-fuzz` on the parser to catch panics on malformed files.
- **Manual QA checklist:** open each sample, scrub all layers, toggle travel, play animation, resize window, check for crashes.

## 9. Security and privacy

- No network access in the application. Verify with a firewall test and by confirming there are no HTTP dependencies in `Cargo.toml`.
- Files are read only; LayerLens never writes to G-code files.
- No telemetry, no accounts, no update checks in v1.
- Dependencies are reviewed with `cargo audit` in CI.

## 10. CI/CD

- **On every push and pull request:** `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, and `cargo audit`.
- **On version tags:** build a release `.exe`, attach it to a GitHub Release, and publish SHA-256 checksums.

## 11. Open technical questions

1. Is memory-mapping worth the complexity, or is buffered streaming fast enough? Decide after the first benchmark.
2. Should arcs be drawn as true arcs in the GPU buffer, or approximated? Start with approximation.
3. Which `egui` version to pin at project start? Check the latest stable release when scaffolding.
