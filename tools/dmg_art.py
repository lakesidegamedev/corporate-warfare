#!/usr/bin/env python3
"""Draws the Mac disk image's background (needs Pillow).

The window that opens when the disk image is mounted shows the game's art, a gold arrow and the
words "Drag to Applications", drawn from the same pieces as the Windows installer pictures
(tools/installer_art.py): the dark cartographic palette, gold accents, the Silkscreen font and
the menu's globe render, painted on a small pixel grid and enlarged without smoothing. The window
is 660 by 400 points; the picture is written at that size and at twice it for Retina screens.
Pictures are generated at build time, never committed.

Usage: dmg_art.py OUT_DIR    writes dmg-background.png and dmg-background@2x.png
"""
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

sys.path.insert(0, str(Path(__file__).resolve().parent))
import installer_art as art  # noqa: E402

WINDOW = (660, 400)
PIXEL = 2
# Where the app and the Applications shortcut sit, in window points (the script puts the icons here).
APP_AT = (170, 215)
APPLICATIONS_AT = (490, 215)


def background(scale: int = 1) -> Image.Image:
    w, h = WINDOW
    bw, bh = w // PIXEL, h // PIXEL
    img = Image.new('RGB', (bw, bh), art.INK)
    d = ImageDraw.Draw(img)
    art.stars(d, bw, bh)
    # The globe rises behind the bottom edge, as on the menu.
    disc = art.globe_disc(int(bw * 0.55))
    img.paste(disc, (bw - int(bw * 0.3), bh - int(bw * 0.12)), disc)
    font = ImageFont.truetype(str(art.FONT), 8)
    title = 'CORPORATE WARFARE'
    d.text(((bw - d.textlength(title, font=font)) // 2, 12), title, font=font, fill=art.GOLD)
    d.rectangle((bw // 2 - 60, 26, bw // 2 + 60, 26), fill=art.GOLD)
    # A stepped pixel arrow between the two icons.
    cx, cy = bw // 2, APP_AT[1] // PIXEL
    for i in range(0, 30):
        d.point((cx - 15 + i, cy), fill=art.GOLD)
        d.point((cx - 15 + i, cy - 1), fill=art.GOLD)
    for i in range(0, 9):
        d.rectangle((cx + 15 - i, cy - 1 - i, cx + 15 - i, cy + i), fill=art.GOLD)
    caption = 'DRAG TO APPLICATIONS'
    d.text(((bw - d.textlength(caption, font=font)) // 2, bh - 24), caption, font=font, fill=art.TEXT)
    small = ImageFont.truetype(str(art.FONT), 8)
    d.text(((bw - d.textlength('RIGHT CLICK, OPEN, THE FIRST TIME', font=small)) // 2, bh - 13), 'RIGHT CLICK, OPEN, THE FIRST TIME', font=small, fill=art.SECONDARY)
    d.rectangle((0, 0, bw - 1, bh - 1), outline=art.STROKE)
    return img.resize((w * scale, h * scale), Image.NEAREST)


def main(out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)
    background(1).save(out / 'dmg-background.png')
    background(2).save(out / 'dmg-background@2x.png')


if __name__ == '__main__':
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(Path(sys.argv[1]))
