#!/usr/bin/env python3
"""Checks the installer pictures: every size Inno Setup is given exists with the right dimensions,
and drawing is deterministic."""
import hashlib
import sys
import tempfile
import unittest
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import installer_art  # noqa: E402


class InstallerArt(unittest.TestCase):
    def test_sizes_and_palette(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            installer_art.main(out)
            for scale, big, small in installer_art.SIZES:
                self.assertEqual(Image.open(out / f'wizard-{scale}.bmp').size, big)
                self.assertEqual(Image.open(out / f'small-{scale}.bmp').size, small)
            # The side picture is mostly the dark ink colour with gold type.
            colours = Image.open(out / 'wizard-100.bmp').convert('RGB').getcolors(1 << 20)
            self.assertIn(installer_art.GOLD, {c for _, c in colours})
            self.assertIn(installer_art.INK, {c for _, c in colours})

    def test_deterministic(self):
        def digest():
            with tempfile.TemporaryDirectory() as tmp:
                installer_art.main(Path(tmp))
                return hashlib.sha256(b''.join(p.read_bytes() for p in sorted(Path(tmp).glob('*.bmp')))).hexdigest()
        self.assertEqual(digest(), digest())


if __name__ == '__main__':
    unittest.main()
