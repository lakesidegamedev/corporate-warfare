#!/usr/bin/env python3
"""Compose the Google Play feature graphic (1024 x 500, RGB, no alpha) from art already in the repo.

Composition only: the 512 store icon, the Great Lakes scenario picture and the Silkscreen font
(SIL Open Font License). No game render is needed, so it runs anywhere with Pillow.

    python3 tools/play_feature.py        writes store/play/feature-graphic.png
"""

from __future__ import annotations

import argparse
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
SPACE = (5, 9, 14)
GOLD = (222, 168, 72)
PAPER = (222, 232, 244)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out", type=Path, default=ROOT / "store" / "play" / "feature-graphic.png")
    args = parser.parse_args()
    width, height = 1024, 500
    canvas = Image.new("RGB", (width, height), SPACE)
    # The scenario picture fills the height on the right; the left part is a flat dark panel.
    scene = Image.open(ROOT / "apple/Resources/scenarios/scenario-great-lakes-2000.png").convert("RGB")
    scale = height / scene.height
    scene = scene.resize((round(scene.width * scale), height), Image.NEAREST)
    canvas.paste(scene, (width - scene.width, 0))
    draw = ImageDraw.Draw(canvas)
    panel = 372
    draw.rectangle((0, 0, panel, height), fill=SPACE)
    draw.rectangle((panel, 0, panel + 3, height), fill=GOLD)
    icon = Image.open(ROOT / "store/play/icon-512.png").convert("RGB").resize((144, 144), Image.NEAREST)
    canvas.paste(icon, (40, 70))
    bold = ImageFont.truetype(str(ROOT / "apple/Resources/Silkscreen-Bold.ttf"), 34)
    small = ImageFont.truetype(str(ROOT / "apple/Resources/Silkscreen-Regular.ttf"), 16)
    draw.text((40, 232), "CORPORATE", font=bold, fill=PAPER)
    draw.text((40, 276), "WARFARE", font=bold, fill=GOLD)
    draw.text((40, 344), "BUILD A BUSINESS EMPIRE", font=small, fill=PAPER)
    draw.text((40, 372), "OFFLINE. NO ADS.", font=small, fill=PAPER)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(args.out)
    print(args.out, canvas.size, canvas.mode)


if __name__ == "__main__":
    main()
