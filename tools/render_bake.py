#!/usr/bin/env python3
"""Bake the render spike's terrain: a small heightmap per scenario from public elevation.

Source: NOAA NCEI ETOPO global relief (public domain), read through NOAA CoastWatch's ERDDAP
service (dataset etopo180, the 1 arc-minute ETOPO1 ice-surface grid). ETOPO 2022 is not offered
there as a grid service; its 60 arc-second tiles are a drop-in replacement for this script when
a download mirror is available. Land and lake surfaces are metres above sea level.

The download is cached in tools/.cache/ (gitignored). The output is one small file per scenario,
averaged down to 0.05 degree cells, written into crates/cw-render/assets/:

    heightmap-<scenario>.cwh   16-byte header then little-endian i16 metres, north row first
                               header: magic "CWH1", u16 width, u16 height, i16 west, i16 south,
                               i16 east, i16 north (whole degrees)

Run: python3 tools/render_bake.py            (all scenarios listed in AREAS)
"""

from __future__ import annotations

import io
import struct
import sys
import urllib.request
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent.parent
CACHE = ROOT / "tools" / ".cache"
OUT = ROOT / "crates" / "cw-render" / "assets"

# West, south, east, north in whole degrees: the scenario's states with room around them.
AREAS = {"great-lakes-2000": (-97, 34, -67, 50)}
SOURCE_PER_DEGREE = 60
CELL_DEGREES = 0.05
ERDDAP = "https://coastwatch.pfeg.noaa.gov/erddap/griddap/etopo180.csv"


def fetch_band(west: int, south: int, east: int, north: int) -> np.ndarray:
    """One latitude band of the source grid as a (rows, cols) int array, north row first."""
    cache = CACHE / f"etopo180-{west}_{south}_{east}_{north}.csv"
    if not cache.exists():
        query = f"?altitude%5B({south}):1:({north})%5D%5B({west}):1:({east})%5D"
        print(f"fetching {west},{south},{east},{north}", file=sys.stderr)
        with urllib.request.urlopen(ERDDAP + query, timeout=300) as r:
            CACHE.mkdir(parents=True, exist_ok=True)
            cache.write_bytes(r.read())
    rows = np.loadtxt(io.StringIO(cache.read_text()), delimiter=",", skiprows=2)
    lats = np.unique(rows[:, 0])
    lons = np.unique(rows[:, 1])
    grid = np.zeros((len(lats), len(lons)))
    li = np.searchsorted(lats, rows[:, 0])
    oi = np.searchsorted(lons, rows[:, 1])
    grid[li, oi] = rows[:, 2]
    return grid[::-1]


def bake(name: str, west: int, south: int, east: int, north: int) -> None:
    # The service includes both edges of each box, so drop the shared north row and east column.
    bands = [fetch_band(west, s, east, s + 4)[1:, :-1] for s in range(south, north, 4)]
    full = np.vstack(bands[::-1])
    step = int(round(CELL_DEGREES * SOURCE_PER_DEGREE))
    h, w = (full.shape[0] // step) * step, (full.shape[1] // step) * step
    cells = full[:h, :w].reshape(h // step, step, w // step, step).mean(axis=(1, 3))
    data = np.round(cells).astype("<i2")
    OUT.mkdir(parents=True, exist_ok=True)
    path = OUT / f"heightmap-{name}.cwh"
    path.write_bytes(b"CWH1" + struct.pack("<HHhhhh", data.shape[1], data.shape[0], west, south, east, north) + data.tobytes())
    print(f"{path} {data.shape[1]}x{data.shape[0]} min {data.min()} max {data.max()} {path.stat().st_size} bytes")


if __name__ == "__main__":
    for name, box in AREAS.items():
        bake(name, *box)
