#!/usr/bin/env python3
"""Build the GPU map's plant models: clean, low sheds, tanks, stacks and towers per sector.

The classic map keeps apple/Resources/voxel-models.json untouched. The GPU renderer loads this
file on top of it and replaces the plant keys (sector/size, site/size, city/size).

Every model stands on a 24 x 24 voxel footprint (x east, y north, z up; the front faces south,
low y). The game sizes the footprint to a real 0.3 to 2 km, so a voxel is 12 to 85 m and no
model is taller than its footprint (a lone mast aside). Boxes never overlap, every box rests on
the ground pad or on another box, and the colours are the 48 palette colours. A box flagged
`trim` is the roof or trim the player's firm colour replaces.

    python3 tools/plant_models.py     writes crates/cw-render/assets/plant-models.json
"""

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "crates" / "cw-render" / "assets" / "plant-models.json"

RAMPS = {
    "ink": ["0B1622", "13212F", "1A2B3C", "22364A"],
    "water": ["0F2B4A", "163555", "1E4772", "2D6396"],
    "forest": ["1B3A22", "28502B", "3A6B35", "548A45"],
    "field": ["4B5A26", "667A33", "8A9A45", "B0B65A"],
    "soil": ["4A3A2A", "6B5440", "8C7556", "AF9A74"],
    "rock": ["3B3F46", "565B63", "7A7F86", "A3A8AE"],
    "snow": ["6F86A8", "9DB2CC", "C9D8EA", "F2F7FC"],
    "urban": ["2B3B4A", "475364", "6A7889", "93A2B4"],
    "gold": ["5E4718", "8C6A25", "BC9238", "E2B04A"],
    "cloud": ["6B7C93", "93A4B8", "C9D4E2", "E8EEF5"],
    "brick": ["5C201B", "7F2B23", "B23A2E", "D9594A"],
    "signal": ["3FBF8A", "F2B748", "E5605E", "C9804A"],
}


def c(name, step):
    return "#" + RAMPS[name][step]


PAD = c("rock", 2)
PAD_Z = 0.5
F = 12  # half footprint


class M:
    def __init__(self):
        self.boxes = []

    def b(self, x0, y0, z0, x1, y1, z1, col, trim=False):
        assert x1 > x0 and y1 > y0 and z1 > z0, (x0, y0, z0, x1, y1, z1)
        self.boxes.append((x0, y0, z0, x1, y1, z1, col, trim))

    def z(self, z):
        return PAD_Z + z

    def shed(self, x0, y0, x1, y1, h, wall, roof, trim=True, ribs=0):
        """Low hall: walls then a roof cap; optional sawtooth ribs along x."""
        top = self.z(h)
        self.b(x0, y0, PAD_Z, x1, y1, top - 0.5, wall)
        self.b(x0, y0, top - 0.5, x1, y1, top, roof, trim)
        if ribs:
            step = (x1 - x0) / ribs
            for i in range(ribs):
                a = x0 + step * i + step * 0.2
                self.b(a, y0 + 0.5, top, a + step * 0.35, y1 - 0.5, top + 0.6, c("water", 3))

    def tower(self, x0, y0, x1, y1, h, wall, roof, trim=True, z0=None):
        z0 = PAD_Z if z0 is None else z0
        top = z0 + h
        self.b(x0, y0, z0, x1, y1, top - 0.6, wall)
        self.b(x0, y0, top - 0.6, x1, y1, top, roof, trim)
        return top

    def tank(self, cx, cy, w, h, wall, lid, z0=None):
        """Octagonal tank: a centre column and two side strips, with a smaller lid."""
        z0 = PAD_Z if z0 is None else z0
        a = w / 2
        self.b(cx - a + 1, cy - a, z0, cx + a - 1, cy + a, z0 + h, wall)
        self.b(cx - a, cy - a + 1, z0, cx - a + 1, cy + a - 1, z0 + h, wall)
        self.b(cx + a - 1, cy - a + 1, z0, cx + a, cy + a - 1, z0 + h, wall)
        self.b(cx - a + 1.2, cy - a + 1.2, z0 + h, cx + a - 1.2, cy + a - 1.2, z0 + h + 0.4, lid)
        return z0 + h + 0.4

    def stack(self, cx, cy, w, h, band=True, cols=None, z0=None):
        z0 = PAD_Z if z0 is None else z0
        cols = cols or (c("brick", 2), c("cloud", 3))
        a = w / 2
        seg = h / 4.0
        for i in range(4):
            self.b(cx - a, cy - a, z0 + seg * i, cx + a, cy + a, z0 + seg * (i + 1), cols[i % 2])
        self.b(cx - a - 0.2, cy - a - 0.2, z0 + h, cx + a + 0.2, cy + a + 0.2, z0 + h + 0.6, c("rock", 0))
        return z0 + h + 0.6

    def column(self, cx, cy, w, h, col, cap, z0=None):
        z0 = PAD_Z if z0 is None else z0
        a = w / 2
        self.b(cx - a, cy - a, z0, cx + a, cy + a, z0 + h - 0.5, col)
        self.b(cx - a, cy - a, z0 + h - 0.5, cx + a, cy + a, z0 + h, cap)

    def rack(self, x0, x1, y, h=2.0, col=None):
        col = col or c("signal", 3)
        for x in (x0, (x0 + x1) / 2 - 0.25, x1 - 0.5):
            self.b(x, y, PAD_Z, x + 0.5, y + 0.5, PAD_Z + h, c("rock", 1))
        self.b(x0, y - 0.25, PAD_Z + h, x1, y + 0.75, PAD_Z + h + 0.5, col)

    def crane(self, x, y, h, jib=9, col=None, rot=0):
        col = col or c("gold", 3)
        self.b(x, y, PAD_Z, x + 1, y + 1, PAD_Z + h - 1, c("gold", 2))
        if rot == 0:
            self.b(x - jib * 0.45, y, PAD_Z + h - 1, x + 1 + jib, y + 1, PAD_Z + h, col)
            self.b(x - jib * 0.45, y, PAD_Z + h, x - jib * 0.45 + 1.6, y + 1, PAD_Z + h + 0.8, c("rock", 1))
        else:
            self.b(x, y - jib * 0.45, PAD_Z + h - 1, x + 1, y + 1 + jib, PAD_Z + h, col)
            self.b(x, y - jib * 0.45, PAD_Z + h, x + 1, y - jib * 0.45 + 1.6, PAD_Z + h + 0.8, c("rock", 1))

    def pad(self, x0=-F, y0=-F, x1=F, y1=F, col=PAD, z=PAD_Z, z0=0.0):
        self.b(x0, y0, z0, x1, y1, z, col)

    def done(self):
        return [list(map(lambda v: round(v, 3) if isinstance(v, float) else v, b)) for b in self.boxes]


def start():
    m = M()
    m.pad()
    return m


# ---------------------------------------------------------------- sectors

def energy(s):
    m = start()
    m.shed(-11, -9, 1, -2, 3.5, c("cloud", 2), c("urban", 2), ribs=3)
    m.tower(1, -9, 8, -1, 7, c("rock", 3), c("signal", 3))
    m.stack(-6, 2, 2, 11 + 2 * s)
    m.rack(-10, 0, -10.5 + 0.0, 1.5)
    # switchyard: framed transformers
    for i in range(2 + s):
        x = 2.5 + i * 2.2
        m.b(x, -11.2, PAD_Z, x + 1.6, -9.6, PAD_Z + 1.2, c("rock", 1))
    if s >= 1:
        m.tank(6, 6, 6, 5.5, c("cloud", 3), c("rock", 2))
    if s >= 2:
        m.tank(-1.5, 8, 6, 5.5, c("cloud", 3), c("rock", 2))
        m.shed(-11, 4, -8, 11, 3.0, c("cloud", 2), c("urban", 2))
    return m


def chemical_industries(s):
    m = start()
    m.shed(-11, -10, -4, -5, 3.0, c("cloud", 2), c("urban", 2))
    for i in range(2 + s):
        m.tank(-8.5 + i * 5.6, 5, 5, 3.5 + (i % 2) * 1.5, c("snow", 3), c("rock", 2))
    m.rack(-11, 11, 0.5, 2.0)
    m.column(7, -6, 2.4, 10 + s, c("rock", 3), c("rock", 1))
    m.column(10, -6, 2.0, 8, c("rock", 3), c("rock", 1))
    m.column(10, -9, 2.0, 6, c("rock", 3), c("rock", 1))
    m.tank(3, -8.5, 3.6, 2.5, c("signal", 3), c("rock", 2))
    # flare stack with a bright tip
    m.b(8.5, 9, PAD_Z, 9.5, 10, PAD_Z + 12, c("rock", 2))
    m.b(8.3, 8.8, PAD_Z + 12, 9.7, 10.2, PAD_Z + 13, c("signal", 1))
    return m


def manufacturing(s):
    m = start()
    bays = 2 + s
    w = 22.0 / bays
    for i in range(bays):
        x0 = -11 + i * w
        m.shed(x0 + 0.3, -6, x0 + w - 0.3, 6, 3.0, c("cloud", 2), c("urban", 2), ribs=3)
    m.shed(-11, -11, -1, -7.5, 2.0, c("signal", 3), c("urban", 3))
    m.stack(9, 9, 2, 10)
    for i in range(2):
        m.b(1 + i * 3, 7.5, PAD_Z, 3.5 + i * 3, 11, PAD_Z + 1.4, c("urban", 3))
    return m


def extraction(s):
    m = start()
    # spoil heap in steps
    for i, half in enumerate([4.8, 3.4, 2.0]):
        m.b(-6.5 - half, 6.5 - half, PAD_Z + 1.5 * i, -6.5 + half, 6.5 + half, PAD_Z + 1.5 * (i + 1), c("soil", 1 + i))
    # headframe: two legs and a top beam
    m.b(3, -4, PAD_Z, 4, -2.5, PAD_Z + 11, c("rock", 1))
    m.b(7, -4, PAD_Z, 8, -2.5, PAD_Z + 11, c("rock", 1))
    m.b(3, -4.5, PAD_Z + 11, 8, -2, PAD_Z + 12, c("signal", 3), True)
    m.shed(-10, -10, -2, -4, 3.0, c("cloud", 2), c("urban", 2))
    m.rack(-2, 3, -3.5, 1.2, c("rock", 2))
    if s >= 1:
        m.tank(7, 4.5, 5, 4.0, c("rock", 3), c("rock", 1))
    if s >= 2:
        m.tank(7.5, 9.5, 3.5, 3.0, c("rock", 3), c("rock", 1))
        m.shed(1, -11, 8, -6.5, 2.2, c("cloud", 2), c("urban", 2))
    return m


def agriculture(s):
    m = start()
    # fields beside the farmstead
    m.b(-11, 1, PAD_Z, -2, 11, PAD_Z + 0.2, c("field", 2))
    m.b(-1.5, 1, PAD_Z, 5.5, 5.5, PAD_Z + 0.2, c("field", 1))
    # barn: low walls, roof ridge stepped
    m.b(-10, -10, PAD_Z, -2, -4, PAD_Z + 2.0, c("brick", 2))
    m.b(-10, -9, PAD_Z + 2.0, -2, -5, PAD_Z + 3.0, c("brick", 1), True)
    n = 2 + s
    for i in range(n):
        m.tank(1.5 + i * 2.8, -7, 2.6, 6 + (i % 2), c("cloud", 3), c("rock", 1))
    m.shed(2, -2, 10, 0.5, 1.8, c("cloud", 2), c("signal", 3)) if s >= 1 else None
    if s >= 2:
        m.b(6.5, 3.5, PAD_Z, 11, 10.5, PAD_Z + 2.0, c("soil", 3))
    return m


def logistics(s):
    m = start()
    m.shed(-11, -3, 11, 8, 3.0, c("cloud", 2), c("urban", 2))
    for i in range(6):
        m.b(-9.5 + i * 3.6, -3.4, PAD_Z, -7.5 + i * 3.6, -3.0, PAD_Z + 1.6, c("urban", 1))
    cols = [c("signal", 2), c("water", 3), c("signal", 3), c("signal", 1)]
    for i in range(3 + 2 * s):
        x = -11 + (i % 6) * 3.6
        y = -10 + (i // 6) * 2.4
        m.b(x, y, PAD_Z, x + 3.0, y + 1.8, PAD_Z + 1.2, cols[i % 4])
    m.crane(9, -5, 8, 8, rot=1) if s >= 1 else None
    if s >= 2:
        m.b(-11, 8.6, PAD_Z, 11, 11, PAD_Z + 1.0, c("urban", 3))
    return m


def construction(s):
    m = start()
    m.crane(-6, 4, 14 + 2 * s, 9)
    m.shed(-11, -10, -3, -5, 2.0, c("gold", 2), c("urban", 2))
    for i in range(2 + s):
        m.tank(-0.5 + i * 3.4, -7.5, 3.0, 5.5, c("rock", 3), c("rock", 1))
    for i in range(3):
        m.b(1.5 + i * 3.2, 4, PAD_Z, 4.2 + i * 3.2, 8, PAD_Z + 1.0 + i * 0.5, c("soil", 2))
    m.b(-2, 6.5, PAD_Z, 0.5, 10, PAD_Z + 2.0, c("signal", 3)) if s >= 2 else None
    return m


def site(s):
    """A construction site: a foundation, a rising frame and a crane."""
    m = start()
    n = 1 + s
    m.b(-8, -8, PAD_Z, 6, 6, PAD_Z + 0.8, c("rock", 3))
    # columns and the floors that rest on them, one storey per size step
    for lvl in range(n + 1):
        z = PAD_Z + 0.8 + lvl * 3.0
        for x, y in [(-8, -8), (-8, 4.5), (4.5, -8), (4.5, 4.5), (-1.5, -1.5)]:
            m.b(x, y, z, x + 1.5, y + 1.5, z + 2.5, c("urban", 3))
        if lvl < n:
            m.b(-8, -8, z + 2.5, 6, 6, z + 3.0, c("rock", 3))
    m.crane(8, -1, 8 + 2 * n + 3, 9, rot=1)
    m.b(-11, 8, PAD_Z, -4, 11, PAD_Z + 1.2, c("signal", 3))
    m.b(8.5, -10.5, PAD_Z, 11, -7, PAD_Z + 1.0, c("soil", 2))
    return m


def defense(s):
    m = start()
    m.b(-11.5, -11.5, PAD_Z, 11.5, -9.5, PAD_Z + 0.2, c("rock", 1))  # apron strip
    m.shed(-10, -6, 0, 5, 3.0, c("forest", 1), c("forest", 0), trim=True)
    for i in range(2 + s):
        m.b(1 + (i % 3) * 3.6, -6 + (i // 3) * 3.5, PAD_Z, 4 + (i % 3) * 3.6, -3.5 + (i // 3) * 3.5, PAD_Z + 1.4, c("forest", 2))
    m.tank(-8, 8.5, 4.5, 2.0, c("rock", 3), c("rock", 1))
    m.column(-2, 9, 2, 6, c("rock", 3), c("signal", 2))
    if s >= 2:
        m.shed(2, 6, 11, 11.5, 1.8, c("forest", 1), c("forest", 0))
    return m


def media(s):
    m = start()
    m.shed(-11, -10, 1, -1, 3.5, c("snow", 3), c("signal", 3))
    m.shed(3, -10, 10, -4, 2.5, c("urban", 3), c("urban", 1), trim=False)
    m.b(-0.5, 2, PAD_Z, 1.5, 4, PAD_Z + 17 + 2 * s, c("rock", 3))
    m.b(-1.5, 1.5, PAD_Z + 17 + 2 * s, 2.5, 4.5, PAD_Z + 17.8 + 2 * s, c("signal", 2))
    m.tank(-7, 6, 4.5, 0.8, c("snow", 3), c("rock", 2))
    if s >= 1:
        m.tank(7, 6, 4.5, 0.8, c("snow", 3), c("rock", 2))
    if s >= 2:
        m.shed(4, 9, 11, 11.5, 1.6, c("urban", 3), c("urban", 1))
    return m


def healthcare(s):
    m = start()
    wall, roof = c("snow", 3), c("cloud", 1)
    m.shed(-10, -4, -3, 7, 4.5 + s * 0.8, wall, roof)
    m.shed(3, -4, 10, 7, 4.5 + s * 0.8, wall, roof)
    m.shed(-3, -2, 3, 1, 3.0, wall, roof)
    # red cross on the roof of the front wing
    top = PAD_Z + 4.5 + s * 0.8
    m.b(-7.5, 1.5, top, -5.5, 6.5, top + 0.3, c("signal", 2))
    m.shed(-10, -10, -2, -6, 2.0, c("cloud", 2), c("urban", 3))
    m.b(4, -11, PAD_Z, 10, -6, PAD_Z + 0.35, c("urban", 2))  # helipad
    m.b(5.8, -9.2, PAD_Z + 0.35, 8.2, -7.8, PAD_Z + 0.55, c("signal", 1))
    return m


def technology(s):
    m = start()
    m.tower(-10, -9, -3, -2, 4 + s * 1.5, c("water", 3), c("cloud", 3))
    m.tower(2, -9, 10, -1, 5 + s * 1.5, c("water", 3), c("cloud", 3))
    m.shed(-10, 2, 6, 10, 2.5, c("urban", 3), c("rock", 2), ribs=0)
    for i in range(4):
        m.b(-9 + i * 3.4, 3, PAD_Z + 2.5, -7.4 + i * 3.4, 5.6, PAD_Z + 3.4, c("rock", 3))
    if s >= 2:
        m.tower(7, 3, 11, 10, 8, c("water", 3), c("cloud", 3))
    return m


def financial(s):
    m = start()
    hmain = 17 + 2 * s
    top = m.tower(-3, -3, 3.5, 3.5, 9, c("urban", 3), c("urban", 2), trim=False)
    m.tower(-2, -2, 2.5, 2.5, hmain - 9, c("cloud", 2), c("cloud", 3), z0=top)
    m.tower(-10, -9, -4.5, -4, 8 + s, c("urban", 3), c("urban", 2))
    m.tower(5, -9, 10.5, -3.5, 10 + s, c("water", 3), c("cloud", 3))
    m.shed(-10, 5, 10, 10, 2.0, c("cloud", 2), c("urban", 2)) if s >= 1 else None
    m.b(-11, -11, PAD_Z, 11, -9.6, PAD_Z + 0.4, c("rock", 3))
    return m


def retail(s):
    m = start()
    m.shed(-11, 0, 5, 9, 3.0, c("signal", 3), c("cloud", 2))
    m.b(-11, 9.3, PAD_Z, 5, 10, PAD_Z + 1.0, c("urban", 3))
    # car park lines
    for i in range(7):
        m.b(-10 + i * 2.4, -10, PAD_Z, -9.4 + i * 2.4, -2, PAD_Z + 0.15, c("cloud", 3))
    m.b(8, -10, PAD_Z, 8.8, -9.2, PAD_Z + 6, c("rock", 2))
    m.b(7.0, -10.4, PAD_Z + 6, 9.8, -8.8, PAD_Z + 7.4, c("signal", 2))
    if s >= 1:
        m.shed(6, 1, 11, 6, 2.2, c("cloud", 2), c("urban", 3))
    if s >= 2:
        m.shed(6, 6.5, 11, 10, 2.2, c("cloud", 2), c("signal", 3))
    return m


def real_estate(s):
    m = start()
    n = 2 + s
    for i in range(n):
        x = -10 + i * (20.0 / n)
        m.tower(x, -4, x + 20.0 / n - 1.0, 4, 6 + (i % 2) * 2, c("brick", 2), c("urban", 2))
    m.b(-11, -11, PAD_Z, 11, -6, PAD_Z + 0.2, c("forest", 3))
    for i in range(3):
        m.b(-9 + i * 7, 6.5, PAD_Z, -5 + i * 7, 10, PAD_Z + 2.0, c("soil", 3))
        m.b(-9 + i * 7, 6.5, PAD_Z + 2.0, -5 + i * 7, 10, PAD_Z + 2.6, c("brick", 1), True)
    return m


def telecommunications(s):
    m = start()
    # lattice mast: a slender stack with bands
    z = PAD_Z
    for i in range(3 + s):
        zb = PAD_Z + 6 + i * 5
        m.b(-1, -1, z, 1, 1, zb, c("rock", 3))
        m.b(-2.2, -0.8, zb, 2.2, 0.8, zb + 0.8, c("signal", 2))
        z = zb + 0.8
    m.b(-1, -1, z, 1, 1, PAD_Z + 28, c("rock", 3))
    m.b(-1.4, -1.4, PAD_Z + 28, 1.4, 1.4, PAD_Z + 29, c("signal", 2))
    m.shed(-10, -10, -2, -5, 2.0, c("cloud", 2), c("urban", 2))
    m.shed(3, -10, 10, -5, 2.0, c("cloud", 2), c("urban", 2)) if s >= 1 else None
    m.tank(7, 6, 4, 0.8, c("snow", 3), c("rock", 2))
    if s >= 2:
        m.tank(-7, 6, 4, 0.8, c("snow", 3), c("rock", 2))
    return m


def city(s):
    """A block of downtown buildings: a few towers of different heights."""
    m = start()
    plan = [
        (-10, -9, -3, -2, 6, "cloud"),
        (-1, -9, 5, -3, 9, "urban"),
        (6, -9, 11, -2, 5, "snow"),
        (-10, 0, -4, 6, 4, "urban"),
        (-2, -1, 4, 6, 11, "cloud"),
        (6, 0, 11, 6, 7, "urban"),
    ]
    for x0, y0, x1, y1, h, col in plan[: 3 + 2 * s]:
        m.tower(x0, y0, x1, y1, h + 3 * s * 0 + s, c(col, 2), c("rock", 2) if col != "urban" else c("urban", 1))
    return m


SECTORS = {
    "energy": energy,
    "chemical_industries": chemical_industries,
    "manufacturing": manufacturing,
    "extraction": extraction,
    "agriculture": agriculture,
    "logistics": logistics,
    "construction": construction,
    "defense": defense,
    "media": media,
    "healthcare": healthcare,
    "technology": technology,
    "financial": financial,
    "retail": retail,
    "real_estate": real_estate,
    "telecommunications": telecommunications,
    "city": city,
    "site": site,
}


ERRS = []


def validate(key, boxes):
    def ov(a, b, i):
        return min(a[i + 3], b[i + 3]) - max(a[i], b[i])
    for i, a in enumerate(boxes):
        if not (a[0] >= -F - 0.01 and a[3] <= F + 0.01 and a[1] >= -F - 0.01 and a[4] <= F + 0.01):
            ERRS.append(f"{key}: outside {a[:6]}")
        for b in boxes[i + 1:]:
            if ov(a, b, 0) > 1e-6 and ov(a, b, 1) > 1e-6 and ov(a, b, 2) > 1e-6:
                ERRS.append(f"{key}: overlap {a[:6]} {b[:6]}")
    for a in boxes:
        if a[2] <= 1e-6:
            continue
        sup = [b for b in boxes if b is not a and abs(b[5] - a[2]) < 1e-6 and ov(a, b, 0) > 1e-6 and ov(a, b, 1) > 1e-6]
        if not sup:
            ERRS.append(f"{key}: floating {a[:6]}")
    top = max(b[5] for b in boxes)
    limit = 29.5 if key.startswith("telecommunications") else 24.0
    if top > limit:
        ERRS.append(f"{key}: {top} high")


def main():
    models = {}
    for name, fn in SECTORS.items():
        for s in range(3):
            key = f"{name}/{s}"
            boxes = fn(s).done()
            validate(key, boxes)
            models[key] = {"w": 2 * F, "d": 2 * F, "boxes": boxes}
    if ERRS:
        raise SystemExit("\n".join(sorted(set(ERRS))))
    OUT.write_text(json.dumps({"note": "GPU map plant models; see tools/plant_models.py", "models": models}, separators=(",", ":")))
    print(f"wrote {len(models)} models to {OUT}")


if __name__ == "__main__":
    main()
