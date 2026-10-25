#!/usr/bin/env python3
"""Generate the original Corporate Warfare app icon and its asset catalog.

The artwork is original: a navy field, a cream industrial skyline and a copper rising
portfolio bar. No text, no currency symbol, no copied mark. Run from the repository root:

    python3 tools/icon_gen.py

The script is deterministic and needs Pillow.
"""
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
ICON_DIR = ROOT / "apple" / "Assets.xcassets" / "AppIcon.appiconset"

NAVY = (20, 37, 58, 255)
NAVY_DEEP = (13, 23, 36, 255)
CREAM = (245, 242, 234, 255)
COPPER = (179, 111, 62, 255)
COPPER_LIGHT = (205, 148, 99, 255)


def draw(size: int) -> Image.Image:
    scale = size / 1024
    image = Image.new("RGBA", (size, size), NAVY)
    draw = ImageDraw.Draw(image)
    # Subtle vertical gradient field.
    for y in range(size):
        t = y / max(1, size - 1)
        colour = tuple(
            int(NAVY[i] + (NAVY_DEEP[i] - NAVY[i]) * t) for i in range(3)
        ) + (255,)
        draw.line([(0, y), (size, y)], fill=colour)

    def box(x0, y0, x1, y1, colour):
        draw.rounded_rectangle(
            [x0 * scale, y0 * scale, x1 * scale, y1 * scale],
            radius=max(2, int(18 * scale)),
            fill=colour,
        )

    # Skyline: five buildings of different heights, cream, with window slits.
    buildings = [
        (150, 600, 300, 850),
        (320, 470, 470, 850),
        (490, 540, 640, 850),
        (660, 380, 810, 850),
        (830, 660, 950, 850),
    ]
    for x0, y0, x1, y1 in buildings:
        box(x0, y0, x1, y1, CREAM)
    for x0, y0, x1, y1 in buildings:
        step = 46
        for wy in range(y0 + 40, y1 - 40, step):
            for wx in range(x0 + 26, x1 - 26, step):
                box(wx, wy, wx + 18, wy + 22, NAVY)

    # Copper rising portfolio bar, drawn over the skyline as the brand motif.
    points = [
        (170, 780),
        (330, 700),
        (470, 720),
        (620, 610),
        (760, 560),
        (900, 430),
    ]
    draw.line(
        [(x * scale, y * scale) for x, y in points],
        fill=COPPER_LIGHT,
        width=max(6, int(34 * scale)),
        joint="curve",
    )
    for x, y in points:
        r = max(6, int(22 * scale))
        draw.ellipse([x * scale - r, y * scale - r, x * scale + r, y * scale + r], fill=COPPER)

    # A thin copper baseline under the skyline.
    box(150, 880, 950, 902, COPPER)
    return image


def main() -> None:
    ICON_DIR.mkdir(parents=True, exist_ok=True)
    master = draw(1024)
    master.save(ICON_DIR / "AppIcon-1024.png", "PNG")
    # Keep a small set for older stores and previews; modern Xcode uses the 1024 universal slot.
    for size in (180, 120, 167, 152, 87, 80, 76, 60, 58, 40, 29, 20):
        draw(size).save(ICON_DIR / f"AppIcon-{size}.png", "PNG")
    print(f"wrote icons to {ICON_DIR}")


if __name__ == "__main__":
    main()
