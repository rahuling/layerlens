# LayerLens — Roadmap

Each milestone ends with something you can run and show. Estimates assume part-time evenings and weekends, and are rough.

## M0 — Setup (½ week)

- Create the Cargo workspace (`crates/gcode`, `crates/layerlens`).
- Add CI (fmt, clippy, test) and a `.gitignore`.
- Collect 3–5 small licence-clean G-code samples (one per major slicer) into `testdata/`.
- **Done when:** `cargo test` passes on an empty project and CI is green.

## M1 — Read a file (1–2 weeks)

- Streaming parser for `G0`, `G1`, `G90`, `G91`, `M82`, `M83`, `G92`.
- Modal state and move classification (travel, extrude, retract).
- Layer detection from Z changes.
- Window with file open and drag-and-drop.
- Top-down 2D view of the current layer, with a layer slider.
- Basic summary: layer count, bounding box, total extrusion length.
- **Done when:** you can open a sample and step through its layers.

## M2 — See it in 3D (2 weeks)

- wgpu line rendering in an egui paint callback.
- Orbit, pan, zoom, and reset view.
- Feature-type colors from slicer comments.
- Travel toggle.
- **Done when:** a sample renders in 3D at 60 fps, and the layer slider works in 3D.

## M3 — Stats and playback (1–2 weeks)

- Print time (slicer value when present, estimate otherwise), filament weight with material dropdown, travel distance, per-layer stats.
- Play, pause, speed control, progress indicator.
- Parse report panel (unknown commands, dialect detected).
- **Done when:** the summary matches the slicer's own footer values on the samples.

## M4 — Polish and first release, v1.0 (1–2 weeks)

- Support for all major slicer dialects in the corpus (accuracy test passing within 1%).
- Performance pass: 200 MB file opens in under 3 s; memory check on 500 MB.
- Error messages and empty states.
- README with screenshots and a GIF; CONTRIBUTING and issue templates.
- GitHub Actions release workflow producing a `.exe`.
- **Done when:** v1.0.0 is tagged and the `.exe` runs on a clean Windows 11 machine.

## v1.x — Comparison and extras (ongoing)

- S1: compare two files (side-by-side, then diff mode).
- S2: jump-to hotspots.
- S3: suspicious-move warnings.
- S4: export screenshots.
- S5–S6: material settings and saved views.

## Later

- `.3mf` support.
- STL overlay.
- Other platforms.
- Plugins.

## Milestone summary

| Milestone | Scope | Rough effort | Ships as |
|---|---|---|---|
| M0 | Setup | ½ week | — |
| M1 | 2D viewer and parser | 1–2 weeks | internal |
| M2 | 3D viewer | 2 weeks | internal |
| M3 | Stats and playback | 1–2 weeks | internal |
| M4 | v1.0 release | 1–2 weeks | GitHub Release |
| v1.x | Compare and extras | ongoing | point releases |
