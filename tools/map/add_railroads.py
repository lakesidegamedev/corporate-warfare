#!/usr/bin/env python3
"""Add real railways round each scenario to the bundled world map, for the trains to run on.

Source: Natural Earth 1:10m railroads (public domain), https://www.naturalearthdata.com/,
https://naciscdn.org/naturalearth/10m/cultural/ne_10m_railroads.zip . Lines of scale rank 9 or
below (the main lines) within each scenario's states padded by three degrees are kept, thinned
with Douglas-Peucker to about a kilometre, and written to world-map.json as "railroads", tagged
with their scenario like the interstates (the default scenario's untagged).

    python3 tools/map/add_railroads.py ~/Downloads/ne_10m_railroads.shp
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import shapefile

ROOT = Path(__file__).resolve().parents[2]
MAP = ROOT / "apple" / "Resources" / "world-map.json"
PAD = 3.0
TOLERANCE = 0.01  # degrees, about a kilometre


def simplify(points, tolerance):
    """Douglas-Peucker on lon, lat pairs."""
    if len(points) < 3:
        return points
    keep = [False] * len(points)
    keep[0] = keep[-1] = True
    stack = [(0, len(points) - 1)]
    while stack:
        a, b = stack.pop()
        (x0, y0), (x1, y1) = points[a], points[b]
        dx, dy = x1 - x0, y1 - y0
        length = (dx * dx + dy * dy) ** 0.5 or 1e-12
        best, at = 0.0, -1
        for i in range(a + 1, b):
            x, y = points[i]
            d = abs(dy * x - dx * y + x1 * y0 - y1 * x0) / length
            if d > best:
                best, at = d, i
        if best > tolerance and at > 0:
            keep[at] = True
            stack += [(a, at), (at, b)]
    return [p for p, k in zip(points, keep) if k]


def bounds_of(document, scenario):
    states = set(scenario["states"])
    w, s, e, n = 180.0, 90.0, -180.0, -90.0
    for state in document["states"]:
        tag = state.get("scenario")
        if state["code"] in states and (tag is None or tag == scenario["id"]):
            for ring in state["rings"]:
                for i in range(0, len(ring) - 1, 2):
                    lon, lat = ring[i] / 10_000, ring[i + 1] / 10_000
                    w, e, s, n = min(w, lon), max(e, lon), min(s, lat), max(n, lat)
    return w - PAD, s - PAD, e + PAD, n + PAD


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("shapefile", type=Path)
    args = parser.parse_args()
    document = json.loads(MAP.read_text())
    reader = shapefile.Reader(str(args.shapefile))
    rank = [f[0] for f in reader.fields[1:]].index("scalerank")
    shapes = [(rec[rank], shape) for rec, shape in zip(reader.records(), reader.shapes())]
    default = document["scenarios"][0]["id"]
    railroads = []
    for scenario in document["scenarios"]:
        w, s, e, n = bounds_of(document, scenario)
        lines = []
        for scalerank, shape in shapes:
            if scalerank > 9:
                continue
            x0, y0, x1, y1 = shape.bbox
            if x1 < w or x0 > e or y1 < s or y0 > n:
                continue
            parts = list(shape.parts) + [len(shape.points)]
            for a, b in zip(parts, parts[1:]):
                points = simplify([tuple(p) for p in shape.points[a:b]], TOLERANCE)
                if len(points) >= 2:
                    lines.append([v for lon, lat in points for v in (round(lon * 10_000), round(lat * 10_000))])
        entry = {"name": "", "lines": lines}
        if scenario["id"] != default:
            entry["scenario"] = scenario["id"]
        railroads.append(entry)
        print(scenario["id"], len(lines), "lines", sum(len(l) // 2 for l in lines), "points")
    document["railroads"] = railroads
    MAP.write_text(json.dumps(document, separators=(",", ":")) + "\n")
    print(MAP, MAP.stat().st_size // 1024, "KiB")


if __name__ == "__main__":
    main()
