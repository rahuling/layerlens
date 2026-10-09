# LayerLens — Product Requirements Document

**Status:** Draft v0.1
**Owner:** rahuling
**Last updated:** 2026-10-09

---

## 1. Summary

LayerLens is a free, open-source desktop app for Windows that lets 3D-printing hobbyists open a G-code file and see exactly what the printer will do: layer by layer, move by move, with the print time and filament use at each point. It runs fully offline, opens very large files quickly, and can compare two versions of a print side by side.

## 2. Problem

People who print regularly run into the same questions:

- "Did the slicer do what I expected?" (supports placed in the wrong spot, a seam in the wrong place, a missed bridge)
- "What changed between my last two slices?" (new profile, new support settings)
- "Where exactly is it going to fail?" (a thin wall, a long travel, an overhang)
- "Can I trust this file before I burn 14 hours and 80 g of filament?"

Existing options:

- **Slicer previews** (PrusaSlicer, Cura, Bambu Studio, OrcaSlicer) show the file, but are tied to one slicer and are hard to use for comparing two versions.
- **Browser viewers** (PrintPal, A23D, and similar) are convenient but upload or depend on the browser, struggle with multi-hundred-MB files, and rarely have a compare mode.
- **Desktop Python viewers** (e.g. LayerView) are dated, slow on big files, and hard to install on Windows.

## 3. Goals

1. Open a typical G-code file (up to ~500 MB) in under 3 seconds on a mid-range Windows PC, and show the first layer.
2. Let the user scrub through layers and play the print at adjustable speed.
3. Show per-layer and whole-print stats: print time, filament length and weight, travel distance, layer count, and retraction count.
4. Work with the main slicers' G-code, including Bambu, Prusa, Cura, Orca, and Klipper-style output.
5. Compare two G-code files, highlighting added, removed, and changed moves.
6. Run with no internet connection and send no data anywhere.
7. Ship as a single `.exe` that needs no installer and no admin rights.

## 4. Non-goals (v1)

- Editing or re-slicing G-code.
- Sending files to a printer or controlling one (no OctoPrint/Moonraker integration yet).
- Rendering `.stl` / `.3mf` model files directly. (`.3mf` may be added later for its embedded thumbnails and metadata.)
- macOS and Linux builds. The code should not block them, but they are not a v1 target.
- Simulating physical behavior (warping, flow, temperature). LayerLens shows what the file says, not what the printer will do.

## 5. Target users

| Persona | What they need |
|---|---|
| **Hobbyist tinkerer** | Quickly check a file before printing, and see what changed after a profile tweak. |
| **Print seller** | Verify the slicing of a product before a batch, and estimate time and filament per unit. |
| **Learner** | See how a slicer turns a model into layers and moves. |

## 6. User stories and requirements

### Must have (v1.0)

| ID | As a… | I want to… | So that… | Acceptance criteria |
|---|---|---|---|---|
| M1 | user | open a `.gcode` file by drag-and-drop or File → Open | I can start quickly | Dropping one file opens it; invalid files show a clear error |
| M2 | user | move a layer slider and see that layer rendered | I can inspect any height | Slider moves through all layers; the view updates within 100 ms |
| M3 | user | see the toolpath colored by feature type (outer wall, inner wall, infill, support, skirt, travel) | I can tell what each part is | Colors come from slicer comments when present; otherwise a fallback "extrusion" color is used |
| M4 | user | orbit, pan, and zoom the 3D view | I can look from any angle | Mouse and keyboard controls; "reset view" button |
| M5 | user | see a summary panel | I know the cost of the print | Shows total time (from slicer comment if available, else estimated), filament length and weight (weight needs density, default PLA 1.24 g/cm³), layer count, and bounding box |
| M6 | user | toggle travel moves on and off | I can see unnecessary movement | Travel shown as a faint line when enabled |
| M7 | user | play the print as an animation | I can watch the sequence | Play / pause, speed control, and a progress indicator |
| M8 | user | see stats for the currently visible layer | I can find the slow or heavy layers | Layer time, filament, and travel shown for the selected layer |
| M9 | user | open the app offline | I can use it anywhere | No network calls at runtime; verified in testing |

### Should have (v1.x)

| ID | As a… | I want to… | So that… | Acceptance criteria |
|---|---|---|---|---|
| S1 | user | compare two G-code files | I can see what changed | Side-by-side view plus a "diff" mode highlighting added (green), removed (red), and unchanged (grey) moves, with a layer-by-layer summary of differences |
| S2 | user | jump to the layer with the longest time or most filament | I can find hotspots | "Jump to" buttons for max time, max filament, and max travel |
| S3 | user | flag suspicious moves | I can catch problems early | Warnings for: moves outside the bed, extrusion while the nozzle is cold, very long travels across the part, and layers with zero extrusion |
| S4 | user | export a screenshot or short video of a layer or animation | I can share results | PNG export at the current view; MP4 optional |
| S5 | user | set the filament type and density | weight estimates are accurate | Dropdown of common materials plus a custom density field |
| S6 | user | save a view (layer, camera angle) | I can return to it | Named bookmarks stored locally |

### Could have (later)

- `.3mf` support, including the embedded thumbnail and print metadata.
- Load a model's STL alongside the G-code to check how the slicer placed it.
- Plugin hooks for custom analysis.
- A light theme and accessibility options (colorblind-safe palette).
- Other platforms (Linux, macOS).

## 7. Success metrics

- **Performance:** a 200 MB G-code file opens in under 3 s and scrubbing stays above 30 fps on a GTX 1650-class GPU.
- **Correctness:** total extruded length and time match the slicer's own reported values to within 1% on a set of 10 test files from at least 3 slicers.
- **Adoption (6 months after launch):** 500 GitHub stars and 10 external contributors. These are aspirational targets, not commitments.
- **Quality:** zero known crashes on the test corpus; no network traffic in the offline check.

## 8. Constraints and assumptions

- **Platform:** Windows 10 and 11, 64-bit. Requires a GPU with DirectX 12 or Vulkan support.
- **Licence:** MIT (permissive, easy for others to use and contribute to).
- **Language:** Rust, to learn the language while building a real tool.
- **Money:** free and open source, no accounts, no telemetry.
- **Assumption:** most G-code is ASCII text, and slicers mark layers and feature types in comments. The parser must still work when those comments are absent, with a degraded but useful view.

## 9. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Slicer G-code dialects differ (comments, arcs, relative extrusion) | Wrong paths or stats | Build a test corpus from each slicer early; add a "parse report" that lists unknown commands |
| Very large files use too much memory | Crash or slow load | Stream-parse into a compact vertex buffer; load layers lazily |
| 3D rendering is hard to get right | Schedule slip | Start with a 2D top-down view in milestone 1; add 3D in milestone 2 |
| Name or trademark conflicts | Rename later | Check name availability on GitHub, crates.io, and the Microsoft Store before public launch |

## 10. Open questions

1. Should v1 include the diff/compare mode, or should it wait for v1.x? (Current plan: v1.x.)
2. Is a Microsoft Store listing wanted, or only GitHub Releases? (Current plan: GitHub Releases first.)
3. Which 10 G-code files should form the public test corpus? Files must be licence-clean to share.
4. Is an icon and branding needed before launch, or can the first release use a placeholder?
