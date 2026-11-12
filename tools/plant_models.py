#!/usr/bin/env python3
"""Build the GPU map's plant models: clean, low sheds, tanks, stacks and towers per sector.

The classic map keeps apple/Resources/voxel-models.json untouched. The GPU renderer loads this
file on top of it and replaces the plant keys (sector/size, site/size, city/size).

Every model stands on a 24 x 24 voxel footprint (x east, y north, z up; the front faces south,
low y). The game sizes the footprint to a real 0.3 to 2 km, so a voxel is 12 to 85 m and no
model is taller than its footprint (a lone mast aside). Boxes never overlap, every box rests on
the ground pad or on another box, and the colours are the 48 palette colours. A box flagged
`trim` is the roof or trim the owner's firm colour replaces (gold for the player). A box flagged
`win` is a wall that gets window rows (dark by day, lit at night); every other box is blank.

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

TAR = c("ink", 3)
METAL = c("cloud", 1)
TILE = c("brick", 1)
CONCRETE = c("cloud", 2)
GLASS = c("water", 3)
DOOR = c("ink", 1)


class M:
    def __init__(self):
        self.boxes = []

    def b(self, x0, y0, z0, x1, y1, z1, col, trim=False, win=False, shape=None):
        """One part. `shape` fills the same box with a solid the renderer knows: cyl, cone,
        taper, flare, dome (upright and round), cylx, cyly (lying), gablex, gabley (pitched roof,
        ridge along x or y), pyr."""
        assert x1 > x0 and y1 > y0 and z1 > z0, (x0, y0, z0, x1, y1, z1)
        if shape:
            self.boxes.append((x0, y0, z0, x1, y1, z1, col, trim, win, shape))
        else:
            self.boxes.append((x0, y0, z0, x1, y1, z1, col, trim, win))

    def z(self, z):
        return PAD_Z + z

    # ---- roof furniture: always rests on a roof at height `top`

    def vent(self, x, y, top, h=0.8):
        self.b(x, y, top, x + 0.7, y + 0.7, top + h, c("rock", 1))

    def ac(self, x, y, top):
        self.b(x, y, top, x + 1.6, y + 1.1, top + 0.6, c("rock", 3))

    def skylight(self, x0, y0, x1, y1, top):
        self.b(x0, y0, top, x1, y1, top + 0.18, c("water", 2))

    def roof_kit(self, x0, y0, x1, y1, top, vents=0, ac=0, sky=0):
        """Vents, AC boxes and skylight strips along a roof, spread from the left."""
        w = x1 - x0
        cy = (y0 + y1) / 2
        n = vents + ac + sky
        if n == 0 or w < 3:
            return
        step = (w - 1.4) / max(n, 1)
        kinds = ["sky"] * sky + ["ac"] * ac + ["vent"] * vents
        for i, k in enumerate(kinds):
            x = x0 + 0.7 + i * step
            if k == "vent" and x + 0.7 <= x1 - 0.3:
                self.vent(x, cy + 0.4 if (y1 - y0) > 3 else cy - 0.35, top)
            elif k == "ac" and x + 1.6 <= x1 - 0.2:
                self.ac(x, cy - 0.5 - (0.6 if (y1 - y0) > 4 else 0), top)
            elif k == "sky" and x + min(step - 0.4, 2.2) <= x1 - 0.2:
                self.skylight(x, y0 + 0.7, x + max(0.8, min(step - 0.5, 2.0)), y1 - 0.7, top)

    def door(self, x, y, w=1.4, h=1.3, col=DOOR):
        """A door or dock opening on a south wall at y (the wall's front face)."""
        self.b(x - w / 2, y - 0.25, PAD_Z, x + w / 2, y, PAD_Z + h, col)

    def truck(self, x, y, col, trailer=c("cloud", 3)):
        self.b(x, y, PAD_Z, x + 2.6, y + 1.1, PAD_Z + 1.0, trailer)
        self.b(x + 2.6, y, PAD_Z, x + 3.4, y + 1.1, PAD_Z + 0.8, col)

    def car(self, x, y, col, vert=False):
        if vert:
            self.b(x, y, PAD_Z, x + 0.7, y + 1.2, PAD_Z + 0.45, col)
        else:
            self.b(x, y, PAD_Z, x + 1.2, y + 0.7, PAD_Z + 0.45, col)

    def shed(self, x0, y0, x1, y1, h, wall, roof, trim=True, ribs=0, docks=0, vents=0, ac=0, sky=0, win=False):
        """Low hall: walls then a roof cap; optional sawtooth ribs, dock doors and roof kit."""
        top = self.z(h)
        self.b(x0, y0, PAD_Z, x1, y1, top - 0.5, wall, win=win)
        self.b(x0, y0, top - 0.5, x1, y1, top, roof, trim)
        if ribs:
            step = (x1 - x0) / ribs
            for i in range(ribs):
                a = x0 + step * i + step * 0.2
                self.b(a, y0 + 0.3, top, a + step * 0.7, y1 - 0.3, top + 0.9, c("water", 3), shape="gabley")
        else:
            self.roof_kit(x0, y0, x1, y1, top, vents, ac, sky)
        if docks and y0 - 0.25 >= -F:
            step = (x1 - x0) / docks
            for i in range(docks):
                self.door(x0 + step * (i + 0.5), y0, 1.3)
        return top

    def tower(self, x0, y0, x1, y1, h, wall, roof, trim=True, z0=None, win=True, kit=True):
        z0 = PAD_Z if z0 is None else z0
        top = z0 + h
        self.b(x0, y0, z0, x1, y1, top - 0.6, wall, win=win)
        self.b(x0, y0, top - 0.6, x1, y1, top, roof, trim)
        if kit and x1 - x0 >= 4 and y1 - y0 >= 4:
            self.b(x0 + 1, y0 + 1, top, x0 + 2.2, y0 + 2.2, top + 0.7, c("rock", 3))
            self.b(x1 - 2.6, y1 - 2.4, top, x1 - 1.0, y1 - 1.3, top + 0.55, c("rock", 3))
        return top

    def tank(self, cx, cy, w, h, wall, lid, z0=None):
        """Octagonal tank: a centre column and two side strips, with a smaller lid."""
        z0 = PAD_Z if z0 is None else z0
        a = w / 2
        self.b(cx - a, cy - a, z0, cx + a, cy + a, z0 + h, wall, shape="cyl")
        self.b(cx - a, cy - a, z0 + h, cx + a, cy + a, z0 + h + 0.4, lid, shape="dome")
        return z0 + h + 0.4

    def octa(self, cx, cy, w, z0, h, col):
        """Plain octagonal drum, no lid (cooling tower sections, silos)."""
        a = w / 2
        self.b(cx - a, cy - a, z0, cx + a, cy + a, z0 + h, col, shape="cyl")
        return z0 + h

    def cooling(self, cx, cy, w, h):
        """Hyperbolic cooling tower in three drums: wide base, narrow waist, flared rim."""
        a = w / 2
        z = PAD_Z
        self.b(cx - a, cy - a, z, cx + a, cy + a, z + h * 0.62, c("cloud", 2), shape="taper")
        b = a * 0.86
        self.b(cx - b, cy - b, z + h * 0.62, cx + b, cy + b, z + h, c("cloud", 3), shape="flare")
        return z + h

    def dike(self, x0, y0, x1, y1):
        """A low bund wall round a tank farm (four thin boxes on the pad)."""
        t, h = 0.4, 0.7
        col = c("rock", 3)
        self.b(x0, y0, PAD_Z, x1, y0 + t, PAD_Z + h, col)
        self.b(x0, y1 - t, PAD_Z, x1, y1, PAD_Z + h, col)
        self.b(x0, y0 + t, PAD_Z, x0 + t, y1 - t, PAD_Z + h, col)
        self.b(x1 - t, y0 + t, PAD_Z, x1, y1 - t, PAD_Z + h, col)

    def stack(self, cx, cy, w, h, band=True, cols=None, z0=None):
        z0 = PAD_Z if z0 is None else z0
        cols = cols or (c("brick", 2), c("cloud", 3))
        a = w / 2
        seg = h / 4.0
        for i in range(4):
            self.b(cx - a, cy - a, z0 + seg * i, cx + a, cy + a, z0 + seg * (i + 1), cols[i % 2], shape="cyl")
        self.b(cx - a - 0.2, cy - a - 0.2, z0 + h, cx + a + 0.2, cy + a + 0.2, z0 + h + 0.6, c("rock", 0), shape="cyl")
        return z0 + h + 0.6

    def column(self, cx, cy, w, h, col, cap, z0=None):
        z0 = PAD_Z if z0 is None else z0
        a = w / 2
        self.b(cx - a, cy - a, z0, cx + a, cy + a, z0 + h - 0.5, col, shape="cyl")
        self.b(cx - a, cy - a, z0 + h - 0.5, cx + a, cy + a, z0 + h, cap, shape="dome")

    def rack(self, x0, x1, y, h=2.0, col=None):
        col = col or c("signal", 3)
        for x in (x0, (x0 + x1) / 2 - 0.25, x1 - 0.5):
            self.b(x, y, PAD_Z, x + 0.5, y + 0.5, PAD_Z + h, c("rock", 1))
        self.b(x0, y - 0.25, PAD_Z + h, x1, y + 0.75, PAD_Z + h + 0.5, col)

    def pylon(self, x, y, h=7):
        self.b(x, y, PAD_Z, x + 0.7, y + 0.7, PAD_Z + h, c("rock", 1))
        self.b(x - 1.5, y + 0.15, PAD_Z + h, x + 2.2, y + 0.55, PAD_Z + h + 0.4, c("rock", 1))

    def dish(self, x, y, z0, w=2.0):
        """A satellite dish: a short post and a wide shallow bowl."""
        self.b(x + w / 2 - 0.3, y + w / 2 - 0.3, z0, x + w / 2 + 0.3, y + w / 2 + 0.3, z0 + 0.9, c("rock", 1))
        self.b(x, y, z0 + 0.9, x + w, y + w, z0 + 1.5, c("snow", 3), shape="flare")

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
        return [[round(v, 3) if isinstance(v, float) else v for v in b] for b in self.boxes]


def start():
    m = M()
    m.pad()
    return m


# ---------------------------------------------------------------- sectors

def energy(s):
    """Power station: turbine hall under a glass sawtooth, boiler house, stack, cooling towers."""
    m = start()
    m.shed(-11, -9, 1, -2, 3.5, CONCRETE, c("urban", 2), ribs=3, docks=2)
    m.tower(1, -9, 8, -1, 7, c("rock", 3), c("signal", 3), win=False, kit=False)
    m.b(1.6, -9.3, PAD_Z, 7.4, -9, PAD_Z + 1.4, c("rock", 1))  # boiler house plinth strip
    m.stack(-6, 2, 2, 11 + 2 * s)
    m.rack(-10, 0, -10.5, 1.5)
    for i in range(2 + s):  # switchyard: framed transformers
        x = 2.5 + i * 2.2
        m.b(x, -11.2, PAD_Z, x + 1.6, -9.6, PAD_Z + 1.2, c("rock", 1))
    m.pylon(9.2, -6, 8)
    m.b(8.4, -8.6, PAD_Z, 11.4, -6.8, PAD_Z + 1.2, c("rock", 0))  # coal pile
    if s >= 1:
        m.cooling(6, 6, 6.4, 9 + s)
    if s >= 2:
        m.cooling(-1.5, 8.5, 6.4, 10)
        m.shed(-11, 4, -8, 11, 3.0, CONCRETE, c("urban", 2), vents=2)
    return m


def chemical_industries(s):
    """Chemical works: bunded tank farm, pipe racks, reactor columns, flare."""
    m = start()
    m.shed(-11, -10, -4, -5, 3.0, CONCRETE, METAL, vents=2, docks=2)
    n = 2 + s
    m.dike(-11.5, 2.2, -7.8 + (n - 1) * 5.0 + 2.3 + 0.7, 9.8)
    for i in range(n):
        m.tank(-7.8 + i * 5.0, 6.0, 4.6, 3.5 + (i % 2) * 1.5, c("snow", 3), c("rock", 2), z0=PAD_Z)
    m.rack(-11, 11, 0.4, 2.0)
    m.column(7, -6, 2.4, 10 + s, c("rock", 3), c("rock", 1))
    m.column(10, -6, 2.0, 8, c("rock", 3), c("rock", 1))
    m.column(10, -9, 2.0, 6, c("rock", 3), c("rock", 1))
    m.tank(3, -8.5, 3.6, 2.5, c("signal", 3), c("rock", 2))
    m.truck(-3, -11.4, c("signal", 2), c("signal", 3))  # tanker at the loading bay
    m.b(8.5, 10.4, PAD_Z, 9.5, 11.4, PAD_Z + 11.5, c("rock", 2))  # flare stack with a bright tip
    m.b(8.3, 10.2, PAD_Z + 11.5, 9.7, 11.6, PAD_Z + 12.5, c("signal", 1))
    return m


def manufacturing(s):
    """Factory: sawtooth bays, loading docks with trucks, stack, finished goods yard."""
    m = start()
    bays = 2 + s
    w = 22.0 / bays
    for i in range(bays):
        x0 = -11 + i * w
        m.shed(x0 + 0.3, -6, x0 + w - 0.3, 6, 3.0, CONCRETE, METAL if i % 2 else c("urban", 2), ribs=3)
    m.shed(-11, -11, -1, -7.5, 2.0, c("signal", 3), TAR, docks=3)
    m.truck(1.5, -10.8, c("brick", 2))
    if s >= 1:
        m.truck(6.0, -10.8, c("water", 3))
    m.stack(9, 9, 2, 10)
    for i in range(2):
        m.b(1 + i * 3, 7.5, PAD_Z, 3.5 + i * 3, 11, PAD_Z + 1.4, c("urban", 3))
    m.b(-6.5, 7.6, PAD_Z, -2, 10.4, PAD_Z + 1.2, c("soil", 3))
    return m


def extraction(s):
    """Mine and oil field: spoil heap, headframe, pit, pumpjacks."""
    m = start()
    for i, half in enumerate([4.8, 3.4, 2.0]):  # spoil heap in steps
        m.b(-6.5 - half, 6.5 - half, PAD_Z + 1.5 * i, -6.5 + half, 6.5 + half, PAD_Z + 1.5 * (i + 1), c("soil", 1 + i))
    # headframe: two legs, a top beam and the winding house wheel
    m.b(3, -4, PAD_Z, 4, -2.5, PAD_Z + 11, c("rock", 1))
    m.b(7, -4, PAD_Z, 8, -2.5, PAD_Z + 11, c("rock", 1))
    m.b(3, -4.5, PAD_Z + 11, 8, -2, PAD_Z + 12, c("signal", 3), True)
    m.b(4.6, -3.9, PAD_Z + 12, 6.4, -2.6, PAD_Z + 13, c("rock", 3))
    m.shed(-10, -10, -2, -4, 3.0, CONCRETE, METAL, vents=2, docks=1)
    m.rack(-2, 3, -3.5, 1.2, c("rock", 2))
    # pumpjack: a base, a post and a rocking beam with a horse head
    for px, py in ([(1.5, 5.5)] + ([(1.5, 9.5)] if s >= 1 else [])):
        m.b(px, py, PAD_Z, px + 2.4, py + 1.2, PAD_Z + 0.5, c("rock", 1))
        m.b(px + 1.0, py + 0.2, PAD_Z + 0.5, px + 1.4, py + 1.0, PAD_Z + 2.4, c("rock", 3))
        m.b(px - 0.6, py + 0.3, PAD_Z + 2.4, px + 3.0, py + 0.9, PAD_Z + 3.0, c("signal", 3))
        m.b(px - 0.6, py + 0.3, PAD_Z, px - 0.2, py + 0.9, PAD_Z + 2.4, c("rock", 3))
    if s >= 1:
        m.tank(7, 4.5, 5, 4.0, c("rock", 3), c("rock", 1))
    if s >= 2:
        m.tank(7.5, 9.5, 3.5, 3.0, c("rock", 3), c("rock", 1))
        m.shed(1, -11, 8, -6.5, 2.2, CONCRETE, METAL, vents=1)
        m.truck(-9.5, -11.6, c("gold", 2), c("soil", 3))
    return m


def agriculture(s):
    """Farm: furrowed fields, red barn with a ridge, silos, farmhouse, tractor."""
    m = start()
    for i in range(5):  # furrows: alternating strips
        m.b(-11, 1.2 + i * 2.0, PAD_Z, -2, 2.2 + i * 2.0, PAD_Z + 0.2, c("field", 2 if i % 2 == 0 else 1))
    m.b(-1.5, 1, PAD_Z, 5.5, 5.5, PAD_Z + 0.2, c("field", 1))
    # barn: low walls, a stepped gable roof
    m.b(-10, -10, PAD_Z, -2, -4, PAD_Z + 2.0, c("brick", 2))
    m.b(-10.3, -10.3, PAD_Z + 2.0, -1.7, -3.7, PAD_Z + 4.4, c("brick", 1), True, shape="gablex")
    m.door(-6, -10, 2.2, 1.7, c("cloud", 3))
    n = 2 + s
    for i in range(n):  # silos with domed caps
        m.tank(1.5 + i * 2.8, -7, 2.6, 6 + (i % 2), c("cloud", 3), c("rock", 1))
    if s >= 1:
        m.shed(2, -2, 10, 0.5, 1.8, CONCRETE, c("signal", 3), vents=1)
    if s >= 2:
        m.b(6.5, 3.5, PAD_Z, 11, 10.5, PAD_Z + 2.0, c("soil", 3))
    # farmhouse and tractor
    m.b(6.0, -11, PAD_Z, 10.4, -8.8, PAD_Z + 1.6, c("cloud", 3), win=False)
    m.b(5.8, -11.2, PAD_Z + 1.6, 10.6, -8.6, PAD_Z + 2.9, c("brick", 2), True, shape="gablex")
    m.b(-1.5, -11.2, PAD_Z, 0.1, -10.2, PAD_Z + 0.9, c("signal", 2))
    return m


def logistics(s):
    """Distribution centre: dock doors, trailers in the yard, container stacks, gantry crane."""
    m = start()
    m.shed(-11, -3, 11, 8, 3.0, CONCRETE, c("urban", 2), docks=6, vents=2, sky=2)
    m.truck(-10, -5.4, c("signal", 3))
    m.truck(-4.5, -5.4, c("water", 3), c("signal", 2))
    cols = [c("signal", 2), c("water", 3), c("signal", 3), c("signal", 1)]
    for i in range(3 + 2 * s):  # container stack
        x = -11 + (i % 6) * 3.6
        y = -10 + (i // 6) * 2.4
        m.b(x, y, PAD_Z, x + 3.0, y + 1.8, PAD_Z + 1.2, cols[i % 4])
        if i % 3 == 0:
            m.b(x, y, PAD_Z + 1.2, x + 3.0, y + 1.8, PAD_Z + 2.4, cols[(i + 1) % 4])
    if s >= 1:
        m.crane(10.2, -6, 8, 8, rot=1)
    if s >= 2:
        m.b(-11, 8.6, PAD_Z, 11, 11, PAD_Z + 1.0, c("urban", 3))
    return m


def construction(s):
    """Construction firm yard: tower crane, site office, pipe stacks, spoil, plant."""
    m = start()
    m.crane(-6, 4, 14 + 2 * s, 9)
    m.shed(-11, -10, -3, -5, 2.0, c("gold", 2), c("urban", 2), win=False, docks=1)
    for i in range(2 + s):
        m.tank(-0.5 + i * 3.4, -7.5, 3.0, 5.5, c("rock", 3), c("rock", 1))
    for i in range(3):
        m.b(1.5 + i * 3.2, 4, PAD_Z, 4.2 + i * 3.2, 8, PAD_Z + 1.0 + i * 0.5, c("soil", 2))
    if s >= 2:
        m.b(-2, 6.5, PAD_Z, 0.5, 10, PAD_Z + 2.0, c("signal", 3))
    m.truck(7.5, -11.2, c("gold", 3), c("soil", 3))
    return m


def site(s):
    """A construction site: a foundation, a rising frame, a crane, a cabin and materials."""
    m = start()
    n = 1 + s
    m.b(-8, -8, PAD_Z, 6, 6, PAD_Z + 0.8, c("rock", 3))
    for lvl in range(n + 1):  # columns and the floors that rest on them, one storey per size step
        z = PAD_Z + 0.8 + lvl * 3.0
        for x, y in [(-8, -8), (-8, 4.5), (4.5, -8), (4.5, 4.5), (-1.5, -1.5)]:
            m.b(x, y, z, x + 1.5, y + 1.5, z + 2.5, c("urban", 3))
        if lvl < n:
            m.b(-8, -8, z + 2.5, 6, 6, z + 3.0, c("rock", 3))
    m.crane(8, -1, 8 + 2 * n + 3, 9, rot=1)
    m.b(-11, 8, PAD_Z, -4, 11, PAD_Z + 1.2, c("signal", 3))  # site cabin
    m.b(-11, 8, PAD_Z + 1.2, -4, 11, PAD_Z + 1.5, c("cloud", 3))
    m.b(8.5, -10.5, PAD_Z, 11, -7, PAD_Z + 1.0, c("soil", 2))  # sand heap
    m.b(-4, 8.5, PAD_Z, -0.5, 11, PAD_Z + 0.7, c("gold", 2))  # pallets
    return m


def defense(s):
    """Base: hangar, vehicle park, radar dome, apron strip, guard tower."""
    m = start()
    m.b(-11.5, -11.5, PAD_Z, 11.5, -9.5, PAD_Z + 0.2, c("rock", 1))  # apron strip
    m.shed(-10, -6, 0, 5, 3.0, c("forest", 1), c("forest", 0), trim=True)
    m.b(-8, -6, PAD_Z + 3.0, -2, 5, PAD_Z + 3.7, c("forest", 0), True)  # hangar crown
    m.door(-5, -6, 4.0, 2.4)
    for i in range(2 + s):  # armoured vehicle park
        m.b(1 + (i % 3) * 3.6, -6 + (i // 3) * 3.5, PAD_Z, 4 + (i % 3) * 3.6, -3.5 + (i // 3) * 3.5, PAD_Z + 1.4, c("forest", 2))
        x, y = 1 + (i % 3) * 3.6, -6 + (i // 3) * 3.5
        m.b(x + 0.8, y + 0.6, PAD_Z + 1.4, x + 2.2, y + 1.9, PAD_Z + 2.0, c("forest", 1))  # turret
    m.tank(-8, 8.5, 4.5, 2.0, c("rock", 3), c("rock", 1))  # radar dome base
    m.column(-2, 9, 2, 6, c("rock", 3), c("signal", 2))  # guard tower
    m.b(-8.6, 7.9, PAD_Z + 2.4, -7.4, 9.1, PAD_Z + 3.4, c("snow", 3))  # radar dome
    if s >= 2:
        m.shed(2, 6, 11, 11.5, 1.8, c("forest", 1), c("forest", 0))
    return m


def media(s):
    """Broadcaster: studio with a bright roof, transmitter mast, dishes, outside broadcast van."""
    m = start()
    m.shed(-11, -10, 1, -1, 3.5, c("snow", 3), c("signal", 3), win=True, docks=1)
    m.shed(3, -10, 10, -4, 2.5, c("urban", 3), c("urban", 1), trim=False, vents=1)
    mast = 15 + 2 * s
    m.b(-0.5, 2, PAD_Z, 1.5, 4, PAD_Z + mast, c("rock", 3))
    m.b(-1.5, 1.5, PAD_Z + mast, 2.5, 4.5, PAD_Z + mast + 0.8, c("signal", 2))
    m.b(0.1, 2.6, PAD_Z + mast + 0.8, 0.9, 3.4, PAD_Z + mast + 2.8, c("signal", 2))
    m.dish(-8, 5, PAD_Z, 3.2)
    if s >= 1:
        m.dish(6, 4.2, PAD_Z, 3.2)
    m.truck(4, -11.4, c("signal", 3), c("snow", 3))
    if s >= 2:
        m.shed(4, 8, 11, 11.5, 1.6, c("urban", 3), c("urban", 1), ac=2)
    return m


def healthcare(s):
    """Hospital: wings with window rows, red cross, helipad, ambulance bay, car park."""
    m = start()
    wall, roof = c("snow", 3), c("cloud", 1)
    h = 4.5 + s * 0.8
    m.tower(-10, -4, -3, 7, h, wall, roof, trim=True, kit=False)
    m.tower(3, -4, 10, 7, h, wall, roof, trim=True, kit=False)
    m.tower(-3, -2, 3, 1, 3.0, wall, roof, trim=True, kit=False)
    top = PAD_Z + h
    m.b(-7.2, 1.5, top, -5.8, 6.5, top + 0.3, c("signal", 2))
    m.b(-8.8, 3.4, top, -7.2, 4.6, top + 0.3, c("signal", 2))
    m.b(-5.8, 3.4, top, -4.2, 4.6, top + 0.3, c("signal", 2))
    m.shed(-10, -10, -2, -6, 2.0, c("cloud", 2), c("urban", 3), docks=2)
    m.b(4, -11, PAD_Z, 10, -6, PAD_Z + 0.35, c("urban", 2))  # helipad
    m.b(5.8, -9.2, PAD_Z + 0.35, 8.2, -7.8, PAD_Z + 0.55, c("signal", 1))
    for i in range(3):
        m.car(-1.4 + i * 1.5, -8.6, c("signal", 1) if i == 1 else c("cloud", 3))
    return m


def technology(s):
    """Tech campus: glass office towers, data hall with rooftop chillers, solar canopy."""
    m = start()
    m.tower(-10, -9, -3, -2, 4 + s * 1.5, c("water", 3), c("cloud", 3))
    m.tower(2, -9, 10, -1, 5 + s * 1.5, c("water", 3), c("cloud", 3))
    top = m.shed(-10, 2, 6, 10, 2.5, c("urban", 3), c("rock", 2), ribs=0, win=False, docks=1)
    for i in range(4):
        m.b(-9 + i * 3.4, 3, top, -7.4 + i * 3.4, 5.6, top + 0.9, c("rock", 3))
        m.b(-9 + i * 3.4, 6.6, top, -7.4 + i * 3.4, 9.2, top + 0.9, c("rock", 3))
    m.b(-10, -11, PAD_Z, -1, -10, PAD_Z + 1.0, c("water", 1))  # solar canopy row
    if s >= 2:
        m.tower(7, 3, 11, 10, 8, c("water", 3), c("cloud", 3))
    return m


def financial(s):
    """Finance house: a stepped tower with a crown, flanking offices, plaza."""
    m = start()
    hmain = 17 + 2 * s
    top = m.tower(-3, -3, 3.5, 3.5, 9, c("urban", 3), c("urban", 2), trim=False, kit=False)
    top2 = m.tower(-2, -2, 2.5, 2.5, hmain - 9, c("cloud", 2), c("cloud", 3), z0=top, kit=False)
    m.b(0, 0, top2, 1, 1, top2 + 1.8, c("rock", 3))  # spire
    m.tower(-10, -9, -4.5, -4, 8 + s, c("urban", 3), c("urban", 2))
    m.tower(5, -9, 10.5, -3.5, 10 + s, c("water", 3), c("cloud", 3))
    if s >= 1:
        m.shed(-10, 5, 10, 10, 2.0, c("cloud", 2), c("urban", 2), win=True, ac=2)
    m.b(-11, -11, PAD_Z, 11, -9.6, PAD_Z + 0.4, c("rock", 3))
    return m


def retail(s):
    """Shopping centre: big box with an entrance, pylon sign, painted car park with cars."""
    m = start()
    m.shed(-11, 0, 5, 9, 3.0, c("signal", 3), c("cloud", 2), ac=3, win=False)
    m.door(-3, 0, 4.0, 1.8, c("water", 1))
    m.b(-11, 9.3, PAD_Z, 5, 10, PAD_Z + 1.0, c("urban", 3))
    for i in range(7):  # car park lines
        m.b(-10 + i * 2.4, -10, PAD_Z, -9.4 + i * 2.4, -2, PAD_Z + 0.15, c("cloud", 3))
    for i, (bay, y, col) in enumerate([(0, -9.4, c("brick", 3)), (0, -5.0, c("cloud", 3)), (2, -8.0, c("water", 3)),
                                       (3, -6.2, c("signal", 1)), (5, -9.0, c("brick", 3)), (5, -4.2, c("cloud", 3))]):
        m.car(-9.4 + 2.4 * bay + 0.55, y, col, vert=True)
    m.b(8, -10, PAD_Z, 8.8, -9.2, PAD_Z + 6, c("rock", 2))
    m.b(7.0, -10.4, PAD_Z + 6, 9.8, -8.8, PAD_Z + 7.4, c("signal", 2))
    if s >= 1:
        m.shed(6, 1, 11, 6, 2.2, CONCRETE, c("urban", 3), vents=1)
    if s >= 2:
        m.shed(6, 6.5, 11, 10, 2.2, CONCRETE, c("signal", 3))
    return m


def real_estate(s):
    """Apartment blocks with stairwell crowns, gardens, terraced row."""
    m = start()
    n = 2 + s
    for i in range(n):
        x = -10 + i * (20.0 / n)
        wx = 20.0 / n - 1.0
        h = 6 + (i % 2) * 2
        top = m.tower(x, -4, x + wx, 4, h, c("brick", 2), c("urban", 2), kit=False)
        m.b(x + wx / 2 - 0.8, -0.8, top, x + wx / 2 + 0.8, 0.8, top + 0.9, c("rock", 3))  # stair crown
    m.b(-11, -11, PAD_Z, 11, -6, PAD_Z + 0.2, c("forest", 3))
    for i in range(3):  # terrace row
        m.b(-9 + i * 7, 6.5, PAD_Z, -5 + i * 7, 10, PAD_Z + 2.0, c("soil", 3), win=True)
        m.b(-9 + i * 7, 6.5, PAD_Z + 2.0, -5 + i * 7, 10, PAD_Z + 2.6, TILE, True)
    for x in (-9.0, -3.0, 4.0):
        m.b(x, -9.5, PAD_Z + 0.2, x + 1.4, -8.1, PAD_Z + 1.4, c("forest", 1))  # trees
    return m


def telecommunications(s):
    """Mast site: banded lattice mast with dishes, equipment sheds, generator."""
    m = start()
    z = PAD_Z + 0.8
    for i in range(3 + s):
        zb = PAD_Z + 6 + i * 5
        m.b(-1, -1, z, 1, 1, zb, c("rock", 3))
        m.b(-2.2, -0.8, zb, 2.2, 0.8, zb + 0.8, c("signal", 2))
        z = zb + 0.8
    m.b(-1, -1, z, 1, 1, PAD_Z + 28, c("rock", 3))
    m.b(-1.4, -1.4, PAD_Z + 28, 1.4, 1.4, PAD_Z + 29, c("signal", 2))
    m.b(-2.4, -2.4, PAD_Z, 2.4, 2.4, PAD_Z + 0.8, c("rock", 1))  # mast base
    m.shed(-10, -10, -2, -5, 2.0, CONCRETE, c("urban", 2), ac=1, docks=1)
    m.shed(3, -10, 10, -5, 2.0, CONCRETE, c("urban", 2), ac=2) if s >= 1 else None
    m.dish(6, 5, PAD_Z, 4.0)
    if s >= 2:
        m.dish(-9, 5, PAD_Z, 4.0)
    m.b(-6, 2, PAD_Z, -3.4, 3.6, PAD_Z + 1.2, c("signal", 3))  # generator
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
        m.tower(x0, y0, x1, y1, h + s, c(col, 2), c("rock", 2) if col != "urban" else c("urban", 1))
    return m


DOWNTOWN_HEIGHTS = {
    # Nine lots, south row first. Tier 0 is a small city's low centre, tier 3 a metropolis core.
    "t0a": [4, 0, 5, 3, 6, 4, 5, 3, 0],
    "t0b": [5, 3, 4, 0, 5, 3, 4, 6, 3],
    "t1a": [6, 9, 6, 8, 11, 7, 5, 8, 6],
    "t1b": [7, 6, 10, 6, 12, 8, 9, 5, 7],
    "t2a": [9, 14, 10, 13, 18, 12, 8, 15, 9],
    "t2b": [10, 12, 16, 14, 19, 11, 12, 9, 14],
    "t3a": [11, 17, 12, 16, 21.5, 15, 12, 18, 10],
    "t3b": [13, 15, 19, 17, 21, 14, 11, 16, 13],
}
LOTS = [(-11.6, -5.4), (-3.1, 3.1), (5.4, 11.6)]
WALLS = [c("cloud", 2), c("urban", 3), c("water", 3), c("snow", 2), c("urban", 2), c("cloud", 3)]


def downtown(key):
    """A downtown block of nine lots: towers with setbacks and a crown, taller toward the middle.
    The tier says how big the city is; the block is scenery, nobody's, so nothing is trim."""
    m = start()
    heights = DOWNTOWN_HEIGHTS[key]
    tier = int(key[1])
    for i, h in enumerate(heights):
        gy, gx = divmod(i, 3)
        x0, x1 = LOTS[gx]
        y0, y1 = LOTS[gy]
        if h == 0:
            m.b(x0 + 0.5, y0 + 0.5, PAD_Z, x1 - 0.5, y1 - 0.5, PAD_Z + 0.25, c("forest", 2))  # a park
            continue
        wall = WALLS[(i * 5 + tier * 2 + (1 if key.endswith("b") else 0)) % len(WALLS)]
        roof = c("urban", 1) if wall == c("urban", 2) else c("rock", 2)
        # Podium under the tall ones, then the tower with a setback.
        if h >= 12:
            m.b(x0, y0, PAD_Z, x1, y1, PAD_Z + 2.0, c("urban", 3), win=True)
            z = PAD_Z + 2.0
            h -= 2.0
        else:
            z = PAD_Z
        lo = h * 0.62 if h >= 8 else h
        top = m.tower(x0, y0, x1, y1, lo, wall, roof, trim=False, z0=z, kit=h < 8)
        if h >= 8:
            top = m.tower(x0 + 1.2, y0 + 1.2, x1 - 1.2, y1 - 1.2, h - lo, wall, roof, trim=False, z0=top, kit=h < 14)
        if h >= 14:  # crown: a mast on the roof
            cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
            m.b(cx - 0.4, cy - 0.4, top, cx + 0.4, cy + 0.4, top + (1.8 if i == 4 and tier == 3 else 1.1), c("rock", 3))
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
    for name in DOWNTOWN_HEIGHTS:
        key = f"downtown/{name}"
        boxes = downtown(name).done()
        validate(key, boxes)
        models[key] = {"w": 2 * F, "d": 2 * F, "boxes": boxes}
    if ERRS:
        raise SystemExit("\n".join(sorted(set(ERRS))))
    OUT.write_text(json.dumps({"note": "GPU map plant models; see tools/plant_models.py", "models": models}, separators=(",", ":")))
    print(f"wrote {len(models)} models to {OUT}")


if __name__ == "__main__":
    main()
