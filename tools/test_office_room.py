#!/usr/bin/env python3
"""Checks the CEO Office room art: the JSON matches its generator, every grid is the declared size,
every object sits inside the room, and the room uses only hard-edged opaque pixels."""
import json
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(Path(__file__).resolve().parent))
import office_room  # noqa: E402

FILE = ROOT / 'apple/Resources/office-room.json'
EXPECTED = {'window', 'chart', 'trophies', 'door', 'paper', 'cabinet', 'safe', 'chief', 'nameplate', 'phone',
            'memo', 'exec0', 'exec1', 'exec2', 'board', 'mandate'}


class OfficeRoomTests(unittest.TestCase):
    def setUp(self):
        self.data = json.loads(FILE.read_text())

    def test_matches_generator(self):
        room, front, sprites, slots = office_room.build()
        self.assertEqual(self.data['room'], room.encode())
        self.assertEqual(self.data['front'], front.encode())
        self.assertEqual(self.data['trophy_slots'], [list(s) for s in slots])

    def test_grids_are_the_declared_size(self):
        w, h = self.data['size']
        for layer in ('room', 'front'):
            rows = self.data[layer]['rows']
            self.assertEqual(len(rows), h, layer)
            self.assertTrue(all(len(r) == w for r in rows), layer)

    def test_palette_is_opaque_hex(self):
        for layer in [self.data['room'], self.data['front'], *self.data['sprites'].values()]:
            for letter, colour in layer['palette'].items():
                self.assertRegex(colour, r'^#[0-9A-Fa-f]{6}$')
            used = {ch for row in layer['rows'] for ch in row} - {'.'}
            self.assertLessEqual(used, set(layer['palette']))

    def test_objects(self):
        w, h = self.data['size']
        ids = [o['id'] for o in self.data['objects']]
        self.assertEqual(set(ids), EXPECTED)
        self.assertEqual(len(ids), len(set(ids)))
        for o in self.data['objects']:
            x, y, rw, rh = o['rect']
            self.assertTrue(x >= 0 and y >= 0 and x + rw <= w and y + rh <= h, o['id'])
            self.assertTrue(o['label'].strip(), o['id'])
            self.assertNotIn('–', o['label'])
            self.assertNotIn('—', o['label'])


if __name__ == '__main__':
    unittest.main()
