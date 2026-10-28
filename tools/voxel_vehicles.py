#!/usr/bin/env python3
"""Bake the vehicles on the map: trucks, cars, trains, cargo ships and cargo planes.

Each vehicle is a tiny voxel model in the same front-on, cel-shaded style as the buildings
(tools/voxel_sprites.py): lit roof, darker front, ink where colours meet and round the
silhouette. The model is turned in the ground plane to eight headings (N, NE, E, SE, S, SW, W,
NW) before the same front-on projection, so a vehicle can face the way it moves on screen. Each
comes in two liveries: neutral for rivals and traffic, and the player's, with a gold cab or band.
Every frame is rendered at three pixel scales so the map draws it one to one, crisp at any zoom.

Outputs, into apple/Resources (the Compose build copies them; both apps read the same files):

    vehicles-1.png, vehicles-2.png, vehicles-3.png   sprite atlases at 1, 2 and 3 pixels per half voxel
    vehicles.json                                     frame rectangles and ground anchors per scale

    python3 tools/voxel_vehicles.py
"""

from __future__ import annotations

import json
import math
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

from voxel_sprites import INK, OUT, SCALES, Model, rgb, tone

HEADINGS = ("N", "NE", "E", "SE", "S", "SW", "W", "NW")
LIVERIES = ("neutral", "player")
GOLD = "#E2B04A"


# ---- the models --------------------------------------------------------------------------------
# Every model points east (+x) and is centred on y = 0; z is up. The player's livery swaps in gold.

def truck(mine):
    m = Model().box(0, -1, 0, 7, 2, 1, "#20262C")
    m.box(0, -1, 1, 5, 2, 4, "#D8DCE0").box(0, -1, 4, 5, 2, 5, "#AEB4BA")
    m.box(5, -1, 1, 7, 2, 3, GOLD if mine else "#4F7FAF").box(6, -1, 3, 7, 2, 4, "#9FD3F0")
    m.box(5, -1, 3, 6, 2, 4, GOLD if mine else "#4F7FAF")
    return m


def car(mine):
    m = Model().box(0, -1, 0, 4, 1, 1, "#20262C")
    m.box(0, -1, 1, 4, 1, 2, GOLD if mine else "#C24A40")
    m.box(1, -1, 2, 3, 1, 3, "#9FD3F0")
    return m


def loco(mine):
    m = Model().box(0, -1, 0, 7, 2, 1, "#20262C")
    m.box(0, -1, 1, 7, 2, 3, "#3E6E8E").box(1, -1, 3, 5, 2, 4, "#5A646E")
    m.box(5, -1, 1, 7, 2, 4, GOLD if mine else "#E5D04A").box(6, -1, 3, 7, 2, 4, "#9FD3F0")
    return m


def wagon(mine):
    m = Model().box(0, -1, 0, 6, 2, 1, "#20262C")
    m.box(0, -1, 1, 6, 2, 3, "#8A5A3C").box(0, -1, 3, 6, 2, 4, "#6E4630")
    if mine:
        m.box(0, -1, 2, 6, 2, 3, GOLD)
    return m


def ship(mine):
    m = Model()
    for x in range(12):
        half = 2 if x < 10 else (1 if x == 10 else 0)
        if half == 0:
            m.box(x, 0, 0, x + 1, 1, 2, "#2E4A66")
            continue
        m.box(x, -half, 0, x + 1, half, 1, "#7A2E2A").box(x, -half, 1, x + 1, half, 2, GOLD if mine else "#2E4A66")
    for i, colour in enumerate(("#C24A40", "#3F8FBF", "#3FA06A", "#E0A040")):
        m.box(3 + 2 * i, -2, 2, 5 + 2 * i, 2, 3, colour)
    m.box(0, -2, 2, 2, 2, 5, "#E8ECF0").box(0, -2, 5, 2, 2, 6, "#5A646E")
    return m


def plane(mine):
    body = GOLD if mine else "#E8ECF0"
    m = Model().box(0, -1, 1, 8, 1, 3, "#E8ECF0").box(7, -1, 2, 8, 1, 3, "#9FD3F0")
    m.box(3, -4, 1, 5, 4, 2, "#B8C0C8")
    m.box(0, -2, 2, 1, 2, 3, "#B8C0C8").box(0, -1, 3, 1, 1, 5, body)
    if mine:
        m.box(1, -1, 2, 7, 1, 3, GOLD)
    return m


MODELS = {f.__name__: f for f in (truck, car, loco, wagon, ship, plane)}


# ---- turning and rendering ---------------------------------------------------------------------

def turned(m: Model, heading: int) -> Model:
    """The model turned to face compass heading [heading] x 45 degrees, resampled on the grid.

    Each target voxel looks back through the rotation to the model's voxel under its centre, so
    the diagonals come out as clean voxel staircases rather than smeared pixels.
    """
    # The model faces east (compass 90); turn it anticlockwise by 90 minus the heading.
    a = math.radians(90 - 45 * heading)
    ca, sa = math.cos(a), math.sin(a)
    xs = [k[0] + 0.5 for k in m.v]
    ys = [k[1] + 0.5 for k in m.v]
    cx = (min(xs) + max(xs)) / 2
    cy = (min(ys) + max(ys)) / 2
    reach = int(math.ceil(max(max(xs) - cx, cx - min(xs), max(ys) - cy, cy - min(ys)) * 1.5)) + 1
    out = Model()
    zs = sorted({k[2] for k in m.v})
    for tx in range(-reach, reach):
        for ty in range(-reach, reach):
            # Undo the turn for the centre of this cell.
            px, py = tx + 0.5, ty + 0.5
            sx = px * ca + py * sa + cx
            sy = -px * sa + py * ca + cy
            ix, iy = math.floor(sx), math.floor(sy)
            for z in zs:
                c = m.v.get((ix, iy, z))
                if c is not None:
                    out.v[(tx, ty, z)] = c
    return out


def render(m: Model, s: int):
    """The model at s pixels per voxel step, front on and cel shaded, and its ground anchor.

    The same projection as the buildings: a voxel shows its roof (2s wide, s deep) and its front
    face (2s wide, 2s tall), looking north and slightly down.
    """
    x0, x1, y0, y1 = m.footprint()
    top = max(k[2] for k in m.v) + 1
    w = (x1 - x0) * 2 * s + 4
    h = (y1 - y0) * s + top * 2 * s + 4
    img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    ids = Image.new("L", (w, h), 0)
    d = ImageDraw.Draw(img)
    di = ImageDraw.Draw(ids)
    base = h - 2
    material: dict = {}

    def at(x, y, z):
        return 2 + (x - x0) * 2 * s, base - (y - y0) * s - z * 2 * s

    for (x, y, z), colour in sorted(m.v.items(), key=lambda kv: (-kv[0][1], kv[0][2])):
        px, py = at(x, y, z)
        front = [(px, py - 2 * s), (px + 2 * s - 1, py - 2 * s), (px + 2 * s - 1, py - 1), (px, py - 1)]
        roof = [(px, py - 3 * s), (px + 2 * s - 1, py - 3 * s), (px + 2 * s - 1, py - 2 * s - 1), (px, py - 2 * s - 1)]
        mid = material.setdefault(colour, len(material) + 1)
        if (x, y, z + 1) not in m.v:
            d.polygon(roof, fill=tone(colour, 1.0)); di.polygon(roof, fill=mid)
        if (x, y - 1, z) not in m.v:
            d.polygon(front, fill=tone(colour, 0.68)); di.polygon(front, fill=mid)

    a = np.array(img)
    solid = a[..., 3] > 0
    # Ink where colours meet only from scale 2: at one pixel a seam would eat the whole detail.
    if s >= 2:
        idv = np.array(ids).astype(int)
        seam = np.zeros_like(solid)
        seam[:, :-1] |= (idv[:, :-1] != idv[:, 1:]) & solid[:, :-1] & solid[:, 1:]
        seam[:-1, :] |= (idv[:-1, :] != idv[1:, :]) & solid[:-1, :] & solid[1:, :]
        a[seam, :3] = (a[seam, :3] * 0.45).astype(np.uint8)
    padded = np.pad(solid, 1)
    ring = (padded[1:-1, 2:] | padded[1:-1, :-2] | padded[2:, 1:-1] | padded[:-2, 1:-1])
    a[ring & ~solid] = (*INK, 255)
    rows = np.where(a[..., 3].any(axis=1))[0]
    cols = np.where(a[..., 3].any(axis=0))[0]
    top_, bottom, left, right = rows[0], rows[-1] + 1, cols[0], cols[-1] + 1
    a = a[top_:bottom, left:right]
    # The ground anchor: the middle of the footprint at ground level.
    ax = 2 + (x1 - x0) * s
    ay = base - (y1 - y0) * s / 2
    return Image.fromarray(a), (float(ax - left), float(ay - top_))


def main() -> None:
    names = [(kind, livery, h) for kind in MODELS for livery in LIVERIES for h in range(len(HEADINGS))]
    models = {n: turned(MODELS[n[0]](n[1] == "player"), n[2]) for n in names}
    frames: dict = {}
    for s in SCALES:
        sprites = [(n, render(models[n], s)) for n in names]
        width = 16 * 2 * 16 * s
        x = y = row = 0
        placed = []
        for n, (img, anchor) in sprites:
            if x + img.width > width:
                x, y, row = 0, y + row + 2, 0
            placed.append((n, img, anchor, x, y))
            x += img.width + 2
            row = max(row, img.height)
        sheet = Image.new("RGBA", (width, y + row), (0, 0, 0, 0))
        for n, img, anchor, px, py in placed:
            sheet.paste(img, (px, py))
            key = f"{n[0]}/{n[1]}/{HEADINGS[n[2]]}"
            frames.setdefault(key, {})[str(s)] = {"x": px, "y": py, "w": img.width, "h": img.height, "ax": anchor[0], "ay": anchor[1]}
        sheet.save(OUT / f"vehicles-{s}.png", optimize=True)
    data = {
        "note": "Voxel vehicles, cel shaded, one frame per type, livery (neutral or player) and heading (N, NE, E, SE, S, SW, "
                "W, NW as seen on screen) at each pixel scale. ax, ay is the ground anchor in the frame. "
                "Generated by tools/voxel_vehicles.py.",
        "scales": list(SCALES),
        "headings": list(HEADINGS),
        "frames": frames,
    }
    (OUT / "vehicles.json").write_text(json.dumps(data, indent=1) + "\n")
    print(f"{len(names)} vehicle frames at scales {SCALES} -> {OUT}")


if __name__ == "__main__":
    main()
