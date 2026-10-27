#!/usr/bin/env python3
"""Checks the shared pixel icon set: grids match the declared size, the JSON matches its
generator, and every icon the Compose and SwiftUI code references exists."""
import json
import re
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(Path(__file__).resolve().parent))
import pixel_icons  # noqa: E402

ICON_FILE = ROOT / 'apple/Resources/pixel-icons.json'
SOURCE_DIRS = [ROOT / 'compose', ROOT / 'apple/Sources']
ID = re.compile(r'^[a-z][a-z0-9_]*$')
# A line mentions icons when it names an icon API or one of the icon tables.
MARKER = re.compile(r'(?i)icon|MapButton\(|Nudge\(|\bspeeds\b|listOf\(0 to')
LITERAL = re.compile(r'"([^"\\]*)"')
# Page("map", "Map", "map", ...) and Tab("Map", "map") enum rows carry the icon as a later literal.
ENUM_ROW = re.compile(r'^\s+\w+\("[a-z]+", "[^"]+", "([a-z_]+)", "')
TAB_ROW = re.compile(r'^\s+\w+\("[^"]+", "([a-z_]+)"\),?;?\s*$')
# Words that share a line with the word icon but are not icon ids (file names, a process argument, the category prefix).
IGNORE = {'palette', 'rows', 'icons', 'size', 'note', 'json', 'start', 'cat_'}


def sources():
    for base in SOURCE_DIRS:
        for path in base.rglob('*'):
            if path.suffix in ('.kt', '.swift') and 'build' not in path.parts and 'Tests' not in path.parts \
                    and 'desktopTest' not in path.parts:
                yield path


def referenced():
    refs = {}
    for path in sources():
        if path.name in ('PixelIcons.kt', 'PixelIcons.swift'):
            continue
        for number, line in enumerate(path.read_text().splitlines(), 1):
            if line.lstrip().startswith(('//', '*', '/*')):
                continue
            found = []
            if MARKER.search(line):
                found += LITERAL.findall(line)
            for rx in (ENUM_ROW, TAB_ROW):
                m = rx.match(line)
                if m:
                    found.append(m.group(1))
            for lit in found:
                if ID.match(lit) and lit not in IGNORE:
                    refs.setdefault(lit, []).append(f'{path.relative_to(ROOT)}:{number}')
    return refs


class PixelIconTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = json.loads(ICON_FILE.read_text())
        cls.icons = cls.data['icons']

    def test_every_grid_is_the_declared_size(self):
        size = self.data['size']
        self.assertEqual(size, 16)
        for name, icon in self.icons.items():
            self.assertTrue(ID.match(name), name)
            self.assertEqual(len(icon['rows']), size, f'{name}: row count')
            for i, row in enumerate(icon['rows']):
                self.assertEqual(len(row), size, f'{name}: row {i} width')
            used = {ch for row in icon['rows'] for ch in row} - {'.'}
            self.assertTrue(used, f'{name}: empty icon')
            self.assertEqual(used, set(icon['palette']), f'{name}: palette and pixels disagree')
            for key, color in icon['palette'].items():
                self.assertEqual(len(key), 1)
                self.assertRegex(color, r'^#[0-9A-Fa-f]{6}$', f'{name}: colour {key}')

    def test_json_matches_generator(self):
        self.assertEqual(pixel_icons.build(), self.data, 'run tools/pixel_icons.py')

    def test_every_referenced_icon_exists(self):
        refs = referenced()
        self.assertGreater(len(refs), 40, 'reference scan found too little')
        missing = {k: v for k, v in refs.items() if k not in self.icons}
        self.assertEqual(missing, {}, 'icons referenced in code but missing from pixel-icons.json')

    def test_command_center_categories_have_icons(self):
        text = (ROOT / 'crates/cw-ffi/src/catalogue.rs').read_text()
        categories = set(re.findall(r'\ba\(\s*"[A-Za-z]+",\s*"([A-Za-z ]+)"', text))
        self.assertGreaterEqual(len(categories), 10)
        for category in categories:
            self.assertIn('cat_' + category.lower().replace(' ', '_'), self.icons, category)

    def test_core_set_is_present(self):
        need = '''map office markets stats inbox portfolio research command menu save load settings search quit
        play pause speed1 speed2 speed3 next_week zoom_in zoom_out globe scope alert warning buy sell
        arrow_up arrow_down trend_up trend_down close back'''.split()
        for name in need:
            self.assertIn(name, self.icons, name)

    def test_no_dashes_in_icon_file(self):
        text = ICON_FILE.read_text()
        self.assertNotIn('\u2014', text)
        self.assertNotIn('\u2013', text)


if __name__ == '__main__':
    unittest.main()
