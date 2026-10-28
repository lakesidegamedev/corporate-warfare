#!/usr/bin/env python3
"""Bake the map's Earth surface from NASA's Blue Marble Next Generation (July 2004).

Source (public domain, NASA Earth Observatory, Reto Stockli): record 74092,
https://eoimages.gsfc.nasa.gov/images/imagerecords/74000/74092/ ; the 5400 x 2700 world image
and the 21600 x 21600 tiles A1, B1 and C1 (the northern hemisphere from 180 W to 90 E). The
sources are not kept in the repository; download them into one folder and pass its path.

Outputs, into apple/Resources (the Compose build copies them):

    earth.jpg                 the world at 4096 x 2048, for the globe and the menu
    earth-<scenario>.jpg      each scenario's surroundings at 40 pixels a degree, for close up
    earth-land.png            a 2048 x 1024 land mask from the game's own country and lake
                              outlines (white land, clear water) for seasonal snow on the globe
    earth-land-<scenario>.png the same for each scenario's surroundings at 20 pixels a degree

The seasons are made in the app from this summer image: snow lines that move with the month,
autumn colour and winter brown in the mid-latitudes.

    python3 tools/earth_tex.py ~/Downloads/blue-marble
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent

# Must match CloudRegions in compose/.../Weather.kt: west, south, east, north in degrees.
REGIONS = {
    "great-lakes-2000": (-122, 18, -42, 66),
    "texas-1981": (-137, 8, -57, 56),
    "silicon-valley-1995": (-160, 13, -80, 61),
    "ruhr-rhine-1991": (-32, 28, 48, 76),
}
TILES = {"A1": (-180, 0), "B1": (-90, 0), "C1": (0, 0)}  # tile name: west, south (each 90 degrees square)
PER_DEGREE = 40


def tile_crop(folder: Path, west, south, east, north) -> Image.Image:
    """The area from the 240-pixels-a-degree tiles, scaled to PER_DEGREE."""
    out = Image.new("RGB", (int((east - west) * PER_DEGREE), int((north - south) * PER_DEGREE)))
    for name, (tw, ts) in TILES.items():
        x0, x1 = max(west, tw), min(east, tw + 90)
        y0, y1 = max(south, ts), min(north, ts + 90)
        if x0 >= x1 or y0 >= y1:
            continue
        tile = Image.open(folder / f"world.200407.3x21600x21600.{name}.png")
        px = lambda lon: int((lon - tw) * 240)
        py = lambda lat: int((ts + 90 - lat) * 240)
        part = tile.crop((px(x0), py(y1), px(x1), py(y0)))
        part = part.resize((int((x1 - x0) * PER_DEGREE), int((y1 - y0) * PER_DEGREE)), Image.LANCZOS)
        out.paste(part, (int((x0 - west) * PER_DEGREE), int((north - y1) * PER_DEGREE)))
        tile.close()
    return out


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("source", type=Path)
    parser.add_argument("--out", type=Path, default=ROOT / "apple" / "Resources")
    args = parser.parse_args()
    Image.MAX_IMAGE_PIXELS = None

    world = Image.open(args.source / "world.200407.3x5400x2700.png").convert("RGB")
    world.resize((4096, 2048), Image.LANCZOS).save(args.out / "earth.jpg", quality=86, optimize=True)

    # Land masks from the game's own map, every country less the lakes: the world for the globe,
    # and each scenario's surroundings at 20 pixels a degree so coasts stay crisp close up.
    data = json.loads((ROOT / "apple" / "Resources" / "world-map.json").read_text())

    def land_mask(west, south, east, north, width, height):
        mask = Image.new("L", (width, height), 0)
        draw = ImageDraw.Draw(mask)

        def ring(points):
            return [((points[i] / 10_000 - west) / (east - west) * width, (north - points[i + 1] / 10_000) / (north - south) * height)
                    for i in range(0, len(points) - 1, 2)]

        for country in data["countries"]:
            for r in country["rings"]:
                if len(r) >= 6:
                    draw.polygon(ring(r), fill=255)
        for lake in data["lakes"]:
            for r in lake["rings"]:
                if len(r) >= 6:
                    draw.polygon(ring(r), fill=0)
        rgba = np.zeros((height, width, 4), np.uint8)
        rgba[..., :3] = 255
        rgba[..., 3] = np.asarray(mask)
        return Image.fromarray(rgba, "RGBA")

    land_mask(-180, -90, 180, 90, 2048, 1024).save(args.out / "earth-land.png", optimize=True)
    for name, (west, south, east, north) in REGIONS.items():
        land_mask(west, south, east, north, (east - west) * 20, (north - south) * 20).save(args.out / f"earth-land-{name}.png", optimize=True)

    for name, (west, south, east, north) in REGIONS.items():
        tile_crop(args.source, west, south, east, north).save(args.out / f"earth-{name}.jpg", quality=84, optimize=True)

    for f in sorted(args.out.glob("earth*")):
        print(f.name, f.stat().st_size // 1024, "KiB")


if __name__ == "__main__":
    main()
