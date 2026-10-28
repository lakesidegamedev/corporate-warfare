#!/usr/bin/env python3
"""Bake the map's cloud textures.

The method follows SatelliteCloudGenerator (Czerkawski et al., University of Strathclyde,
https://github.com/strath-ai/SatelliteCloudGenerator, Apache License 2.0): fractal Perlin noise
where coarse octaves weigh most, squashed with tanh, several fields multiplied together so clouds
gather into local banks, then a clear-sky threshold that cuts the thin tail away. This is a numpy
reimplementation for still textures; no code from that project is copied.

Both textures tile: the world one round the globe (east to west), the local one in both
directions. Output is white with the cloud in the alpha channel; the app tints the same image
dark for shadows.

    python3 tools/cloud_gen.py            writes apple/Resources/clouds-{world,local,low,hurricane}.png
"""

from __future__ import annotations

import argparse
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent


def perlin(x: np.ndarray, y: np.ndarray, cells_x: int, cells_y: int, rng: np.random.Generator) -> np.ndarray:
    """One octave of gradient noise at points given in cells, on a grid that wraps so it tiles."""
    angles = rng.uniform(0, 2 * np.pi, (cells_y, cells_x))
    gx, gy = np.cos(angles), np.sin(angles)
    x0 = np.floor(x).astype(int)
    y0 = np.floor(y).astype(int)
    fx = x - x0
    fy = y - y0
    x1 = (x0 + 1) % cells_x
    y1 = (y0 + 1) % cells_y
    x0 %= cells_x
    y0 %= cells_y

    def corner(yi, xi, dx, dy):
        return gx[yi, xi] * dx + gy[yi, xi] * dy

    n00 = corner(y0, x0, fx, fy)
    n10 = corner(y0, x1, fx - 1, fy)
    n01 = corner(y1, x0, fx, fy - 1)
    n11 = corner(y1, x1, fx - 1, fy - 1)
    sx = fx * fx * (3 - 2 * fx)
    sy = fy * fy * (3 - 2 * fy)
    top = n00 + sx * (n10 - n00)
    bottom = n01 + sx * (n11 - n01)
    return top + sy * (bottom - top)


def fractal(u: np.ndarray, v: np.ndarray, width: int, height: int, rng, finest: int, coarsest: int, decay: float) -> np.ndarray:
    """Octaves from coarse to fine at texture points (u, v) in pixels, each weighted by its cell
    size to the power decay, squashed by tanh."""
    out = np.zeros(u.shape)
    cell = coarsest
    while cell >= finest:
        cx = max(1, width // cell)
        cy = max(1, height // cell)
        out += (cell ** decay) * perlin(u / width * cx, v / height * cy, cx, cy, rng)
        cell //= 2
    out /= max(out.max(), -out.min())
    return np.tanh(4 * out)


def warped(width, height, rng, finest, coarsest, decay, swirl):
    """A fractal field sampled through another, so cloud bands curl like weather systems."""
    v, u = np.mgrid[0:height, 0:width].astype(float)
    if swirl > 0:
        # The bending field is smooth: only the coarsest octaves, so bands curl instead of marbling.
        du = fractal(u, v, width, height, rng, max(coarsest // 4, 1), coarsest, 1.0)
        dv = fractal(u, v, width, height, rng, max(coarsest // 4, 1), coarsest, 1.0)
        u = (u + swirl * du) % width
        v = (v + swirl * dv) % height
    return fractal(u, v, width, height, rng, finest, coarsest, decay)


def cloud_mask(width, height, rng, finest, coarsest, decay, locality, threshold, coverage=None, swirl=0.0):
    """Clouds from 0 to 1: several fractal fields multiplied, then the clear-sky threshold."""
    mask = np.ones((height, width))
    for _ in range(locality):
        field = warped(width, height, rng, finest, coarsest, decay, swirl)
        field -= field.min()
        field /= field.max()
        mask *= field
    if coverage is not None:
        # Where the climate is cloudier the threshold drops, so more of the field survives.
        threshold = threshold - coverage
    mask = np.clip((mask - threshold) / np.maximum(1 - threshold, 1e-6), 0, 1)
    return mask / max(mask.max(), 1e-6)


def latitude_coverage(height: int) -> np.ndarray:
    """Cloudier at the equator and in the mid-latitude storm tracks, clearer in the subtropics."""
    lat = (0.5 - (np.arange(height) + 0.5) / height) * 180
    a = np.abs(lat)
    itcz = np.exp(-((a - 3) / 9) ** 2)
    storms = np.exp(-((a - 52) / 12) ** 2)
    subtropics = np.exp(-((a - 25) / 7) ** 2)
    return (0.14 * itcz + 0.16 * storms - 0.14 * subtropics)[:, None]


def storm(size: int, rng: np.random.Generator, kind: str) -> np.ndarray:
    """A northern-hemisphere storm seen from above, centred, spinning counterclockwise.

    "low": a mid-latitude depression, a comma of cloud wound round the centre with the cold
    front trailing away south-west. "hurricane": tight spiral bands round a dense core with a
    clear eye. The app mirrors them for the southern hemisphere.
    """
    v, u = np.mgrid[0:size, 0:size].astype(float)
    x = (u - size / 2) / (size / 2)
    y = (size / 2 - v) / (size / 2)
    r = np.hypot(x, y) + 1e-6
    theta = np.arctan2(y, x)
    detail = fractal(u, v, size, size, rng, 1, size // 4, 0.8)
    detail = (detail - detail.min()) / (detail.max() - detail.min())
    if kind == "hurricane":
        # Logarithmic spiral bands tightening inward, a dense ring, an open eye.
        arms = 0.5 + 0.5 * np.cos(3 * (theta + 2.6 * np.log(r)))
        body = np.exp(-(r / 0.62) ** 2) * (0.35 + 0.65 * arms ** 1.5)
        core = np.exp(-((r - 0.12) / 0.09) ** 2)
        mask = np.maximum(body, core)
        mask *= 1 - np.exp(-(r / 0.035) ** 2)
        mask *= 0.55 + 0.45 * detail
    else:
        # The comma head: a loose spiral wound round the low.
        arms = 0.5 + 0.5 * np.cos(2 * (theta + 1.8 * np.log(r)))
        head = np.exp(-(r / 0.5) ** 2) * (0.35 + 0.65 * arms)
        # The cold front: a band leaving the centre eastward, swinging south and then south-west.
        t = np.linspace(0, 1, 160)
        fx = 0.12 + 0.30 * np.sin(t * 1.8) - 0.62 * t ** 2
        fy = -0.02 - 0.92 * t
        d = np.full(x.shape, 9.0)
        for px, py, pt in zip(fx, fy, t):
            d = np.minimum(d, np.hypot(x - px, y - py) / (0.06 + 0.07 * pt))
        front = np.exp(-d ** 2) * 0.95
        mask = np.maximum(head, front * np.clip(1.15 - r, 0, 1))
        mask *= 0.6 + 0.4 * detail
    mask = np.clip((mask - 0.16) / 0.84, 0, 1)
    # Fade to nothing well inside the square so the sprite never shows its edge.
    mask *= np.clip((1 - r) / 0.15, 0, 1)
    return mask / max(mask.max(), 1e-6)


def to_rgba(mask: np.ndarray) -> Image.Image:
    alpha = mask ** 0.85
    # Thin cloud is a touch grey-blue, thick cloud white.
    shade = 0.80 + 0.20 * alpha
    rgb = np.stack([shade * 0.96, shade * 0.98, shade], axis=-1)
    rgba = np.concatenate([rgb, alpha[..., None]], axis=-1)
    return Image.fromarray((np.clip(rgba, 0, 1) * 255).astype(np.uint8), "RGBA")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out", type=Path, default=ROOT / "apple" / "Resources")
    parser.add_argument("--seed", type=int, default=1953)
    args = parser.parse_args()
    rng = np.random.default_rng(args.seed)

    # The whole planet, 2048 by 1024 equirectangular: curling weather systems from 256 px down to 2.
    world = cloud_mask(2048, 1024, rng, finest=2, coarsest=256, decay=0.9, locality=2, threshold=0.18,
                       coverage=latitude_coverage(1024), swirl=40.0)
    to_rgba(world).save(args.out / "clouds-world.png", optimize=True)

    # Close up, 1024 square, tiling both ways: broken fair-weather cloud with crisp tops.
    local = cloud_mask(1024, 1024, rng, finest=1, coarsest=128, decay=0.85, locality=2, threshold=0.20, swirl=4.0)
    to_rgba(local).save(args.out / "clouds-local.png", optimize=True)
    # Storm systems the app moves across the world.
    to_rgba(storm(512, rng, "low")).save(args.out / "clouds-low.png", optimize=True)
    to_rgba(storm(512, rng, "hurricane")).save(args.out / "clouds-hurricane.png", optimize=True)
    for name in ("clouds-world.png", "clouds-local.png", "clouds-low.png", "clouds-hurricane.png"):
        print(name, (args.out / name).stat().st_size // 1024, "KiB")


if __name__ == "__main__":
    main()
