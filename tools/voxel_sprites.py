#!/usr/bin/env python3
"""Bake the voxel buildings that stand for the player's divisions on the map.

Each of the fifteen industries has a small voxel model in three sizes (a division grows from
small to large with its plant count), drawn in 2:1 dimetric view with cel shading: three flat
tones per colour (lit top, left, darker right), ink lines where colours meet, and a dark outline
round the silhouette. Every model is rendered at three pixel scales so the map can always draw a
sprite one to one, crisp at any zoom, without resampling.

Outputs, into apple/Resources (the Compose build copies them; both apps read the same files):

    voxels-1.png, voxels-2.png, voxels-3.png   sprite atlases at 1, 2 and 3 pixels per half voxel
    voxels.json                                 frame rectangles and ground anchors at scale 1

    python3 tools/voxel_sprites.py
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "apple" / "Resources"
SCALES = (1, 2, 3)
INK = (12, 16, 24)


def rgb(h: str):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def tone(c, k):
    return tuple(max(0, min(255, int(v * k))) for v in c)


class Model:
    def __init__(self):
        self.v: dict[tuple[int, int, int], tuple[int, int, int]] = {}

    def box(self, x0, y0, z0, x1, y1, z1, colour):
        c = rgb(colour)
        for x in range(x0, x1):
            for y in range(y0, y1):
                for z in range(z0, z1):
                    self.v[(x, y, z)] = c
        return self

    def footprint(self):
        xs = [k[0] for k in self.v]
        ys = [k[1] for k in self.v]
        return min(xs), max(xs) + 1, min(ys), max(ys) + 1


# ---- the fifteen industries --------------------------------------------------------------------
# Sizes: 0 small, 1 medium, 2 large. Colours echo each sector's badge colour.

def agriculture(s):
    w = 8 + 2 * s
    m = Model().box(0, 0, 0, w, w, 1, "#6FA83B")
    for i in range(0, w, 2):
        m.box(i, 0, 1, i + 1, w // 2, 2, "#C9A13A")
    m.box(1, w // 2 + 1, 1, 5, w - 1, 4, "#B23A2E").box(1, w // 2 + 1, 4, 5, w - 1, 5, "#7A2620")
    for k in range(s + 1):
        x = w - 3 - 3 * k
        m.box(x, w - 3, 1, x + 2, w - 1, 6 + s, "#C8CED4").box(x, w - 3, 6 + s, x + 2, w - 1, 7 + s, "#8A9299")
    return m


def extraction(s):
    w = 8 + 2 * s
    m = Model().box(0, 0, 0, w, w, 1, "#7A6A58")
    m.box(1, 1, 1, 4, 4, 3 + s, "#A1887F")
    m.box(w - 4, 1, 1, w - 3, 2, 8 + 2 * s, "#4A5560").box(w - 1, 1, 1, w, 2, 8 + 2 * s, "#4A5560")
    m.box(w - 4, 1, 8 + 2 * s, w, 2, 9 + 2 * s, "#E2B04A")
    m.box(1, w - 4, 1, 5 + s, w - 1, 2, "#5D4A3A").box(2, w - 3, 2, 4 + s, w - 2, 3, "#5D4A3A")
    return m


def energy(s):
    w = 9 + 2 * s
    m = Model().box(0, 0, 0, w, w, 1, "#5C636A")
    for k, (cx, cy) in enumerate(((1, 1), (5, 1), (1, 5), (5, 5))[: 2 + s]):
        m.box(cx, cy, 1, cx + 3, cy + 3, 4, "#D9DDE0").box(cx, cy, 4, cx + 3, cy + 3, 5, "#B5BCC2")
    m.box(w - 3, 1, 1, w - 1, 3, 10 + 2 * s, "#C0C6CC").box(w - 3, 1, 10 + 2 * s, w - 1, 3, 11 + 2 * s, "#FFC107")
    m.box(w - 2, w - 2, 1, w - 1, w - 1, 12 + 2 * s, "#8A9299").box(w - 2, w - 2, 12 + 2 * s, w - 1, w - 1, 13 + 2 * s, "#FF8A3A")
    return m


def chemical_industries(s):
    w = 9 + 2 * s
    m = Model().box(0, 0, 0, w, w, 1, "#4E5A62")
    for k in range(2 + s):
        x = 1 + 3 * k
        m.box(x, 1, 1, x + 2, 3, 6 + k, "#26C6DA").box(x, 1, 6 + k, x + 2, 3, 7 + k, "#1A8A99")
    m.box(1, 5, 1, w - 1, 6, 2, "#B5BCC2").box(1, 5, 2, 2, 6, 5, "#B5BCC2")
    m.box(1, w - 4, 1, 6, w - 1, 4, "#9AA6B2").box(1, w - 4, 4, 6, w - 1, 5, "#6E7A86")
    return m


def manufacturing(s):
    w = 9 + 2 * s
    m = Model().box(0, 0, 0, w, 7 + s, 3, "#9AA6B2")
    for i in range(0, w, 2):
        m.box(i, 0, 3, i + 1, 7 + s, 4, "#B9C3CC")
    m.box(w - 3, 1, 3, w - 1, 3, 9 + 2 * s, "#A0583C").box(w - 3, 1, 9 + 2 * s, w - 1, 3, 10 + 2 * s, "#5A3020")
    m.box(1, 7 + s, 0, 4, 9 + s, 1, "#4A5560").box(4, 6 + s, 0, 6, 7 + s, 2, "#E2B04A")
    return m


def construction(s):
    w = 8 + 2 * s
    m = Model().box(0, 0, 0, w, w, 1, "#8D7B68")
    m.box(1, 1, 1, 5, 5, 4 + 2 * s, "#FF8A65")
    for z in range(2, 4 + 2 * s, 2):
        m.box(1, 1, z, 5, 5, z + 1, "#C46A4E")
    m.box(w - 2, 1, 1, w - 1, 2, 10 + 2 * s, "#E2B04A").box(1, 1, 10 + 2 * s, w - 1, 2, 11 + 2 * s, "#E2B04A")
    m.box(2, w - 3, 1, 5, w - 1, 2, "#B0B6BC")
    return m


def logistics(s):
    w = 10 + 2 * s
    m = Model().box(0, 0, 0, w, 8, 1, "#3E6E8E")
    m.box(1, 1, 1, 7 + s, 6, 4, "#7E57C2").box(1, 1, 4, 7 + s, 6, 5, "#5A3E91")
    m.box(w - 2, 1, 1, w - 1, 2, 9 + s, "#E2B04A").box(w - 4, 1, 9 + s, w, 2, 10 + s, "#E2B04A")
    for i, colour in enumerate(("#E54840", "#3FBF8A", "#4DA3F0", "#E2B04A")[: 2 + s]):
        m.box(1 + 2 * i, 6, 1, 3 + 2 * i, 8, 2, colour)
    return m


def retail(s):
    w = 8 + 2 * s
    m = Model().box(0, 0, 0, w, w, 1, "#5A6470")
    m.box(1, 1, 1, w - 1, w - 2, 4 + s, "#EC407A").box(1, 1, 4 + s, w - 1, w - 2, 5 + s, "#B02E5C")
    m.box(1, w - 2, 3, w - 1, w - 1, 4, "#F8F2E8")
    m.box(3, w - 2, 1, 5, w - 1, 3, "#2A3440")
    return m


def healthcare(s):
    w = 8 + 2 * s
    m = Model().box(0, 0, 0, w, w, 1, "#5A6470")
    m.box(1, 1, 1, w - 1, w - 2, 6 + 2 * s, "#ECEFF1")
    for z in range(3, 6 + 2 * s, 2):
        m.box(1, 1, z, w - 1, w - 2, z + 1, "#B8C2CA")
    c = w // 2
    m.box(c - 1, w - 2, 3 + s, c + 1, w - 1, 6 + s, "#EF5350").box(c - 2, w - 2, 4 + s, c + 2, w - 1, 5 + s, "#EF5350")
    return m


def technology(s):
    w = 7 + 2 * s
    m = Model().box(0, 0, 0, w, w, 1, "#4A5560")
    m.box(1, 1, 1, w - 1, w - 1, 9 + 3 * s, "#42A5F5")
    for z in range(2, 9 + 3 * s, 2):
        m.box(1, 1, z, w - 1, w - 1, z + 1, "#2E6BA8")
    m.box(2, 2, 9 + 3 * s, w - 2, w - 2, 10 + 3 * s, "#C8CED4")
    return m


def media(s):
    w = 8 + 2 * s
    m = Model().box(0, 0, 0, w, w, 1, "#4A5560")
    m.box(1, 1, 1, w - 2, w - 2, 5 + s, "#AB47BC").box(1, 1, 5 + s, w - 2, w - 2, 6 + s, "#7B2E8A")
    m.box(w - 3, 2, 6 + s, w - 2, 3, 10 + s, "#C8CED4").box(w - 4, 1, 10 + s, w - 1, 4, 11 + s, "#E8EEF4")
    return m


def telecommunications(s):
    w = 6 + 2 * s
    m = Model().box(0, 0, 0, w, w, 1, "#4A5560")
    c = w // 2
    m.box(c - 1, c - 1, 1, c + 1, c + 1, 14 + 3 * s, "#29B6F6")
    for z in range(3, 14 + 3 * s, 3):
        m.box(c - 1, c - 1, z, c + 1, c + 1, z + 1, "#E54840")
    m.box(1, 1, 1, 3, 3, 3, "#9AA6B2")
    return m


def real_estate(s):
    w = 7 + 2 * s
    m = Model().box(0, 0, 0, w, w, 1, "#4A5560")
    m.box(1, 1, 1, w - 1, w - 1, 7 + 2 * s, "#66BB6A")
    for z in range(2, 7 + 2 * s, 2):
        for x in range(2, w - 1, 2):
            m.box(x, w - 1, z, x + 1, w, z + 1, "#F2E6A8")
    m.box(1, 1, 7 + 2 * s, w - 1, w - 1, 8 + 2 * s, "#3E7F43")
    return m


def defense(s):
    w = 9 + 2 * s
    m = Model().box(0, 0, 0, w, w, 1, "#55604A")
    m.box(1, 1, 1, w - 3, 6, 4, "#78909C").box(2, 1, 4, w - 4, 6, 5, "#5E727C")
    m.box(w - 3, w - 3, 1, w - 1, w - 1, 3 + s, "#C8CED4").box(w - 3, w - 3, 3 + s, w - 1, w - 1, 4 + s, "#E8EEF4")
    m.box(1, w - 2, 1, 4 + s, w - 1, 2, "#3E4A36")
    return m


def financial(s):
    w = 7 + 2 * s
    m = Model().box(0, 0, 0, w, w, 1, "#4A5560")
    m.box(1, 1, 1, w - 1, w - 1, 11 + 3 * s, "#E8E2D0")
    for z in range(3, 11 + 3 * s, 3):
        m.box(1, 1, z, w - 1, w - 1, z + 1, "#B8AE92")
    m.box(1, 1, 11 + 3 * s, w - 1, w - 1, 12 + 3 * s, "#FFD54F")
    return m


MODELS = {
    f.__name__: f for f in (
        agriculture, extraction, energy, chemical_industries, manufacturing, construction, logistics,
        retail, healthcare, technology, media, telecommunications, real_estate, defense, financial,
    )
}


# ---- rendering ---------------------------------------------------------------------------------

def render(m: Model, s: int):
    """The model at s pixels per half voxel, cel shaded, and its ground anchor in the sprite."""
    x0, x1, y0, y1 = m.footprint()
    top = max(k[2] for k in m.v) + 1
    w = (x1 - x0 + y1 - y0) * 2 * s + 4
    h = (x1 - x0 + y1 - y0) * s + top * 2 * s + 4
    img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    ids = Image.new("L", (w, h), 0)
    d = ImageDraw.Draw(img)
    di = ImageDraw.Draw(ids)
    ox = (y1 - y0) * 2 * s + 2
    oy = top * 2 * s + 2 + 2 * s
    material: dict = {}

    def at(x, y, z):
        return ox + ((x - x0) - (y - y0)) * 2 * s, oy + ((x - x0) + (y - y0)) * s - z * 2 * s

    for (x, y, z), colour in sorted(m.v.items(), key=lambda kv: (kv[0][0] + kv[0][1], kv[0][2])):
        px, py = at(x, y, z)
        T = [(px, py - 2 * s), (px + 2 * s, py - s), (px, py), (px - 2 * s, py - s)]
        L = [(px - 2 * s, py - s), (px, py), (px, py + 2 * s), (px - 2 * s, py + s)]
        R = [(px, py), (px + 2 * s, py - s), (px + 2 * s, py + s), (px, py + 2 * s)]
        mid = material.setdefault(colour, len(material) + 1)
        if (x, y, z + 1) not in m.v:
            d.polygon(T, fill=tone(colour, 1.0)); di.polygon(T, fill=mid)
        if (x, y + 1, z) not in m.v:
            d.polygon(L, fill=tone(colour, 0.72)); di.polygon(L, fill=mid)
        if (x + 1, y, z) not in m.v:
            d.polygon(R, fill=tone(colour, 0.52)); di.polygon(R, fill=mid)

    a = np.array(img)
    solid = a[..., 3] > 0
    idv = np.array(ids).astype(int)
    seam = np.zeros_like(solid)
    seam[:, :-1] |= (idv[:, :-1] != idv[:, 1:]) & solid[:, :-1] & solid[:, 1:]
    seam[:-1, :] |= (idv[:-1, :] != idv[1:, :]) & solid[:-1, :] & solid[1:, :]
    a[seam, :3] = (a[seam, :3] * 0.35).astype(np.uint8)
    # Shift without wrapping round the image edges.
    padded = np.pad(solid, 1)
    ring = (padded[1:-1, 2:] | padded[1:-1, :-2] | padded[2:, 1:-1] | padded[:-2, 1:-1])
    edge = ring & ~solid
    a[edge] = (*INK, 255)
    # Crop to the building itself so the top of the frame is the top of the roof.
    rows = np.where(a[..., 3].any(axis=1))[0]
    cols = np.where(a[..., 3].any(axis=0))[0]
    top, bottom, left, right = rows[0], rows[-1] + 1, cols[0], cols[-1] + 1
    a = a[top:bottom, left:right]
    # The ground anchor: the middle of the footprint at ground level.
    cx = (x0 + x1) / 2
    cy = (y0 + y1) / 2
    ax = ox + ((cx - x0) - (cy - y0)) * 2 * s
    ay = oy + ((cx - x0) + (cy - y0)) * s - s
    return Image.fromarray(a), (ax - left, ay - top)


def main() -> None:
    names = [(sector, size) for sector in MODELS for size in range(3)]
    sheets = {}
    for s in SCALES:
        sprites = [(n, render(MODELS[n[0]](n[1]), s)) for n in names]
        width = 16 * 2 * 26 * s
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
            sheets.setdefault(n, {})[s] = {"x": px, "y": py, "w": img.width, "h": img.height, "ax": anchor[0], "ay": anchor[1]}
        sheet.save(OUT / f"voxels-{s}.png", optimize=True)
    data = {
        "note": "Voxel division buildings, cel shaded, one frame per sector and size (0 small, 1 medium, 2 large) at each "
                "pixel scale. ax, ay is the ground anchor in the frame. Generated by tools/voxel_sprites.py.",
        "scales": list(SCALES),
        "frames": {f"{k[0]}/{k[1]}": {str(s): v for s, v in by.items()} for k, by in sheets.items()},
    }
    (OUT / "voxels.json").write_text(json.dumps(data, indent=1) + "\n")
    print(f"{len(names)} buildings at scales {SCALES} -> {OUT}")


if __name__ == "__main__":
    main()
