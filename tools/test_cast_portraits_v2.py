#!/usr/bin/env python3
"""Checks the 1.9 cast proposal: roster shape, 32 by 32 portraits with three expressions, palettes,
the 16 by 16 derivation, office portraits, and that the atlas matches its generator."""
import json
import re
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(Path(__file__).resolve().parent))
import cast_portraits_v2 as C  # noqa: E402

ATLAS = ROOT / 'apple/Resources/cast-portraits-v2.json'
ROSTER = json.loads((ROOT / 'content/cast-1.9.json').read_text())
OLD = json.loads((ROOT / 'content/cast.json').read_text())
HEX = re.compile(r'^#[0-9A-Fa-f]{6}$')
SCENARIOS = {'great-lakes-2000', 'texas-1981', 'silicon-valley-1995', 'ruhr-rhine-1991'}


def luminance(h):
    return C.lum(h)


class Roster(unittest.TestCase):
    def test_forty_people_ten_per_scenario(self):
        people = ROSTER['people']
        self.assertEqual(len(people), 40)
        for sc in SCENARIOS:
            self.assertEqual(sum(p['scenario'] == sc for p in people), 10, sc)

    def test_names_and_ids_unique_and_not_extras(self):
        people = ROSTER['people']
        self.assertEqual(len({p['id'] for p in people}), 40)
        self.assertEqual(len({p['name'] for p in people}), 40)
        extras = {e['name'] for e in OLD['extras']}
        new_names = {p['name'] for p in people if p['id'] not in {o['id'] for o in OLD['people']}}
        self.assertFalse(new_names & extras)

    def test_existing_twelve_unchanged(self):
        by_id = {p['id']: p for p in ROSTER['people']}
        for o in OLD['people']:
            for key, value in o.items():
                self.assertEqual(by_id[o['id']][key], value, (o['id'], key))

    def test_fields_and_role_coverage(self):
        for p in ROSTER['people']:
            for key in ('voice', 'background', 'temperament', 'roles', 'expressions', 'office_portrait'):
                self.assertTrue(p.get(key) is not None, (p['id'], key))
            self.assertEqual(p['expressions'], list(C.EXPRESSIONS))
            self.assertEqual(p['office_portrait'], 'ceo' in p['roles'])
            self.assertEqual(p['background'].count('. ') + 1, 2, p['id'])
            self.assertNotRegex(json.dumps(p), '[–—]')
        for sc in SCENARIOS:
            here = [p for p in ROSTER['people'] if p['scenario'] == sc]
            roles = {r for p in here for r in p['roles']}
            self.assertTrue({'ceo', 'chair', 'union_leader', 'journalist', 'analyst'} <= roles, sc)
            self.assertTrue(roles & {'governor', 'leader'}, sc)
            self.assertGreaterEqual(sum('ceo' in p['roles'] for p in here), 4, sc)
            self.assertEqual(sum(bool(set(p['roles']) & {'cfo', 'coo', 'logistics'}) for p in here), 2, sc)


class Portraits(unittest.TestCase):
    atlas = json.loads(ATLAS.read_text())

    def test_atlas_matches_generator(self):
        self.assertEqual(self.atlas, C.build())

    def test_every_person_three_expressions_of_right_size(self):
        self.assertEqual(set(self.atlas['portraits']), {p['id'] for p in ROSTER['people']})
        for pid, e in self.atlas['portraits'].items():
            self.assertEqual(set(e['expressions']), set(C.EXPRESSIONS), pid)
            for name in C.EXPRESSIONS:
                rows = e['expressions'][name]
                self.assertEqual((len(rows), {len(r) for r in rows}), (32, {32}), pid)
                small = e['small'][name]
                self.assertEqual((len(small), {len(r) for r in small}), (16, {16}), pid)
                for r in rows + small:
                    self.assertTrue(set(r) <= set(e['palette']) | {'.'}, (pid, name))

    def test_expressions_differ_only_in_eyes_brows_mouth(self):
        for pid, e in self.atlas['portraits'].items():
            base = e['expressions']['neutral']
            for name in ('pleased', 'displeased'):
                other = e['expressions'][name]
                self.assertNotEqual(base, other, (pid, name))
                for y in range(32):
                    if y < 7 or y > 21:
                        self.assertEqual(base[y], other[y], (pid, name, y))

    def test_palettes_valid_and_limited(self):
        for pid, e in self.atlas['portraits'].items():
            pal = e['palette']
            self.assertLessEqual(len(pal), 16, pid)
            for k, v in pal.items():
                self.assertRegex(v, HEX)
            self.assertEqual(pal['o'], C.OUTLINE)
            self.assertLess(luminance(pal['d']), luminance(pal['s']), pid)
            self.assertLess(luminance(pal['s']), luminance(pal['t']), pid)

    def test_bust_is_symmetric_in_the_eyes(self):
        for pid, e in self.atlas['portraits'].items():
            for name in C.EXPRESSIONS:
                for y in (12, 13):
                    row = e['expressions'][name][y][10:22]
                    row = ''.join(ch if ch in 'owlf' else '.' for ch in row)
                    self.assertEqual(row, row[::-1], (pid, name, y))

    def test_16_derivation_is_stable_and_from_32(self):
        for pid, e in self.atlas['portraits'].items():
            for name in C.EXPRESSIONS:
                again = C.downscale16(e['expressions'][name], e['palette'])
                self.assertEqual(again, e['small'][name], (pid, name))

    def test_office_portraits_for_chiefs_only(self):
        chiefs = {p['id'] for p in ROSTER['people'] if p['office_portrait']}
        self.assertEqual(set(self.atlas['office']), chiefs)
        self.assertEqual(len(chiefs), 16)
        for pid, o in self.atlas['office'].items():
            self.assertEqual((len(o['rows']), {len(r) for r in o['rows']}), (64, {64}), pid)
            for r in o['rows']:
                self.assertTrue(set(r) <= set(o['palette']), pid)

    def test_expressions_read_at_a_glance(self):
        # pleased shows a lit teeth row and displeased a brow shadow, in every portrait
        for pid, e in self.atlas['portraits'].items():
            pleased = ''.join(e['expressions']['pleased'][19])
            self.assertIn('www', pleased, pid)
            displeased = e['expressions']['displeased']
            self.assertTrue(any('d' in r[15:17] for r in displeased[9:12]) or 'm' in ''.join(displeased[19]), pid)

    def test_head_variety(self):
        shapes = {C.SPECS[p['id']]['jaw'] for p in ROSTER['people']}
        self.assertGreaterEqual(len(shapes), 6)
        noses = {C.SPECS[p['id']]['traits']['nose'] for p in ROSTER['people']}
        self.assertEqual(noses, {'button', 'long', 'broad'})
        self.assertEqual({C.SPECS[p['id']]['traits']['eyes'] for p in ROSTER['people']}, {'close', 'mid', 'wide'})
        self.assertGreaterEqual(len({C.SPECS[p['id']]['traits']['ears'] for p in ROSTER['people']}), 3)
        self.assertGreaterEqual(len({C.SPECS[p['id']]['traits']['neck'] for p in ROSTER['people']}), 3)

    def test_office_is_a_repaint_with_a_trade_object(self):
        for pid, o in self.atlas['office'].items():
            self.assertTrue(set(o['palette']) & set('uUq'), pid)
            hair = {ch for r in o['rows'] for ch in r if ch in 'hjk'}
            self.assertGreaterEqual(len(hair), 2 if C.SPECS[pid]['hair'] != 'horseshoe' else 1, pid)
            # no pixel pair is a plain 2 by 2 block of one letter across the whole face area
            face = [r[22:42] for r in o['rows'][20:42]]
            blocks = sum(1 for y in range(0, len(face) - 1, 2) for x in range(0, 19, 2)
                         if face[y][x] == face[y][x + 1] == face[y + 1][x] == face[y + 1][x + 1] and face[y][x] in 'iwm')
            self.assertEqual(blocks, 0, pid)

    def test_extras_sample_names_unique(self):
        sample = C.extras_sample()
        firsts = [e['name'].split()[0] for e, _ in sample]
        lasts = [e['name'].split()[-1] for e, _ in sample]
        self.assertEqual(len(set(firsts)), len(firsts))
        self.assertEqual(len(set(lasts)), len(lasts))
        roster_first = {p['name'].split()[0] for p in ROSTER['people']}
        roster_last = {p['name'].split()[-1] for p in ROSTER['people']}
        self.assertFalse(set(firsts) & roster_first)
        self.assertFalse(set(lasts) & roster_last)

    def test_extras_sample(self):
        sample = self.atlas['extras_sample']
        self.assertEqual(len(sample), 24)
        taken = {p['name'] for p in ROSTER['people']}
        for key, a in sample.items():
            self.assertEqual((len(a['rows']), {len(r) for r in a['rows']}), (32, {32}), key)
            self.assertNotIn(a['name'], taken)


if __name__ == '__main__':
    unittest.main()
