#!/usr/bin/env python3
"""1.9 cast portraits, a proposal that is not wired into the app yet.

Draws 32 by 32 busts with three expressions (neutral, pleased, displeased) for the forty people in
content/cast-1.9.json, a 64 by 64 office portrait for each chief executive, and a kit of generated
extras. Output is apple/Resources/cast-portraits-v2.json in the same palette-and-rows format as
cast-portraits.json: '.' is transparent and each letter maps to that portrait's own palette.

Every portrait is built from shapes in 32 by 32 units and sampled at the pixel centres, so the 64 by
64 office portrait is the same person at twice the resolution. Light comes from the upper left:
highlights sit top left, shade on the right edge, under the chin and under hair and hats. Only the
eyes, brows and mouth differ between expressions.

Palette letters: o outline; t s d skin light, base, shade (d also draws the mouth); h j k hair base,
highlight, shade (k also draws brows); c e coat base and shade; a accent (tie, blouse, hat band);
w white (shirt and the whites of the eyes); l spectacle lens. Office portraits add y Y g G for the
wall and the frame.

The 16 by 16 busts are derived from the 32 by 32 ones by downscale16(), see its docstring.

    tools/cast_portraits_v2.py                   write the atlas
    tools/cast_portraits_v2.py --sheets DIR      also write the contact sheets and roster summary
"""
import json
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'apple/Resources/cast-portraits-v2.json'
ROSTER = ROOT / 'content/cast-1.9.json'
OLD_CAST = ROOT / 'content/cast.json'

SIZE = 32
OFFICE = 64
SMALL = 16
EXPRESSIONS = ('neutral', 'pleased', 'displeased')
EXTRAS_SAMPLE = 24

# ---- Colours ---------------------------------------------------------------------------------

OUTLINE = '#1B1B22'
WHITE = '#E8EEF4'
LENS = '#CFE6F5'
FRAME = '#7C8AA6'

SKINS = [  # light, base, shade
    ('#FBE0CC', '#F1C7A4', '#C98A6E'),
    ('#F6D4B8', '#E8BE98', '#B87A5E'),
    ('#EEC79A', '#DDAB7A', '#A8704C'),
    ('#E0A978', '#C98C5E', '#8E5638'),
    ('#CC9768', '#AE7548', '#6E4128'),
    ('#B07A52', '#946038', '#5A3322'),
    ('#9A6642', '#774A30', '#432619'),
]
HAIRS = {  # base, highlight, shade
    'black': ('#2A2522', '#5A524C', '#120F0E'),
    'darkbrown': ('#4A2E20', '#7A5640', '#26160F'),
    'brown': ('#6A4B36', '#946F52', '#3E2A1E'),
    'chestnut': ('#94603A', '#C28A5C', '#62391F'),
    'auburn': ('#8E3F24', '#BE6E48', '#5A2412'),
    'blond': ('#CBA45E', '#EDD698', '#9C7A3A'),
    'grey': ('#8E8E96', '#C4C4CC', '#5E5E68'),
    'white': ('#C8C8D0', '#F0F0F6', '#8C8C96'),
    'saltpepper': ('#6E6E76', '#A6A6AE', '#40404A'),
}
COATS = {  # base, shade
    'navy': ('#2E4A78', '#1E3250'),
    'charcoal': ('#3E434E', '#272A33'),
    'brown': ('#6A5238', '#46341F'),
    'green': ('#2F5A44', '#1F3A2C'),
    'burgundy': ('#7A2E3C', '#521E28'),
    'teal': ('#2F6F6B', '#1F4B48'),
    'purple': ('#5A3A66', '#3A2444'),
    'tan': ('#A28C66', '#74603F'),
    'steel': ('#5A6578', '#3A4352'),
    'black': ('#2C2C36', '#17171E'),
    'rust': ('#A04A2E', '#6E3220'),
    'olive': ('#5E5E3A', '#3C3C22'),
    'grey': ('#6A6E78', '#44474F'),
}
ACCENTS = {
    'red': '#B02E2E', 'gold': '#D8B24A', 'blue': '#3A64B0', 'cream': '#E8D8A8',
    'orange': '#D8702E', 'teal': '#2F9A9A', 'pink': '#C86A8A', 'green': '#3A8A5A',
    'burgundy': '#7A1F2B', 'rust': '#C4572B', 'brown': '#7A5232',
}
WALLS = {  # office wall light, wall dark, frame light, frame dark per scenario
    'texas-1981': ('#6E4B33', '#4A3123', '#E2B04A', '#9A7428'),
    'great-lakes-2000': ('#3F5A66', '#2A3E48', '#E2B04A', '#9A7428'),
    'silicon-valley-1995': ('#3C5F63', '#274244', '#E2B04A', '#9A7428'),
    'ruhr-rhine-1991': ('#5E3A44', '#3E2430', '#E2B04A', '#9A7428'),
}

# ---- Geometry (32 unit space; pixel (x, y) has its centre at (x + .5, y + .5) / scale) --------

JAW = {'round': (17.5, 1.0, 3.0), 'square': (19.5, 1.0, 4.0), 'narrow': (15.5, 0.8, 2.5)}


def head_hw(v, W, jaw):
    if v < 4.0 or v > 22.0:
        return -1.0
    top = min(W, 4.0 + (v - 4.5))
    start, slope, floor = JAW[jaw]
    bottom = W if v <= start else max(floor, W - slope * (v - start))
    return min(top, bottom)


def body_hw(v, pad):
    if v < 23.0:
        return -1.0
    return min(15.0, 4.2 + (4.4 if pad else 3.0) * (v - 23.0))


def noise(u, v):
    a, b = int(abs(u - 16.0)), int(v)
    h = (a * 374761393 + b * 668265263) & 0xFFFFFFFF
    h = ((h ^ (h >> 13)) * 1274126177) & 0xFFFFFFFF
    return (((h >> 16) & 0xFF) / 255.0 - 0.5) * 2.0


# Hair: an ellipse around the head minus a face window (below the hairline, inside the sideburns).
# top is the highest row of hair, padx the overshoot beyond the skull, hl0 the hairline at the
# centre, sb the sideburn width, sbend where the side hair stops, recede lifts the temples, tilt
# lowers the right side for a part, length adds side curtains to that row, bun adds a knot.
HAIR = {
    'bald': None,
    'buzz': dict(top=3.6, padx=0.1, hl0=7.0, sb=0.8, sbend=10.5),
    'crop': dict(top=3.0, padx=0.4, hl0=7.5, sb=1.0, sbend=11.0),
    'short': dict(top=2.4, padx=0.7, hl0=8.0, sb=1.0, sbend=11.5, tilt=0.8),
    'side_part': dict(top=2.2, padx=0.8, hl0=7.6, sb=1.0, sbend=11.5, tilt=1.6, part=12.5),
    'mop': dict(top=2.4, padx=1.0, hl0=8.8, sb=1.5, sbend=13.5, tilt=1.2, noise=0.3),
    'slick': dict(top=1.8, padx=1.2, hl0=6.6, sb=1.0, sbend=12.0, recede=1.2),
    'wave': dict(top=1.8, padx=1.5, hl0=7.0, sb=1.3, sbend=13.0, recede=0.7, noise=0.5),
    'perm': dict(top=0.8, padx=2.4, hl0=7.6, sb=1.3, sbend=16.0, noise=0.8),
    'afro_round': dict(top=0.4, padx=3.0, hl0=7.6, sb=1.3, sbend=12.5, noise=0.8),
    'curly_short': dict(top=2.2, padx=1.3, hl0=7.4, sb=1.2, sbend=12.0, noise=0.7),
    'bob': dict(top=2.6, padx=1.6, hl0=7.8, sb=1.8, sbend=12.0, length=19.5, cw=1.8, tilt=0.6),
    'bob_fringe': dict(top=2.6, padx=1.6, hl0=9.0, sb=1.8, sbend=12.0, length=19.5, cw=1.8),
    'long': dict(top=2.0, padx=1.4, hl0=7.6, sb=1.8, sbend=12.0, length=28.0, cw=2.6, tilt=0.6),
    'pixie': dict(top=2.6, padx=0.9, hl0=8.4, sb=1.5, sbend=13.0, tilt=1.0),
    'bun': dict(top=2.8, padx=0.9, hl0=7.2, sb=1.3, sbend=11.5, knot=(16.0, 2.2, 3.2, 2.2)),
    'bun_high': dict(top=2.8, padx=1.1, hl0=7.6, sb=1.3, sbend=12.0, knot=(16.0, 2.0, 3.8, 2.2), noise=0.3),
    'horseshoe': dict(horseshoe=True),
}


def hairline(H, W, u):
    du = u - 16.0
    hl = H['hl0'] + H.get('tilt', 0.0) * du / W
    hl -= H.get('recede', 0.0) * min(1.0, abs(du) / (0.8 * W))
    return hl


def hair_mask(H, W, u, v):
    """True where hair covers the pixel (before shading)."""
    if H is None:
        return False
    du = u - 16.0
    adu = abs(du)
    if H.get('horseshoe'):
        return 10.0 <= v <= 15.0 and W - 1.2 <= adu <= W + 0.8
    n = H.get('noise', 0.0) * noise(u, v)
    rx = W + H['padx'] + n
    ry = 11.0 - H['top'] + n
    cov = False
    if v <= H['sbend'] and (du / rx) ** 2 + ((v - 11.0) / ry) ** 2 <= 1.0:
        window = v >= hairline(H, W, u) and adu <= W - H['sb']
        cov = not window
    if H.get('length') and 7.0 <= v <= H['length']:
        if W - H['sb'] <= adu <= W + H['cw'] and (v <= 21.0 or adu >= 4.0):
            cov = True
    if H.get('knot'):
        cx, cy, kx, ky = H['knot']
        if ((u - cx) / kx) ** 2 + ((v - cy) / ky) ** 2 <= 1.0:
            cov = True
    return cov


def hair_tone(H, W, u, v):
    du = u - 16.0
    rx = W + H.get('padx', 1.0)
    ry = max(2.0, 11.0 - H.get('top', 2.0))
    l = -(0.7 * du / rx + 0.8 * (v - 11.0) / ry)
    if 'hl0' in H:
        near = abs(du) <= W - H['sb'] + 1.0 and v >= hairline(H, W, u) - 1.1 and v >= 6.0
        if H.get('part') and abs(u - H['part']) < 0.5 and v < hairline(H, W, u) + 0.5:
            return 'k'
        if near:
            return 'k'
    if l > 0.52:
        return 'j'
    if l < -0.42:
        return 'k'
    return 'h'


def skin_tone(u, v, W):
    nx = (u - 16.0) / W
    ny = (v - 13.0) / 9.0
    l = -(0.75 * nx + 0.65 * ny)
    if l > 0.45:
        return 't'
    if l < -0.6 or u - 16.0 > W - 1.2:
        return 'd'
    return 's'


# ---- Layers -----------------------------------------------------------------------------------

def body_char(spec, u, v):
    pad = spec.get('pad', False)
    hw = body_hw(v, pad)
    du = u - 16.0
    adu = abs(du)
    if adu > hw:
        return None
    style = spec['body']
    ch = 'c'
    if du > 7.4 + (27.0 - min(v, 27.0)) * 0.25 or (du > 5.0 and v > 29.0):
        ch = 'e'
    t = v - 23.0
    if style in ('tie', 'bow'):
        sk = 3.0 - 1.0 * t
        sv = 5.2 - 0.85 * t
        if adu < sk:
            ch = 'd'
        elif adu < sv:
            ch = 'w'
        elif adu < sv + 0.9 and v > 24.0:
            ch = 'e'
        if style == 'tie':
            knot = 24.0 <= v < 26.0 and adu <= 1.0
            blade = v >= 26.0 and adu <= min(2.0, 1.0 + 0.5 * (v - 26.0))
            if knot or blade:
                ch = 'a' if du < 0.3 or v > 26.0 and du < 1.2 else 'a'
        else:
            if 24.0 <= v < 26.2 and adu <= 3.4 and adu >= 0.9:
                ch = 'a'
            elif 24.0 <= v < 26.2 and adu < 0.9:
                ch = 'e'
    elif style == 'open':
        sk = 4.2 - 0.85 * t
        sv = 5.8 - 0.75 * t
        if adu < sk:
            ch = 'd'
        elif adu < sv:
            ch = 'w'
        elif adu < sv + 0.9 and v > 24.0:
            ch = 'e'
    elif style in ('blouse', 'blouse_round'):
        if style == 'blouse':
            sk = 3.4 - 0.75 * t
            sv = 5.8 - 0.8 * t
        else:
            sk = 3.6 * math.sqrt(max(0.0, 1.0 - (t / 3.2) ** 2)) if t < 3.2 else 0.0
            sv = 5.6 - 0.35 * t
        if adu < sk:
            ch = 'd'
        elif adu < sv:
            ch = 'a'
        elif adu < sv + 0.9 and v > 24.0:
            ch = 'e'
        if spec.get('pearls'):
            pv = 24.4 + 0.12 * du * du
            if abs(v - pv) < 0.55 and adu <= 3.6:
                ch = 'w'
    elif style == 'turtle':
        sv = 4.6 - 0.7 * t
        if adu < sv:
            ch = 'a'
        elif adu < sv + 0.9 and v > 24.0:
            ch = 'e'
    elif style == 'knit':
        ring = (du / 3.7) ** 2 + (t / 2.6) ** 2
        if ring <= 1.0:
            ch = 'd'
        elif ring <= 1.9:
            ch = 'e'
        if 23.0 <= v < 24.8 and 3.0 <= adu <= 5.0 and ring > 1.9:
            ch = 'a'
    elif style == 'work':
        sk = 3.2 - 1.2 * t
        if adu < sk:
            ch = 'd'
        elif 23.0 <= v < 26.0 and adu < 5.6 - 0.9 * t + 0.6:
            ch = 'e'
        elif adu < 0.6 and v >= 25.0:
            ch = 'e'
        elif adu < 1.4 and v >= 25.0 and du < 0:
            ch = 'a' if False else ch
    elif style == 'scarf':
        sk = 4.2 - 0.85 * t
        sv = 5.8 - 0.75 * t
        if adu < sk:
            ch = 'd'
        elif adu < sv:
            ch = 'w'
        elif adu < sv + 0.9 and v > 24.0:
            ch = 'e'
        band = 22.6 <= v < 25.6 and adu <= 4.8 - 0.1 * (v - 22.6) * 2
        hang = 25.0 <= v < 30.0 and -2.6 <= du <= 0.4
        if band or hang:
            ch = 'a'
    return ch


def neck_char(u, v):
    if 19.0 <= v < 26.0 and abs(u - 16.0) <= 3.0:
        if v < 23.7 or u - 16.0 > 1.8:
            return 'd'
        return 's'
    return None


def beard_mask(spec, W, jaw, u, v):
    kind = spec.get('beard')
    if not kind:
        return False
    adu = abs(u - 16.0)
    hw = head_hw(v, W, jaw)
    if hw < 0 or adu > hw:
        return False
    mous = 15.7 <= v < 17.6 and adu <= 3.6 and not (v >= 17.0 and adu <= 1.0 and False)
    if kind == 'moustache':
        return mous
    if kind == 'goatee':
        return mous or (19.2 <= v and adu <= 2.4) or (18.0 <= v < 19.2 and adu >= 3.4 and adu <= 4.2 and False)
    if kind == 'beard':
        return mous or v >= 17.0 or (v >= 14.6 and adu >= hw - 2.4)
    return False


def hat_char(spec, u, v):
    kind = spec.get('hat')
    du = u - 16.0
    adu = abs(du)
    if kind == 'stetson':
        crown = 0.8 <= v < 6.3 and adu <= 5.4 - 0.12 * max(0.0, 2.2 - v) * 3 and not (v < 1.7 and adu < 2.4)
        if v < 1.7 and adu < 3.0:
            crown = v >= 1.0 and adu >= 2.4
        brim_v = 8.1 - 0.5 * max(0.0, adu - 7.0)
        brim = 6.3 <= v < brim_v and adu <= 11.0
        if crown:
            if 4.9 <= v < 6.3:
                return 'a'
            return 'e' if du > 2.8 else 'c'
        if brim:
            return 'e' if (du > 5.0 or v > brim_v - 1.0) else 'c'
    if kind == 'flatcap':
        dome = ((du / 9.0) ** 2 + ((v - 7.0) / 4.6) ** 2 <= 1.0) and v < 7.2
        band = 7.2 <= v < 8.4 and adu <= 8.2
        bill = 8.4 <= v < 9.4 and adu <= 6.6
        if bill:
            return 'e'
        if band:
            return 'e' if du > 2.0 else 'c'
        if dome:
            if v > 3.6 and abs(du) < 0.5:
                return 'e'
            return 'e' if du > 4.0 else 'c'
    return None


# Eye, brow and mouth shapes per expression. Eyes are 3 wide, 2 tall for the left eye (columns
# 11 to 13, rows 12 and 13) and mirrored for the right. 'o' outline, 'w' white, 's' leaves skin,
# 'd' shade, '.' leaves whatever is there. Brows and mouth are lists of (x, y) pixels.
EYES = {
    'neutral': ['sss', 'wow'],
    'pleased': ['.o.', 'o.o'],
    'displeased': ['ooo', 'wow'],
}
BROWS = {  # left brow, mirrored for the right; a row above the eyes' block row so 16 by 16 keeps a gap
    'neutral': [(10, 9), (11, 9), (12, 9), (13, 9)],
    'pleased': [(10, 9), (11, 8), (12, 8), (13, 9)],
    'displeased': [(10, 8), (11, 8), (12, 9), (13, 10)],
}
MOUTH = {  # centred, mirrored automatically by listing both sides
    'neutral': [(14, 18), (15, 18), (16, 18), (17, 18)],
    'pleased': [(14, 18), (15, 18), (16, 18), (17, 18), (13, 17), (18, 17)],
    'displeased': [(14, 19), (15, 19), (16, 19), (17, 19), (13, 20), (18, 20)],
}
LENS_ROWS = {
    'round': ['.ooo.', 'o...o', 'o...o', 'o...o', '.ooo.'],
    'square': ['ooooo', 'o...o', 'o...o', 'o...o', 'ooooo'],
}


def glasses_pixels(kind):
    """(x, y, char) for the left lens at columns 10 to 14, rows 11 to 15, mirrored for the right."""
    out = []
    for dy, row in enumerate(LENS_ROWS[kind]):
        for dx, c in enumerate(row):
            if c == 'o':
                out.append((10 + dx, 11 + dy, 'f'))
            elif c == '.' and 0 < dx < 4 and 0 < dy < 4:
                out.append((10 + dx, 11 + dy, 'l'))
    return out


def new_grid(n):
    return [['.'] * n for _ in range(n)]


def mirror_x(x):
    return 31 - x


def render(spec, expression, k=1):
    """One portrait as rows of letters at SIZE * k pixels square."""
    n = SIZE * k
    g = new_grid(n)
    W = spec.get('W', 7)
    jaw = spec.get('jaw', 'round')
    H = HAIR[spec['hair']]

    def each(fn):
        for y in range(n):
            u_row = None
            v = (y + 0.5) / k
            for x in range(n):
                u = (x + 0.5) / k
                r = fn(u, v)
                if r:
                    g[y][x] = r

    each(lambda u, v: neck_char(u, v))
    each(lambda u, v: body_char(spec, u, v))

    def head(u, v):
        hw = head_hw(v, W, jaw)
        if hw < 0:
            return None
        adu = abs(u - 16.0)
        if adu <= hw:
            return skin_tone(u, v, W)
        if 12.0 <= v < 15.0 and adu <= hw + 1.0 and hw >= W - 0.01:
            return 'd' if u > 16.0 else 's'
        return None

    each(head)

    # chin shadow on the neck is part of neck_char; deepen skin under the hairline and hat brim
    def beard(u, v):
        if beard_mask(spec, W, jaw, u, v):
            du = u - 16.0
            rx = W
            l = -(0.7 * du / rx + 0.8 * (v - 17.0) / 6.0)
            if l > 0.55:
                return 'j'
            if l < -0.35:
                return 'k'
            return 'h'
        return None

    each(beard)

    def hair(u, v):
        if hair_mask(H, W, u, v):
            return hair_tone(H, W, u, v)
        return None

    if H is not None:
        each(hair)
        despeckle(g, k, W, jaw)

    if spec.get('hat'):
        each(lambda u, v: hat_char(spec, u, v))
        if spec['hat'] == 'stetson':
            def shade(u, v):
                if g[int(v * k)][int(u * k)] in 'sdt' and v < 9.4 and abs(u - 16.0) <= W + 0.5:
                    return 'd'
                return None
            each(shade)

    # facial lines for age
    lines = spec.get('lines', 0)
    if lines:
        for (x, y) in [(12, 16), (12, 17), (19, 16), (19, 17)]:
            stamp(g, k, x, y, 'd', only='stds')
        if lines > 1:
            for (x, y) in [(12, 14), (19, 14), (13, 8), (14, 8), (17, 8), (18, 8)]:
                stamp(g, k, x, y, 'd', only='st')

    # nose
    for (x, y) in [(15, 15), (16, 15), (16, 14)]:
        stamp(g, k, x, y, 'd', only='stds')

    # spectacles: silver frame, clear lens, the eyes are drawn over the lens
    if spec.get('glasses'):
        for (x, y, c) in glasses_pixels(spec['glasses']):
            stamp(g, k, x, y, c)
            stamp(g, k, mirror_x(x), y, c)
        for x in (15, 16):
            stamp(g, k, x, 12, 'f')
        for x in (9, 22):
            stamp(g, k, x, 12, 'f', only='stds')
    # brows (hair shade) only over skin
    bc = 'k'
    for (x, y) in BROWS[expression]:
        stamp(g, k, x, y, bc, only='stds')
        stamp(g, k, mirror_x(x), y, bc, only='stds')
    # eyes (a closed smiling eye inside a lens is a flat line, arcs would read as a cross)
    eyes = EYES[expression]
    if spec.get('glasses') and expression == 'pleased':
        eyes = ['.l.', 'ooo']
    for dy, row in enumerate(eyes):
        for dx, c in enumerate(row):
            if c == '.':
                continue
            x = 11 + dx
            stamp(g, k, x, 12 + dy, c)
            stamp(g, k, mirror_x(x), 12 + dy, c)
    if spec.get('lashes') and expression != 'pleased':
        stamp(g, k, 10, 12, 'o', only='stds')
        stamp(g, k, 21, 12, 'o', only='stds')
    # mouth
    lip = 'd'
    if spec.get('beard') in ('beard', 'goatee', 'moustache'):
        hc = HAIRS[spec['hc']]
        light = lum(hc[0]) > 150
        lip = 'o' if light else 't'
        if spec['beard'] == 'moustache':
            lip = 'd'
    for (x, y) in MOUTH[expression]:
        stamp(g, k, x, y, lip)
    return outline(g)


def despeckle(g, k, W, jaw):
    """Drop hair pixels that touch no other hair pixel on a side, so curls do not leave strays."""
    n = len(g)
    kill = []
    for y in range(n):
        for x in range(n):
            if g[y][x] in 'hjk':
                near = sum(1 for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))
                           if 0 <= x + dx < n and 0 <= y + dy < n and g[y + dy][x + dx] in 'hjk')
                if near <= 1:
                    kill.append((x, y))
    for x, y in kill:
        u, v = (x + 0.5) / k, (y + 0.5) / k
        inside = abs(u - 16.0) <= head_hw(v, W, jaw)
        g[y][x] = skin_tone(u, v, W) if inside else '.'


def stamp(g, k, x, y, ch, only=None):
    """Set unit pixel (x, y) (k by k real pixels), optionally only over the given letters."""
    for dy in range(k):
        for dx in range(k):
            yy, xx = y * k + dy, x * k + dx
            if 0 <= yy < len(g) and 0 <= xx < len(g):
                if only is None or g[yy][xx] in only:
                    g[yy][xx] = ch


def outline(g):
    n = len(g)
    out = [row[:] for row in g]
    for y in range(n):
        for x in range(n):
            if g[y][x] != '.':
                continue
            for (dx, dy) in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                yy, xx = y + dy, x + dx
                if 0 <= yy < n and 0 <= xx < n and g[yy][xx] not in '.o':
                    out[y][x] = 'o'
                    break
    return [''.join(r) for r in out]


def lum(hexc):
    c = hexc.lstrip('#')
    r, g, b = (int(c[i:i + 2], 16) for i in (0, 2, 4))
    return 0.299 * r + 0.587 * g + 0.114 * b


# ---- 16 by 16 derivation ----------------------------------------------------------------------

WEIGHT = {'o': 3.1, 'd': 1.0}


def downscale16(rows, palette):
    """Hand-checked rule: each 2 by 2 block of the 32 by 32 bust becomes one pixel.

    1. A block is transparent when it holds no outline pixel and at most one filled pixel.
    2. Otherwise its colour is the highest scoring letter, where each pixel scores its weight:
       outline 3.1 (so one outline pixel always survives and the silhouette keeps its edge), the
       mouth and shade letter d 1.0, every other letter 1.0.
    3. Ties go to the darker colour, so a two pixel brow or lash line stays a line.
    4. A single mouth pixel (the corner of a smile or frown) is kept when its three neighbours are
       skin: a lone d pixel on a skin block beats the skin, and nothing else uses d alone.
    The rule is checked by eye on every sheet and pinned by a test (derivation is deterministic).
    """
    out = []
    for by in range(SMALL):
        line = ''
        for bx in range(SMALL):
            px = [rows[2 * by + dy][2 * bx + dx] for dy in (0, 1) for dx in (0, 1)]
            filled = [p for p in px if p != '.']
            if 'o' not in filled and len(filled) <= 1:
                line += '.'
                continue
            score = {}
            for p in filled:
                score[p] = score.get(p, 0.0) + WEIGHT.get(p, 1.0)
            # mouth corner rule: one d pixel among skin pixels wins
            if filled.count('d') == 1 and len(set(filled) - {'d'}) == 1 and set(filled) - {'d'} <= {'s', 't'}:
                score['d'] = score['d'] + 2.1
            best = max(score.items(), key=lambda kv: (kv[1], -lum(palette[kv[0]])))
            line += best[0]
        out.append(line)
    return out


# ---- People -----------------------------------------------------------------------------------

def P(skin, hair, hc, W, jaw, body, coat, acc, **kw):
    d = dict(skin=skin, hair=hair, hc=hc, W=W, jaw=jaw, body=body, coat=coat, acc=acc)
    d.update(kw)
    return d


SPECS = {
    # texas-1981
    'walter_pruitt': P(1, 'side_part', 'grey', 7, 'square', 'open', 'tan', 'rust', hat='stetson', lines=2),
    'marisol_ybarra': P(3, 'bun', 'black', 6, 'round', 'blouse', 'navy', 'gold', pad=True, pearls=True, lashes=True),
    'desmond_broussard': P(5, 'crop', 'black', 7, 'square', 'tie', 'charcoal', 'red', pad=True, beard='moustache', glasses='square', lines=1),
    'cassandra_rutledge': P(0, 'perm', 'auburn', 7, 'round', 'blouse_round', 'teal', 'cream', pad=True, lashes=True),
    'raymond_delgado': P(3, 'side_part', 'white', 7, 'round', 'tie', 'brown', 'gold', beard='moustache', lines=2),
    'odell_tibbs': P(6, 'crop', 'saltpepper', 8, 'square', 'work', 'olive', 'rust', beard='beard', lines=1),
    'evelyn_hollis': P(1, 'bob', 'blond', 6, 'round', 'scarf', 'tan', 'teal', pad=True, glasses='round', lashes=True, lines=1),
    'anil_deshpande': P(4, 'short', 'black', 6, 'narrow', 'tie', 'steel', 'blue', beard='moustache', glasses='square'),
    'lionel_abrams': P(1, 'horseshoe', 'grey', 7, 'round', 'bow', 'charcoal', 'red', glasses='round', lines=2),
    'teresa_villanueva': P(3, 'wave', 'darkbrown', 6, 'round', 'knit', 'rust', 'cream', lashes=True, lines=1),
    # great-lakes-2000
    'helen_kowalczyk': P(0, 'pixie', 'grey', 7, 'round', 'blouse', 'purple', 'cream', glasses='round', lashes=True, lines=2),
    'kenji_watanabe_moore': P(2, 'short', 'black', 7, 'square', 'tie', 'green', 'orange'),
    'gloria_abernathy': P(6, 'curly_short', 'grey', 7, 'round', 'blouse', 'burgundy', 'cream', pearls=True, glasses='round', lashes=True, lines=2),
    'dennis_halloran': P(1, 'slick', 'saltpepper', 7, 'square', 'open', 'black', 'blue', lines=1),
    'carol_metzger': P(0, 'bob', 'white', 7, 'round', 'blouse', 'teal', 'pink', glasses='square', lashes=True, lines=2),
    'darnell_pierce': P(5, 'buzz', 'black', 8, 'square', 'work', 'steel', 'rust', beard='beard'),
    'adaora_nwosu': P(6, 'afro_round', 'black', 6, 'narrow', 'blouse_round', 'tan', 'teal', lashes=True),
    'daniel_hsieh': P(2, 'mop', 'black', 6, 'narrow', 'knit', 'grey', 'cream', glasses='square'),
    'frank_pietrowski': P(1, 'horseshoe', 'brown', 8, 'square', 'open', 'grey', 'blue', beard='moustache', lines=2),
    'meredith_lahti': P(0, 'long', 'blond', 6, 'narrow', 'turtle', 'green', 'cream', lashes=True, lines=1),
    # silicon-valley-1995
    'priya_raman': P(4, 'long', 'black', 6, 'round', 'blouse', 'teal', 'gold', lashes=True),
    'joel_rosenthal': P(1, 'horseshoe', 'brown', 7, 'round', 'tie', 'navy', 'blue', glasses='round', lines=2),
    'linh_tran': P(2, 'bob', 'black', 6, 'narrow', 'blouse_round', 'rust', 'cream', lashes=True),
    'leonard_hirsch': P(1, 'horseshoe', 'white', 7, 'narrow', 'tie', 'brown', 'green', glasses='round', lines=2),
    'elena_montoya': P(3, 'bob', 'saltpepper', 7, 'round', 'blouse', 'burgundy', 'gold', pearls=True, lashes=True, lines=1),
    'delia_quintanilla': P(3, 'curly_short', 'darkbrown', 7, 'round', 'work', 'olive', 'rust', lashes=True, lines=1),
    'jonah_whitcomb': P(1, 'mop', 'chestnut', 7, 'round', 'blouse_round', 'charcoal', 'teal', beard='goatee', glasses='round'),
    'tamika_greer': P(6, 'bun_high', 'black', 6, 'narrow', 'turtle', 'black', 'gold', lashes=True),
    'judith_aronson': P(1, 'curly_short', 'brown', 7, 'round', 'blouse', 'purple', 'cream', pearls=True, glasses='square', lashes=True, lines=1),
    'darius_rahimi': P(3, 'short', 'black', 7, 'square', 'open', 'steel', 'blue', beard='moustache'),
    # ruhr-rhine-1991
    'ingrid_vossberg': P(0, 'long', 'blond', 7, 'round', 'blouse', 'charcoal', 'red', pearls=True, lashes=True, lines=2),
    'emre_kaplan': P(4, 'short', 'black', 7, 'square', 'tie', 'grey', 'red', beard='moustache'),
    'tobias_wendt': P(1, 'crop', 'white', 7, 'round', 'tie', 'brown', 'blue', glasses='square', lines=2),
    'sabine_lohmeier': P(0, 'wave', 'saltpepper', 6, 'narrow', 'blouse', 'steel', 'teal', glasses='round', lashes=True, lines=1),
    'johannes_rauhut': P(1, 'side_part', 'white', 7, 'narrow', 'tie', 'charcoal', 'burgundy', beard='goatee', glasses='square', lines=2),
    'juergen_zielke': P(2, 'crop', 'brown', 8, 'square', 'work', 'olive', 'rust', beard='moustache', hat='flatcap', lines=1),
    'elisabeth_maurer': P(1, 'bob_fringe', 'auburn', 6, 'narrow', 'scarf', 'black', 'red', glasses='square', lashes=True),
    'marek_dombrowski': P(1, 'side_part', 'chestnut', 7, 'square', 'tie', 'navy', 'gold'),
    'giuseppe_ferrante': P(3, 'slick', 'saltpepper', 7, 'round', 'knit', 'green', 'cream', beard='moustache', lines=2),
    'birgit_nordmann': P(0, 'pixie', 'brown', 7, 'round', 'turtle', 'teal', 'cream', lashes=True, lines=1),
}


def palette_for(spec, rows_list):
    sk = SKINS[spec['skin']]
    hc = HAIRS[spec['hc']]
    co = COATS[spec['coat']]
    full = {
        'o': OUTLINE, 't': sk[0], 's': sk[1], 'd': sk[2],
        'h': hc[0], 'j': hc[1], 'k': hc[2],
        'c': co[0], 'e': co[1], 'a': ACCENTS[spec['acc']],
        'w': WHITE, 'l': LENS, 'f': FRAME,
    }
    used = {ch for rows in rows_list for row in rows for ch in row}
    return {k: v for k, v in full.items() if k in used}


def office_portrait(spec, scenario):
    """64 by 64 framed portrait: the person at twice the resolution on a painted wall."""
    rows = render(spec, 'neutral', k=2)
    n = OFFICE
    g = [list(r) for r in rows]
    wall_l, wall_d, frame_l, frame_d = WALLS[scenario]
    # wall behind the sitter, lighter toward the upper left, then the frame over everything
    for y in range(n):
        for x in range(n):
            if g[y][x] == '.':
                glow = ((x - 18) / 36.0) ** 2 + ((y - 20) / 36.0) ** 2
                g[y][x] = 'y' if glow < 0.45 else 'Y'
    for y in range(n):
        for x in range(n):
            e = min(x, y, n - 1 - x, n - 1 - y)
            if e == 0 or e == 1:
                g[y][x] = 'G'
            elif e == 2:
                g[y][x] = 'g'
            elif e == 3 and g[y][x] in 'yY':
                g[y][x] = 'Y'
    # keep the sitter's own outline against the wall: it is already drawn by render
    return [''.join(r) for r in g]


def build_person(pid, spec, scenario, chief):
    shapes = {e: render(spec, e) for e in EXPRESSIONS}
    pal = palette_for(spec, list(shapes.values()))
    small = {e: downscale16(shapes[e], palette_for(spec, [shapes[e]]) if False else {**{'o': OUTLINE}, **pal}) for e in EXPRESSIONS}
    entry = {'palette': pal, 'expressions': shapes, 'small': small}
    office = None
    if chief:
        orows = office_portrait(spec, scenario)
        wl, wd, fl, fd = WALLS[scenario]
        opal = dict(pal)
        used = {ch for row in orows for ch in row}
        opal.update({'y': wl, 'Y': wd, 'g': fl, 'G': fd})
        opal = {k: v for k, v in opal.items() if k in used}
        office = {'palette': opal, 'rows': orows}
    return entry, office


# ---- Generated extras kit ---------------------------------------------------------------------

FEMALE = {
    'Adaeze', 'Amara', 'Anita', 'Beatrix', 'Bronwen', 'Carmen', 'Dalia', 'Delphine', 'Eleanor',
    'Esperanza', 'Fiona', 'Greta', 'Harriet', 'Ines', 'Joanna', 'Kalinda', 'Leila', 'Lucia',
    'Marguerite', 'Mei', 'Nadia', 'Noor', 'Olga', 'Paloma', 'Renata', 'Rosalind', 'Sigrid', 'Silvia',
    'Tamsin', 'Thandi', 'Valentina', 'Wanjiru', 'Zofia',
}
ERA_BY_PLACE = {
    'Texas': 1981, 'Louisiana': 1981, 'Oklahoma': 1981, 'California': 1995, 'Germany': 1991,
}
MEN_HAIR = ['crop', 'short', 'side_part', 'mop', 'slick', 'wave', 'buzz', 'curly_short', 'horseshoe', 'bald', 'afro_round', 'crop']
WOMEN_HAIR = ['bob', 'bob_fringe', 'long', 'pixie', 'bun', 'bun_high', 'perm', 'curly_short', 'wave', 'afro_round', 'long', 'bob']
ERA_BODY = {
    1981: [('tie', 0.4, True), ('open', 0.3, True), ('bow', 0.1, True), ('work', 0.1, False), ('tie', 0.1, True)],
    1991: [('tie', 0.5, True), ('knit', 0.2, False), ('open', 0.15, True), ('scarf', 0.05, False), ('tie', 0.1, True)],
    1995: [('tie', 0.25, False), ('open', 0.3, False), ('blouse_round', 0.2, False), ('turtle', 0.15, False), ('knit', 0.1, False)],
    2000: [('open', 0.4, False), ('tie', 0.25, False), ('knit', 0.15, False), ('blouse_round', 0.1, False), ('work', 0.1, False)],
}
FEMALE_BODY = {
    1981: ['blouse', 'blouse_round', 'scarf', 'blouse'],
    1991: ['blouse', 'scarf', 'blouse_round', 'turtle'],
    1995: ['blouse_round', 'turtle', 'blouse', 'knit'],
    2000: ['blouse_round', 'blouse', 'turtle', 'knit'],
}


class Lcg:
    """A small fixed generator so extras never change with the Python version."""

    def __init__(self, seed):
        self.state = seed & 0xFFFFFFFFFFFFFFFF

    def next(self):
        self.state = (self.state * 6364136223846793005 + 1442695040888963407) & 0xFFFFFFFFFFFFFFFF
        return self.state >> 33

    def below(self, n):
        return self.next() % n

    def random(self):
        return self.below(10_000) / 10_000

    def choice(self, items):
        return items[self.below(len(items))]

    def weighted(self, items):
        r = self.random()
        acc = 0.0
        for it in items:
            acc += it[1]
            if r < acc:
                return it
        return items[-1]


def extra_spec(rng, name, home, age):
    female = name.split()[0] in FEMALE
    place = home.split(', ')[-1]
    era = ERA_BY_PLACE.get(place, 2000)
    skin = rng.choice([0, 0, 1, 1, 2, 3, 3, 4, 5, 6])
    grey = age >= 56 or (age >= 48 and rng.random() < 0.4)
    colours = ['black', 'darkbrown', 'brown', 'chestnut', 'blond', 'auburn']
    hc = rng.choice(['grey', 'white', 'saltpepper']) if grey else rng.choice(colours)
    if skin >= 5 and not grey:
        hc = rng.choice(['black', 'black', 'darkbrown'])
    hair = rng.choice(WOMEN_HAIR if female else MEN_HAIR)
    if female and age >= 58 and hair in ('long', 'perm', 'bun_high'):
        hair = 'bob'
    body, _, pad = rng.weighted(ERA_BODY[era]) if not female else (rng.choice(FEMALE_BODY[era]), 0, era == 1981)
    spec = dict(
        skin=skin, hair=hair, hc=hc, W=rng.choice([6, 7, 7, 8]), jaw=rng.choice(['round', 'square', 'narrow']),
        body=body, coat=rng.choice(sorted(COATS)), acc=rng.choice(sorted(ACCENTS)), pad=pad or None,
    )
    spec = {k: v for k, v in spec.items() if v is not None}
    if female:
        spec['lashes'] = True
        if body in ('blouse', 'blouse_round') and rng.random() < 0.4:
            spec['pearls'] = True
    elif rng.random() < 0.38:
        spec['beard'] = rng.choice(['moustache', 'moustache', 'goatee', 'beard'])
    if hair == 'bald' and spec.get('beard') is None and rng.random() < 0.5:
        spec['beard'] = 'beard'
    if rng.random() < (0.35 if era != 1981 else 0.45):
        spec['glasses'] = rng.choice(['round', 'square'])
    spec['lines'] = 2 if age >= 58 else (1 if age >= 45 else 0)
    if body == 'work' and not female and rng.random() < 0.5:
        spec['hat'] = 'flatcap'
    if era == 1981 and not female and body == 'open' and rng.random() < 0.25:
        spec['hat'] = 'stetson'
    if spec['hair'] == 'horseshoe' and spec.get('hat'):
        spec['hair'] = 'crop'
    return spec


def extras_sample():
    """First EXTRAS_SAMPLE generated extras from content/cast.json's extras list: (record, spec)."""
    extras = json.loads(OLD_CAST.read_text())['extras']
    rng = Lcg(0x32B17)
    out = []
    for e in extras[:EXTRAS_SAMPLE]:
        out.append((e, extra_spec(rng, e['name'], e['home'], e['age'])))
    return out


# ---- Atlas ------------------------------------------------------------------------------------

def scenario_of(p):
    return p['scenario']


def build():
    roster = json.loads(ROSTER.read_text())
    portraits, office = {}, {}
    for p in roster['people']:
        entry, off = build_person(p['id'], SPECS[p['id']], p['scenario'], p['office_portrait'])
        portraits[p['id']] = entry
        if off:
            office[p['id']] = off
    sample = {}
    for e, spec in extras_sample():
        rows = render(spec, 'neutral')
        sample[e['id']] = {'palette': palette_for(spec, [rows]), 'rows': rows}
    return {
        'size': SIZE,
        'small_size': SMALL,
        'office_size': OFFICE,
        'note': 'PROPOSAL, not referenced by the app yet. Cast portraits for 1.9: 32 by 32 busts with three expressions, '
                'the 16 by 16 derivation, 64 by 64 office portraits for chief executives, and a sample of generated extras. '
                '"." is transparent, each letter maps to that portrait\'s palette. Generated by tools/cast_portraits_v2.py.',
        'portraits': portraits,
        'office': dict(sorted(office.items())),
        'extras_sample': sample,
    }


# ---- Contact sheets ---------------------------------------------------------------------------

BG = (0x13, 0x21, 0x2F)
TEXT = (0xE8, 0xEE, 0xF5)
DIM = (0x93, 0xA4, 0xB8)
GOLD = (0xE2, 0xB0, 0x4A)


def rgb(h):
    h = h.lstrip('#')
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def draw_rows(img, rows, palette, ox, oy, scale):
    for y, row in enumerate(rows):
        for x, ch in enumerate(row):
            if ch == '.':
                continue
            c = rgb(palette[ch])
            for dy in range(scale):
                for dx in range(scale):
                    img.putpixel((ox + x * scale + dx, oy + y * scale + dy), c)


def font(size, bold=False):
    from PIL import ImageFont
    name = 'DejaVuSans-Bold.ttf' if bold else 'DejaVuSans.ttf'
    return ImageFont.truetype('/usr/share/fonts/truetype/dejavu/' + name, size)


def wrap(draw, text, fnt, width):
    words, lines, cur = text.split(), [], ''
    for w in words:
        t = (cur + ' ' + w).strip()
        if draw.textlength(t, font=fnt) <= width or not cur:
            cur = t
        else:
            lines.append(cur)
            cur = w
    if cur:
        lines.append(cur)
    return lines


ROLE_LABEL = {
    'ceo': 'chief executive', 'chair': 'central bank chair candidate', 'governor': 'governor',
    'leader': 'party leader', 'union_leader': 'union leader', 'journalist': 'business journalist',
    'analyst': 'market analyst', 'cfo': 'CFO for hire', 'coo': 'COO for hire', 'logistics': 'logistics chief for hire',
}


def role_text(p):
    return ', '.join(ROLE_LABEL[r] for r in p['roles'])


def roster_sheet(path, scenario, roster, atlas):
    from PIL import Image, ImageDraw
    people = [p for p in roster['people'] if p['scenario'] == scenario]
    scale = 4
    cw = 3 * (SIZE * scale + 12) + SMALL * scale + 40
    ch = SIZE * scale + 118
    cols, rows_n = 2, (len(people) + 1) // 2
    pad = 20
    title_h = 56
    img = Image.new('RGB', (cols * (cw + pad) + pad, title_h + rows_n * (ch + pad) + pad), BG)
    d = ImageDraw.Draw(img)
    d.text((pad, 14), scenario, font=font(26, True), fill=GOLD)
    d.text((pad + 330, 22), 'neutral, pleased, displeased at 4x, then the 16 by 16 neutral. Proposal awaiting approval.', font=font(14), fill=DIM)
    fn, fb, fs = font(16, True), font(13), font(12)
    for i, p in enumerate(people):
        ox = pad + (i % cols) * (cw + pad)
        oy = title_h + (i // cols) * (ch + pad)
        d.rectangle([ox, oy, ox + cw, oy + ch], outline=(0x22, 0x36, 0x4A), width=2, fill=(0x1A, 0x2B, 0x3C))
        e = atlas['portraits'][p['id']]
        x = ox + 14
        for name in EXPRESSIONS:
            draw_rows(img, e['expressions'][name], e['palette'], x, oy + 12, scale)
            x += SIZE * scale + 12
        draw_rows(img, e['small']['neutral'], e['palette'], x + 8, oy + 12 + 32, scale)
        ty = oy + SIZE * scale + 22
        d.text((ox + 14, ty), f"{p['name']}, {p['age']}", font=fn, fill=TEXT)
        d.text((ox + 14, ty + 22), role_text(p), font=fb, fill=GOLD)
        d.text((ox + 14, ty + 40), f"{p['temperament']}, {p['home']}", font=fb, fill=DIM)
        d.text((ox + 14, ty + 58), 'voice: ' + p['voice'], font=fs, fill=DIM)
    img.save(path)


def office_sheet(path, roster, atlas):
    from PIL import Image, ImageDraw
    chiefs = [p for p in roster['people'] if p['office_portrait']]
    scale = 3
    side = OFFICE * scale
    cols = 4
    rows_n = (len(chiefs) + cols - 1) // cols
    pad = 20
    label = 44
    img = Image.new('RGB', (cols * (side + pad) + pad, 50 + rows_n * (side + label + pad) + pad), BG)
    d = ImageDraw.Draw(img)
    d.text((pad, 12), 'Office portraits, 64 by 64 at 3x. Proposal awaiting approval.', font=font(20, True), fill=GOLD)
    for i, p in enumerate(chiefs):
        ox = pad + (i % cols) * (side + pad)
        oy = 50 + (i // cols) * (side + label + pad)
        o = atlas['office'][p['id']]
        draw_rows(img, o['rows'], o['palette'], ox, oy, scale)
        d.text((ox, oy + side + 4), p['name'], font=font(14, True), fill=TEXT)
        d.text((ox, oy + side + 22), f"{p['temperament']}, {p['scenario']}", font=font(12), fill=DIM)
    img.save(path)


def extras_sheet(path, atlas):
    from PIL import Image, ImageDraw
    sample = extras_sample()
    scale = 4
    side = SIZE * scale
    cols = 8
    rows_n = (len(sample) + cols - 1) // cols
    pad = 14
    label = 34
    img = Image.new('RGB', (cols * (side + pad) + pad, 44 + rows_n * (side + label + pad) + pad), BG)
    d = ImageDraw.Draw(img)
    d.text((pad, 10), 'Generated extras kit, 24 samples at 4x. Proposal awaiting approval.', font=font(18, True), fill=GOLD)
    for i, (e, _) in enumerate(sample):
        ox = pad + (i % cols) * (side + pad)
        oy = 44 + (i // cols) * (side + label + pad)
        a = atlas['extras_sample'][e['id']]
        draw_rows(img, a['rows'], a['palette'], ox, oy, scale)
        d.text((ox, oy + side + 3), e['name'].split()[0], font=font(12, True), fill=TEXT)
        d.text((ox, oy + side + 18), f"{e['age']}, {e['home'].split(', ')[-1]}", font=font(11), fill=DIM)
    img.save(path)


def summary_md(path, roster):
    lines = ['# Corporate Warfare 1.9 cast proposal: roster summary', '',
             'Forty fictional people, ten per scenario. Awaiting owner approval. Nothing here is wired into the game.', '']
    for sc in ('great-lakes-2000', 'texas-1981', 'silicon-valley-1995', 'ruhr-rhine-1991'):
        lines += [f'## {sc}', '']
        for p in [q for q in roster['people'] if q['scenario'] == sc]:
            star = ' (existing)' if p['id'] in OLD_IDS else ''
            lines.append(f"- **{p['name']}**, {p['age']}, {p['home']}{star}. {role_text(p)}. {p['temperament']}. Voice: {p['voice']}."
                         + (f" Trait: {p['hire_note']}" if p.get('hire_note') else ''))
        lines.append('')
    Path(path).write_text('\n'.join(lines))


OLD_IDS = {'walter_pruitt', 'marisol_ybarra', 'desmond_broussard', 'helen_kowalczyk', 'kenji_watanabe_moore',
           'gloria_abernathy', 'priya_raman', 'joel_rosenthal', 'linh_tran', 'ingrid_vossberg', 'emre_kaplan', 'tobias_wendt'}


def main(argv):
    atlas = build()
    OUT.write_text(json.dumps(atlas, indent=1) + '\n')
    print(f'{len(atlas["portraits"])} people, {len(atlas["office"])} office portraits -> {OUT}')
    if len(argv) == 3 and argv[1] == '--sheets':
        out = Path(argv[2])
        out.mkdir(parents=True, exist_ok=True)
        roster = json.loads(ROSTER.read_text())
        for sc in ('great-lakes-2000', 'texas-1981', 'silicon-valley-1995', 'ruhr-rhine-1991'):
            roster_sheet(out / f'roster-{sc}.png', sc, roster, atlas)
        office_sheet(out / 'office-portraits.png', roster, atlas)
        extras_sheet(out / 'extras-sample.png', atlas)
        summary_md(out / 'roster-summary.md', roster)
        print('sheets ->', out)


if __name__ == '__main__':
    main(sys.argv)
