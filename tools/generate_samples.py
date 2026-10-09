#!/usr/bin/env python3
"""Generate synthetic G-code samples for LayerLens tests.

The files are written by this script and released under the repo's MIT
licence, so they are safe to publish. Each file imitates one slicer's comment
and extrusion conventions so the parser can be tested against them.

Footer totals and expected.json are computed from the generated moves, so
they are exact expected values. Real slicers also model acceleration, so their
time estimates will differ slightly; these samples do not.

Usage: python tools/generate_samples.py [output_dir]
"""

import json
import math
import pathlib
import sys

LAYER_HEIGHT = 0.2
LAYERS = 10
SIZE = 20.0
INSET = 0.45  # inner wall offset, one extrusion width
LINE_SPACING = 0.45  # infill line spacing
EXTRUSION_WIDTH = 0.45
FILAMENT_AREA = math.pi * (1.75 / 2) ** 2  # mm^2 for 1.75 mm filament
E_PER_MM = EXTRUSION_WIDTH * LAYER_HEIGHT / FILAMENT_AREA
RETRACT_MM = 0.8
RETRACT_FEED = 2400.0
TRAVEL_FEED = 9000.0
PRINT_FEED = 1800.0
Z_FEED = 720.0


class Writer:
    """Tracks position, extrusion and time while emitting G-code lines."""

    def __init__(self, e_mode="M82", inline_notes=False):
        self.lines = []
        self.e_mode = e_mode
        self.inline_notes = inline_notes
        self.x = self.y = self.z = 0.0
        self.e_pos = 0.0  # absolute E position, used in M82 mode
        self.net_filament = 0.0  # net E movement; retractions cancel out
        self.seconds = 0.0
        self.retracted = False
        self.rel_xy = False
        self.label = None
        self.count = 0
        self.bounds_min = [None, None, None]
        self.bounds_max = [None, None, None]

    def _track_point(self, x, y, z):
        for i, v in enumerate((x, y, z)):
            self.bounds_min[i] = v if self.bounds_min[i] is None else min(self.bounds_min[i], v)
            self.bounds_max[i] = v if self.bounds_max[i] is None else max(self.bounds_max[i], v)

    def _track(self):
        self._track_point(self.x, self.y, self.z)

    def raw(self, text):
        self.lines.append(text)

    def comment(self, text):
        self.lines.append(text)

    def emit(self, text):
        self.count += 1
        if self.inline_notes and self.count % 37 == 0:
            text += " ; check"
        self.lines.append(text)

    def _xy(self, x, y):
        if self.rel_xy:
            return f"X{x - self.x:.3f} Y{y - self.y:.3f}"
        return f"X{x:.3f} Y{y:.3f}"

    def _e(self, de):
        self.net_filament += de
        if self.e_mode == "M82":
            self.e_pos += de
            return f"E{self.e_pos:.5f}"
        return f"E{de:.5f}"

    def _move_to(self, x, y):
        self.x, self.y = x, y
        self._track()

    def set_relative(self, on):
        self.rel_xy = on
        self.raw("G91" if on else "G90")

    def set_feature(self, label):
        if label != self.label:
            self.label = label
            self.comment(f";TYPE:{label}")

    def reset_e(self):
        self.e_pos = 0.0
        self.raw("G92 E0")

    def set_z(self, z):
        self.seconds += abs(z - self.z) / (Z_FEED / 60)
        self.emit(f"G1 Z{z:.3f} F{Z_FEED:.0f}")
        self.z = z
        self._track()

    def retract(self):
        if not self.retracted:
            self.emit(f"G1 {self._e(-RETRACT_MM)} F{RETRACT_FEED:.0f}")
            self.retracted = True

    def unretract(self):
        if self.retracted:
            self.emit(f"G1 {self._e(RETRACT_MM)} F{RETRACT_FEED:.0f}")
            self.retracted = False

    def travel(self, x, y):
        self.retract()
        dist = math.hypot(x - self.x, y - self.y)
        self.emit(f"G0 {self._xy(x, y)} F{TRAVEL_FEED:.0f}")
        self.seconds += dist / (TRAVEL_FEED / 60)
        self._move_to(x, y)

    def extrude(self, x, y):
        self.unretract()
        dist = math.hypot(x - self.x, y - self.y)
        self.emit(f"G1 {self._xy(x, y)} {self._e(dist * E_PER_MM)} F{PRINT_FEED:.0f}")
        self.seconds += dist / (PRINT_FEED / 60)
        self._move_to(x, y)

    def arc_semicircle(self, x, y, i, j):
        """Clockwise semicircle from the current position to (x, y) with centre offset (i, j)."""
        assert math.isclose(x, self.x + 2 * i, abs_tol=1e-6) and math.isclose(y, self.y + 2 * j, abs_tol=1e-6)
        self.unretract()
        # The arc bulges past its end points, so sample it for the bounding box.
        r = math.hypot(i, j)
        cx, cy = self.x + i, self.y + j
        start_angle = math.atan2(-j, -i)
        for k in range(1, 33):
            a = start_angle - math.pi * k / 32
            self._track_point(cx + r * math.cos(a), cy + r * math.sin(a), self.z)
        length = math.pi * r
        self.emit(f"G2 {self._xy(x, y)} I{i:.3f} J{j:.3f} {self._e(length * E_PER_MM)} F{PRINT_FEED:.0f}")
        self.seconds += length / (PRINT_FEED / 60)
        self._move_to(x, y)


def perimeter(w, inset):
    i, s = inset, SIZE
    corners = [(i, i), (s - i, i), (s - i, s - i), (i, s - i), (i, i)]
    w.travel(*corners[0])
    for c in corners[1:]:
        w.extrude(*c)


def zigzag(w):
    """Back-and-forth infill lines inside the inner wall, drawn as one continuous path."""
    start, end = 2 * INSET, SIZE - 2 * INSET
    pts = []
    y, k = start, 0
    while y <= end + 1e-9:
        xa, xb = (start, end) if k % 2 == 0 else (end, start)
        pts += [(xa, y), (xb, y)]
        y += LINE_SPACING
        k += 1
    w.travel(*pts[0])
    for p in pts[1:]:
        w.extrude(*p)


def edge_extras(w, n, z):
    """Unusual constructs for the edge-case file, placed on specific layers."""
    if n == 2:
        w.raw("T0")
        w.raw("M117 Layer 3 of 10")
        w.raw("M900 K0.04")
    if n == 3:
        w.set_feature("Custom ironing")
        w.travel(5, 5)
        w.arc_semicircle(9, 5, 2, 0)
    if n == 5:
        w.reset_e()
        w.set_z(z + 0.4)  # z-hop
        w.travel(15, 15)
        w.set_z(z)


def start_lines(e_mode):
    mode = "absolute" if e_mode == "M82" else "relative"
    return [
        "G21 ; millimetres",
        "G90 ; absolute positioning",
        f"{e_mode} ; {mode} extrusion",
        "M140 S60 ; bed temperature",
        "M104 S215 ; nozzle temperature",
        "M109 S215 ; wait for nozzle",
        "G28 ; home",
        "G92 E0",
    ]


def hms(seconds):
    total = int(round(seconds))
    return f"{total // 3600}h {total % 3600 // 60}m {total % 60}s"


def prusa_layer(n, z):
    return [";LAYER_CHANGE", f";Z:{z:.3f}", f";HEIGHT:{LAYER_HEIGHT:.3f}"]


def bambu_layer(n, z):
    return [";LAYER_CHANGE", f";Z:{z:.3f}", f";HEIGHT:{LAYER_HEIGHT:.3f}"]


def cura_layer(n, z):
    return [f";LAYER:{n}"]


DIALECTS = {
    "prusa": {
        "layer": prusa_layer,
        "labels": ("External perimeter", "Perimeter", "Internal infill"),
        "reset_e_per_layer": False,
        "footer": lambda w: [
            f"; estimated printing time (normal mode) = {hms(w.seconds)}",
            f"; filament used [mm] = {w.net_filament:.2f}",
        ],
    },
    "bambu": {
        "layer": bambu_layer,
        "labels": ("Outer wall", "Inner wall", "Sparse infill"),
        "reset_e_per_layer": False,
        "footer": lambda w: [
            f"; model printing time: {hms(w.seconds)}; total estimated time: {hms(w.seconds)}",
            f"; total filament length [mm] : {w.net_filament:.2f}",
        ],
    },
    "cura": {
        "layer": cura_layer,
        "labels": ("WALL-OUTER", "WALL-INNER", "FILL"),
        "reset_e_per_layer": True,
        "footer": lambda w: [f";Filament used: {w.net_filament / 1000:.5f}m"],
        "header": lambda w: [f";TIME:{int(round(w.seconds))}"],
    },
}


def build(dialect_name, e_mode="M82", rel_infill=False, edge=False, inline=False):
    d = DIALECTS[dialect_name]
    w = Writer(e_mode=e_mode, inline_notes=inline)
    for line in start_lines(e_mode):
        w.raw(line)
    outer, inner, infill = d["labels"]
    for n in range(LAYERS):
        z = round((n + 1) * LAYER_HEIGHT, 3)
        for line in d["layer"](n, z):
            w.comment(line)
        if d["reset_e_per_layer"]:
            w.reset_e()
        w.set_z(z)
        w.set_feature(outer)
        perimeter(w, 0.0)
        w.set_feature(inner)
        perimeter(w, INSET)
        w.set_feature(infill)
        if rel_infill:
            w.set_relative(True)
        zigzag(w)
        if rel_infill:
            w.set_relative(False)
        if edge:
            edge_extras(w, n, z)
        w.raw("")
    footer = d["footer"](w)
    header = d["header"](w) if "header" in d else []
    lines = header + w.lines + footer
    expected = {
        "layers": LAYERS,
        "z_max": round(LAYERS * LAYER_HEIGHT, 3),
        "bounds_min": [round(v, 3) for v in w.bounds_min],
        "bounds_max": [round(v, 3) for v in w.bounds_max],
        "filament_mm": round(w.net_filament, 2),
        "time_s": round(w.seconds),
    }
    return lines, expected


FILES = [
    # name, dialect, e_mode, rel_infill, edge, inline
    ("prusa_cube.gcode", "prusa", "M82", False, False, False),
    ("bambu_cube.gcode", "bambu", "M82", False, False, False),
    ("cura_cube.gcode", "cura", "M82", False, False, False),
    ("relative_e_cube.gcode", "prusa", "M83", True, False, False),
    ("edge_cases.gcode", "prusa", "M82", False, True, True),
]


def main():
    default_out = pathlib.Path(__file__).resolve().parent.parent / "testdata" / "synthetic"
    out = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else default_out
    out.mkdir(parents=True, exist_ok=True)
    manifest = {}
    for name, dialect, e_mode, rel, edge, inline in FILES:
        lines, expected = build(dialect, e_mode, rel, edge, inline)
        # edge_cases uses CRLF line endings on purpose
        newline = "\r\n" if name == "edge_cases.gcode" else "\n"
        with open(out / name, "w", newline=newline, encoding="ascii") as f:
            f.write("\n".join(lines) + "\n")
        manifest[name] = expected
        print(f"wrote {out / name}")
    with open(out / "expected.json", "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2)
        f.write("\n")
    print(f"wrote {out / 'expected.json'}")


if __name__ == "__main__":
    main()
