#!/usr/bin/env python3
"""Bake the render crate's real city footprints from Natural Earth urban areas.

Source: Natural Earth 1:10m urban areas (public domain), https://www.naturalearthdata.com/,
https://naciscdn.org/naturalearth/10m/cultural/ne_10m_urban_areas.zip . Polygons that touch a
scenario's terrain box are kept, thinned with Douglas-Peucker to about 400 m, and written to
crates/cw-render/assets/urban-<scenario>.json as integer ten-thousandths of a degree, the same
unit as the bundled world map. Each area keeps its size in square kilometres.

    python3 tools/render_bake_urban.py [path/to/ne_10m_urban_areas.shp]

With no path the zip is downloaded into tools/.cache/ (gitignored).
"""

from __future__ import annotations

import io
import json
import sys
import urllib.request
import zipfile
from pathlib import Path

import shapefile

ROOT = Path(__file__).resolve().parent.parent
CACHE = ROOT / "tools" / ".cache"
OUT = ROOT / "crates" / "cw-render" / "assets"
URL = "https://naciscdn.org/naturalearth/10m/cultural/ne_10m_urban_areas.zip"
# West, south, east, north in whole degrees, the heightmap box of render_bake.py.
AREAS = {"great-lakes-2000": (-97, 34, -67, 50)}
TOLERANCE = 0.004


def simplify(points, tolerance):
    """Douglas-Peucker on lon, lat pairs."""
    if len(points) < 4:
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


def open_reader(path: str | None) -> shapefile.Reader:
    if path:
        return shapefile.Reader(path)
    CACHE.mkdir(parents=True, exist_ok=True)
    zpath = CACHE / "ne_10m_urban_areas.zip"
    if not zpath.exists():
        zpath.write_bytes(urllib.request.urlopen(URL, timeout=120).read())
    z = zipfile.ZipFile(zpath)
    parts = {n.rsplit(".", 1)[1]: io.BytesIO(z.read(n)) for n in z.namelist() if n.endswith((".shp", ".shx", ".dbf"))}
    return shapefile.Reader(shp=parts["shp"], shx=parts["shx"], dbf=parts["dbf"])


def main() -> None:
    reader = open_reader(sys.argv[1] if len(sys.argv) > 1 else None)
    for scenario, (w, s, e, n) in AREAS.items():
        areas = []
        for sr in reader.iterShapeRecords():
            b = sr.shape.bbox
            if b[2] < w or b[0] > e or b[3] < s or b[1] > n:
                continue
            parts = list(sr.shape.parts) + [len(sr.shape.points)]
            rings = []
            for a, z in zip(parts, parts[1:]):
                pts = [tuple(p) for p in sr.shape.points[a:z]][:-1]
                # Split the closed ring in two so Douglas-Peucker has a real chord to work from.
                h = len(pts) // 2
                ring = simplify(pts[: h + 1], TOLERANCE)[:-1] + simplify(pts[h:] + pts[:1], TOLERANCE)[:-1]
                if len(ring) >= 3:
                    rings.append([int(round(v * 10_000)) for p in ring for v in p])
            if rings:
                areas.append({"sqkm": round(float(sr.record["area_sqkm"]), 1), "rings": rings})
        areas.sort(key=lambda a: -a["sqkm"])
        path = OUT / f"urban-{scenario}.json"
        path.write_text(json.dumps({"scenario": scenario, "units": "1e-4 degrees, lon then lat", "areas": areas}, separators=(",", ":")))
        print(f"{path.relative_to(ROOT)}: {len(areas)} areas, {path.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()
