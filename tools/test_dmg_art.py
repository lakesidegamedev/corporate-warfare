#!/usr/bin/env python3
"""Checks the Mac disk image's background: the right sizes for a 660 by 400 window, the game's
palette, and drawing is deterministic."""
import hashlib
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
try:
    from PIL import Image
    import dmg_art  # noqa: E402
except ImportError:  # Pillow is only installed where the pictures are drawn
    dmg_art = None


@unittest.skipIf(dmg_art is None, 'Pillow is not installed')
class DmgArt(unittest.TestCase):
    def test_sizes_and_palette(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            dmg_art.main(out)
            self.assertEqual(Image.open(out / 'dmg-background.png').size, (660, 400))
            self.assertEqual(Image.open(out / 'dmg-background@2x.png').size, (1320, 800))
            colours = {c for _, c in Image.open(out / 'dmg-background.png').convert('RGB').getcolors(1 << 20)}
            self.assertIn(dmg_art.art.GOLD, colours)
            self.assertIn(dmg_art.art.INK, colours)

    def test_deterministic(self):
        def digest():
            with tempfile.TemporaryDirectory() as tmp:
                dmg_art.main(Path(tmp))
                return hashlib.sha256(b''.join(p.read_bytes() for p in sorted(Path(tmp).glob('*.png')))).hexdigest()
        self.assertEqual(digest(), digest())


if __name__ == '__main__':
    unittest.main()
