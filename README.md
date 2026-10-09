# LayerLens

**See exactly what your printer will do.**

LayerLens is a free, offline G-code viewer for Windows. Open a file, scrub through the layers, play the print back, and check the time and filament before you start. It's fast on large files, sends no data anywhere, and needs no install.

> 🚧 Early development. Nothing to download yet. See [docs/ROADMAP.md](docs/ROADMAP.md).

## Planned features

- Drag and drop a `.gcode` file and see the toolpath in 3D
- Layer slider and animated playback
- Print time, filament length and weight, and travel distance per layer and in total
- Colors by feature (outer wall, infill, support, and more) from slicer comments
- Works with Bambu, Prusa, Cura, and Orca G-code
- Compare two versions of a print (v1.x)

## Why not a browser viewer?

Browser viewers are handy, but they can struggle with large files, depend on your connection, and rarely compare two versions. LayerLens runs natively and stays on your machine.

## Building from source

Requires the stable Rust toolchain on Windows 10 or 11.

```bash
git clone https://github.com/rahuling/layerlens
cd layerlens
cargo run --release -p layerlens -- path/to/file.gcode
```

## Contributing

Sample G-code files are very welcome, especially from slicers and printers we don't cover yet. Please only submit files you have the right to share. See [CONTRIBUTING.md](CONTRIBUTING.md) (coming with v1.0).

## License

MIT. See [LICENSE](LICENSE).
