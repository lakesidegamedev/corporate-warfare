#!/usr/bin/env python3
"""Checks the generated logos: content/logos.json matches its generator, the banned-shape check
passes, composing is deterministic and the pinned results agree with the Rust composer."""
import json
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(Path(__file__).resolve().parent))
import logo_gen  # noqa: E402

PINS = {
    'Maumee Valley Grain': 'c542aac79e72fb2f',
    'Lakeshore Steel': '72508df24c03d0b3',
    'Prairie Wheat Co': '4e5bb91c79184181',
    'Zeta Works': '422e5baba54f1269',
}


class Logos(unittest.TestCase):
    def test_json_matches_generator(self):
        self.assertEqual(json.loads(logo_gen.OUT.read_text()), logo_gen.build())

    def test_library_has_no_banned_shapes(self):
        self.assertEqual(logo_gen.check_library(), [])

    def test_compose_is_deterministic_and_square(self):
        a = logo_gen.compose('Ember Oil')
        self.assertEqual(a, logo_gen.compose('  ember oil '))
        self.assertEqual(len(a[1]), 16)
        self.assertTrue(all(len(r) == 16 for r in a[1]))

    def test_pins_agree_with_rust(self):
        for name, pin in PINS.items():
            self.assertEqual(logo_gen.rows_digest(logo_gen.compose(name)[1]), pin, name)


if __name__ == '__main__':
    unittest.main()
