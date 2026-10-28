#!/usr/bin/env python3
"""Compose the Google Play feature graphic (1024 x 500, RGB, no alpha) from the game's own renders.

Inputs come from the desktop render tests (run with CW_MAP_RENDER_DIR set):
GraphicsRenderTest writes lander.png (the menu globe) and UiRenderTest writes ui-menu.png (the
menu with the pixel logo). The globe fills the right, the logo sits on the left over deep space.

    python3 tools/play_feature.py <render-dir>     writes store/play/feature-graphic.png
"""

from __future__ import annotations

import argparse
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SPACE = (5, 9, 14)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("renders", type=Path)
    parser.add_argument("--out", type=Path, default=ROOT / "store" / "play" / "feature-graphic.png")
    args = parser.parse_args()
    lander = Image.open(args.renders / "lander.png").convert("RGB")
    menu = Image.open(args.renders / "ui-menu.png").convert("RGB")
    canvas = Image.new("RGB", (1024, 500), SPACE)
    # The globe: the lander's right side, scaled to the graphic's height.
    globe = lander.crop((470, 0, lander.width, lander.height))
    scale = 560 / globe.height
    globe = globe.resize((int(globe.width * scale), 560), Image.LANCZOS)
    canvas.paste(globe, (1024 - globe.width + 40, -30))
    # The logo block from the menu, on the left.
    logo = menu.crop((90, 80, 370, 350))
    logo = logo.resize((int(logo.width * 1.25), int(logo.height * 1.25)), Image.NEAREST)
    canvas.paste(logo, (70, (500 - logo.height) // 2))
    args.out.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(args.out)
    print(args.out, canvas.size, canvas.mode)


if __name__ == "__main__":
    main()
