#!/usr/bin/env python3
"""Checks the cast portraits: one 16 by 16 portrait per person in content/cast.json, the JSON
matches its generator, and the spectacles use a light frame and a light lens."""
import json
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(Path(__file__).resolve().parent))
import cast_portraits  # noqa: E402

FILE = ROOT / 'apple/Resources/cast-portraits.json'
CAST = ROOT / 'content/cast.json'


def luminance(hex_colour):
    c = hex_colour.lstrip('#')
    r, g, b = (int(c[i:i + 2], 16) for i in (0, 2, 4))
    return 0.299 * r + 0.587 * g + 0.114 * b


class CastPortraits(unittest.TestCase):
    def test_every_person_has_a_portrait_and_no_one_else(self):
        cast = json.loads(CAST.read_text())
        people = {p['id'] for p in cast['people']} | {e['id'] for e in cast['extras']}
        self.assertEqual(people, set(json.loads(FILE.read_text())['portraits']))

    def test_grids_are_16_by_16_with_known_letters(self):
        for key, art in json.loads(FILE.read_text())['portraits'].items():
            self.assertEqual(len(art['rows']), 16, key)
            for row in art['rows']:
                self.assertEqual(len(row), 16, key)
                self.assertTrue(set(row) <= set(art['palette']) | {'.'}, key)

    def test_json_matches_generator(self):
        self.assertEqual(json.loads(FILE.read_text()), cast_portraits.build())

    def test_spectacles_read_as_clear_glass(self):
        worn = 0
        for key, art in json.loads(FILE.read_text())['portraits'].items():
            pal = art['palette']
            if 'f' in pal:
                worn += 1
                self.assertGreater(luminance(pal['f']), 120, f'{key} frame too dark')
                self.assertGreater(luminance(pal['l']), 200, f'{key} lens too dark')
        self.assertGreater(worn, 0)


if __name__ == '__main__':
    unittest.main()
