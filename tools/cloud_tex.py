#!/usr/bin/env python3
"""Build the map's cloud textures from a real satellite cloud map.

Source: Solar System Scope, "8K Earth clouds" (8192 x 4096, CC BY 4.0, based on NASA Blue Marble
imagery), https://www.solarsystemscope.com/textures/download/8k_earth_clouds.jpg. The credit is
shown in Settings, About. The source is not kept in the repository; download it and pass its path.

Brightness becomes the alpha of a white image: below the black point is clear sky (the darkest
grey in the source is haze or surface, not cloud), above the white point solid cloud, with a smooth
step between. Outputs, into apple/Resources (the Compose build copies them):

    clouds.webp               the whole world at 4096 x 2048, for the globe
    clouds-<scenario>.webp    each scenario's surroundings at the source's full resolution, with
                              a higher black point so close up only distinct clouds remain

    python3 tools/cloud_tex.py ~/Downloads/8k_earth_clouds.jpg
"""

from __future__ import annotations

import argparse
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent

# West, south, east, north in degrees: each scenario's states with room for clouds to drift in.
# These must match CloudRegions in compose/.../Weather.kt and the Apple app.
REGIONS = {
    "great-lakes-2000": (-122, 18, -42, 66),
    "texas-1981": (-137, 8, -57, 56),
    "silicon-valley-1995": (-160, 13, -80, 61),
    "ruhr-rhine-1991": (-32, 28, 48, 76),
}


def to_alpha(grey: np.ndarray, black: float, white: float) -> Image.Image:
    a = np.clip((grey - black) / (white - black), 0, 1)
    a = a * a * (3 - 2 * a)
    rgba = np.zeros(grey.shape + (4,), np.uint8)
    rgba[..., :3] = 255
    rgba[..., 3] = (a * 255).astype(np.uint8)
    return Image.fromarray(rgba, "RGBA")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("source", type=Path)
    parser.add_argument("--out", type=Path, default=ROOT / "apple" / "Resources")
    args = parser.parse_args()
    Image.MAX_IMAGE_PIXELS = None
    source = Image.open(args.source).convert("L")
    w, h = source.size

    world = np.asarray(source.resize((4096, 2048), Image.LANCZOS)).astype(np.float32) / 255
    to_alpha(world, 0.16, 0.80).save(args.out / "clouds.webp", quality=82, method=6)

    full = np.asarray(source).astype(np.float32) / 255
    for name, (west, south, east, north) in REGIONS.items():
        x0 = int((west + 180) / 360 * w); x1 = int((east + 180) / 360 * w)
        y0 = int((90 - north) / 180 * h); y1 = int((90 - south) / 180 * h)
        to_alpha(full[y0:y1, x0:x1], 0.26, 0.82).save(args.out / f"clouds-{name}.webp", quality=84, method=6)

    for f in sorted(args.out.glob("clouds*.webp")):
        print(f.name, f.stat().st_size // 1024, "KiB")


if __name__ == "__main__":
    main()
