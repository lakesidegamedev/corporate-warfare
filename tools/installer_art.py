#!/usr/bin/env python3
"""Draws the Windows installer's side and corner pictures (needs Pillow).

The setup wizard shows a tall picture on its welcome and finish pages and a small one in the corner
of the others. Both are drawn here from the game's own art, so they match the menu: the dark
cartographic palette in docs/rules/VISUAL.md, gold accents, the Silkscreen font and the menu's
globe render. The pictures are painted on a small pixel grid and enlarged without smoothing, which
gives the 8-bit look at every screen scale. Bitmaps are generated at build time, never committed.

Usage: installer_art.py OUT_DIR    writes wizard-<scale>.bmp and small-<scale>.bmp for each scale
"""
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
FONT = ROOT / 'apple/Resources/Silkscreen-Bold.ttf'
GLOBE = ROOT / 'evidence/1.8-clouds/globe.jpg'
LOGO = ROOT / 'compose/shared/icons/corporate-warfare.png'

INK = (0x0B, 0x16, 0x22)
PANEL = (0x13, 0x21, 0x2F)
STROKE = (0x22, 0x36, 0x4A)
TEXT = (0xE8, 0xEE, 0xF5)
SECONDARY = (0x93, 0xA4, 0xB8)
GOLD = (0xE2, 0xB0, 0x4A)

# Inno Setup picks the size nearest the screen's scale: (scale percent, wizard w x h, small w x h).
SIZES = [
    (100, (164, 314), (55, 58)),
    (125, (192, 386), (64, 68)),
    (150, (246, 459), (83, 80)),
    (200, (328, 628), (110, 106)),
]
PIXEL = 2  # Each painted pixel becomes a PIXEL by PIXEL block.


def globe_disc(diameter: int) -> Image.Image:
    """The menu globe cropped to a round disc with a hard edge."""
    src = Image.open(GLOBE).convert('RGB')
    side = 740
    left, top = (src.width - side) // 2, (src.height - side) // 2
    crop = src.crop((left, top, left + side, top + side)).resize((diameter, diameter), Image.NEAREST)
    mask = Image.new('L', (diameter, diameter), 0)
    ImageDraw.Draw(mask).ellipse((0, 0, diameter - 1, diameter - 1), fill=255)
    out = Image.new('RGBA', (diameter, diameter))
    out.paste(crop, (0, 0), mask)
    return out


def logo_tile(size: int) -> Image.Image:
    return Image.open(LOGO).convert('RGB').resize((size, size), Image.NEAREST)


def stars(draw: ImageDraw.ImageDraw, w: int, h: int) -> None:
    state = 12345
    for _ in range(w * h // 90):
        state = (state * 1103515245 + 12345) & 0x7FFFFFFF
        x = state % w
        state = (state * 1103515245 + 12345) & 0x7FFFFFFF
        y = state % h
        draw.point((x, y), fill=STROKE if state % 3 else SECONDARY)


def wizard(w: int, h: int) -> Image.Image:
    bw, bh = w // PIXEL, h // PIXEL
    img = Image.new('RGB', (bw, bh), INK)
    d = ImageDraw.Draw(img)
    stars(d, bw, bh)
    disc = globe_disc(int(bw * 1.25))
    img.paste(disc, (bw - int(bw * 0.8), bh - int(bw * 0.62)), disc)
    big = ImageFont.truetype(str(FONT), 8)
    small = ImageFont.truetype(str(FONT), 8)
    img.paste(logo_tile(min(32, bw - 16)), (8, 10))
    y = 10 + min(32, bw - 16) + 8
    for word in ('CORPORATE', 'WARFARE'):
        d.text((8, y), word, font=big, fill=GOLD)
        y += 11
    d.rectangle((8, y + 1, bw - 9, y + 1), fill=GOLD)
    d.text((8, y + 6), 'ECONOMIC', font=small, fill=SECONDARY)
    d.text((8, y + 15), 'STRATEGY', font=small, fill=SECONDARY)
    d.rectangle((0, 0, bw - 1, bh - 1), outline=STROKE)
    return img.resize((w, h), Image.NEAREST)


def corner(w: int, h: int) -> Image.Image:
    bw, bh = w // PIXEL, h // PIXEL
    img = Image.new('RGB', (bw, bh), PANEL)
    d = ImageDraw.Draw(img)
    disc = globe_disc(min(bw, bh) - 4)
    img.paste(disc, ((bw - disc.width) // 2, (bh - disc.height) // 2), disc)
    d.rectangle((0, 0, bw - 1, bh - 1), outline=GOLD)
    return img.resize((w, h), Image.NEAREST)


def main(out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)
    for scale, big, small in SIZES:
        wizard(*big).save(out / f'wizard-{scale}.bmp')
        corner(*small).save(out / f'small-{scale}.bmp')


if __name__ == '__main__':
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(Path(sys.argv[1]))
