#!/usr/bin/env python3
"""Flatten Play screenshots to RGB in place and check Google's rules (each side 320 to 3840 px, at most 2:1).

    python3 tools/play_screens_check.py <folder with png files>
"""

from __future__ import annotations

import sys
from pathlib import Path

from PIL import Image


def main() -> int:
    folder = Path(sys.argv[1])
    files = sorted(folder.glob("*.png"))
    if not files:
        print(f"no pictures in {folder}")
        return 1
    bad = 0
    for path in files:
        image = Image.open(path)
        w, h = image.size
        flat = Image.new("RGB", image.size, (0, 0, 0))
        flat.paste(image.convert("RGBA"), mask=image.convert("RGBA").split()[3])
        flat.save(path)
        ok = 320 <= min(w, h) and max(w, h) <= 3840 and max(w, h) <= 2 * min(w, h)
        print(f"{path.name}: {w} x {h} RGB {'ok' if ok else 'REJECTED BY GOOGLE'}")
        bad += 0 if ok else 1
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
