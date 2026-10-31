#!/usr/bin/env python3
"""1.9 cast portraits, the source of apple/Resources/cast-portraits.json.

Draws 32 by 32 busts with three expressions (neutral, pleased, displeased) for the forty people and
the generated chiefs in content/cast.json, and a 64 by 64 office portrait for each named chief.
Output is apple/Resources/cast-portraits.json, palette-and-rows format: '.' is transparent and each letter maps to that portrait's own palette.

Every portrait is built from shapes in 32 by 32 units and sampled at the pixel centres, so the 64 by
64 office portrait is the same person at twice the resolution. Light comes from the upper left:
highlights sit top left, shade on the right edge, under the chin and under hair and hats. Only the
eyes, brows and mouth differ between expressions.

Palette letters: o outline; t s d skin light, base, shade (d also draws the mouth); h j k hair base,
highlight, shade (k also draws brows); c e coat base and shade; a accent (tie, blouse, hat band);
w white (shirt and the whites of the eyes); l spectacle lens. Office portraits add y Y g G for the
wall and the frame.

The 16 by 16 busts are derived from the 32 by 32 ones by downscale16(), see its docstring.

    tools/cast_portraits.py                   write the atlas
    tools/cast_portraits.py --sheets DIR      also write the contact sheets and roster summary
    tools/cast_portraits.py --sheets DIR --compare OLD_ATLAS.json   also the pass 1 against pass 2 sheet
"""
import json
import math
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'apple/Resources/cast-portraits.json'
ROSTER = ROOT / 'content/cast.json'

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
    ('#E6B284', '#CF9468', '#94593A'),
    ('#D8A474', '#BA8458', '#7E4D30'),
    ('#C89468', '#AA7A50', '#6E4129'),
    ('#B88258', '#946444', '#5C3724'),
]
GOLD_JEWEL = '#E6C25A'
IRISES = {'dark': '#3A2A22', 'brown': '#6A4A30', 'blue': '#3F6F9F', 'green': '#4C7A4C', 'hazel': '#7A6A34'}


def blend(a, b, t):
    ca, cb = a.lstrip('#'), b.lstrip('#')
    out = [round(int(ca[i:i + 2], 16) * (1 - t) + int(cb[i:i + 2], 16) * t) for i in (0, 2, 4)]
    return '#%02X%02X%02X' % tuple(out)
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

# Head shapes: width change, where the jaw starts, how fast it closes, the narrowest chin half width
# and the lowest row of the chin.
SHAPES = {
    'round': dict(dw=0.0, start=17.5, slope=1.0, floor=3.0, chin=22.0),
    'long': dict(dw=-0.5, start=18.5, slope=0.9, floor=3.0, chin=23.2),
    'square': dict(dw=0.0, start=19.5, slope=1.0, floor=4.0, chin=22.0),
    'narrow': dict(dw=-0.5, start=15.5, slope=0.8, floor=2.5, chin=22.0),
    'broad': dict(dw=1.0, start=18.0, slope=0.8, floor=4.2, chin=22.0),
    'soft': dict(dw=0.0, start=16.5, slope=1.15, floor=3.2, chin=22.0),
    'heart': dict(dw=0.0, start=14.5, slope=1.25, floor=2.0, chin=22.5),
}


def eff_w(spec):
    return spec.get('W', 7) + SHAPES[spec.get('jaw', 'round')]['dw']


def head_hw(v, W, jaw):
    sh = SHAPES[jaw]
    if v < 4.0 or v > sh['chin']:
        return -1.0
    top = min(W, 4.0 + (v - 4.5))
    bottom = W if v <= sh['start'] else max(sh['floor'], W - sh['slope'] * (v - sh['start']))
    return min(top, bottom)


def body_hw(v, pad, cap=15.0):
    if v < 23.0:
        return -1.0
    return min(cap, 4.2 + (4.4 if pad else 3.0) * (v - 23.0))


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
    if H.get('length') and v > 12.0 and abs(du) >= W - H['sb'] - 0.5:
        # long hair: a dark strand edge on the outside and next to the face, lit strands between
        adu = abs(du)
        if adu >= W + H['cw'] - 1.0 or adu < W - H['sb'] + 0.8:
            return 'k'
        if int(adu * 1.0 + (v - 12.0) * 0.18 + (0.0 if du < 0 else 1.0)) % 3 == 0:
            return 'j' if du < 0 else 'h'
        return 'h'
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
    hw = body_hw(v, pad, 13.6 if spec['body'] == 'tie_slim' else 15.0)
    du = u - 16.0
    adu = abs(du)
    if adu > hw:
        return None
    style = spec['body']
    ch = 'c'
    if du > 7.4 + (27.0 - min(v, 27.0)) * 0.25 or (du > 5.0 and v > 29.0):
        ch = 'e'
    t = v - 23.0
    if style in ('tie', 'bow', 'tie_wide', 'tie_slim'):
        # 1981 wide: wide collar spread and lapels, broad tie. 1991 slim: close collar, thin lapel and tie.
        spread, lap = {'tie_wide': (6.9, 2.3), 'tie_slim': (4.5, 0.7)}.get(style, (5.2, 0.9))
        sk = 3.0 - 1.0 * t
        sv = spread - 0.85 * t
        if adu < sk:
            ch = 'd'
        elif adu < sv:
            ch = 'w'
        elif adu < sv + lap and v > 24.0:
            ch = 'e'
        if style != 'bow':
            tw = {'tie_wide': 1.7, 'tie_slim': 0.9}.get(style, 1.0)
            knot = 24.0 <= v < 26.0 and adu <= tw
            blade = v >= 26.0 and adu <= min(tw + 1.0, tw + 0.5 * (v - 26.0))
            if knot or blade:
                ch = 'a'
        else:
            if 24.0 <= v < 26.2 and adu <= 3.4 and adu >= 0.9:
                ch = 'a'
            elif 24.0 <= v < 26.2 and adu < 0.9:
                ch = 'e'
    elif style == 'polo':
        sk = 2.6 - 0.5 * t
        if adu < sk:
            ch = 'd'
        elif 22.8 <= v < 25.4 and sk <= adu < 5.2 - 0.4 * t:
            ch = 'e'
        elif adu < 0.6 and 24.0 <= v < 29.0:
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


def neck_char(u, v, nw=3.0):
    if 19.0 <= v < 26.0 and abs(u - 16.0) <= nw:
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


# Eye, brow and mouth shapes per expression. Eyes are 3 wide for the left eye (columns 11 to 13 plus
# the eye spacing offset, rows 12 and 13) and mirrored for the right. 'o' outline, 'w' white, 's'
# leaves skin, 'l' lens. Brows and mouth are lists of (x, y) pixels; brows are listed for the left
# brow and mirrored, mouths are listed in full. m is the mouth colour, d the skin shade.
EYES = {
    'neutral': ['sss', 'wow'],
    'pleased': ['.o.', 'o.o'],
    'displeased': ['ooo', 'wow'],
}
BROWS = {  # left brow, outer end first; the inner end is the pixel closest to the nose
    'neutral': [(10, 9), (11, 9), (12, 9), (13, 9)],
    'pleased': [(10, 9), (11, 8), (12, 8), (13, 8)],
    'displeased': [(10, 8), (11, 9), (12, 9), (13, 10), (13, 11)],
}
MOUTH = {
    'neutral': [(14, 18), (15, 18), (16, 18), (17, 18)],
    # wide upturned smile: corners up, a lit teeth row, a bright lower lip
    'pleased': [(12, 17), (19, 17), (13, 18), (14, 18), (15, 18), (16, 18), (17, 18), (18, 18)],
    # tight flat mouth with both corners pulled down
    'displeased': [(14, 19), (15, 19), (16, 19), (17, 19), (13, 20), (18, 20)],
}
LENS_ROWS = {
    'round': ['.ooo.', 'o...o', 'o...o', 'o...o', '.ooo.'],
    'square': ['ooooo', 'o...o', 'o...o', 'o...o', 'ooooo'],
}
EYE_OFFSET = {'close': 1, 'mid': 0, 'wide': -1}
NECK_W = {'slim': 2.4, 'mid': 3.0, 'thick': 3.7}
NOSES = {  # shade pixels, highlight pixels
    'button': ([(15, 15), (16, 15)], [(15, 14)]),
    'long': ([(16, 13), (16, 14), (16, 15), (15, 15), (15, 16)], [(15, 13), (15, 14)]),
    'broad': ([(16, 14), (14, 15), (15, 15), (16, 15), (17, 15)], [(15, 14), (15, 13)]),
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


def ear_col(W):
    return math.ceil(15.5 - W - 1e-9) - 1


EAR_ROWS = {'small': (13, 15), 'big': (11, 16), 'lobe': (12, 17), 'flat': None}


def render(spec, expression, k=1, fine=False):
    """One portrait as rows of letters at SIZE * k pixels square. fine skips the face details
    (eyes, brows, nose, mouth, lines, glasses) so the office painter can draw them at full size."""
    n = SIZE * k
    g = new_grid(n)
    jaw = spec.get('jaw', 'round')
    W = eff_w(spec)
    H = HAIR[spec['hair']]
    tr = traits(spec)

    def each(fn):
        for y in range(n):
            v = (y + 0.5) / k
            for x in range(n):
                u = (x + 0.5) / k
                r = fn(u, v)
                if r:
                    g[y][x] = r

    each(lambda u, v: neck_char(u, v, NECK_W[tr['neck']]))
    each(lambda u, v: body_char(spec, u, v))

    def head(u, v):
        hw = head_hw(v, W, jaw)
        if hw < 0:
            return None
        if abs(u - 16.0) <= hw:
            return skin_tone(u, v, W)
        return None

    each(head)
    xl = ear_col(W)
    rows = EAR_ROWS[tr['ears']]
    if rows:
        for y in range(rows[0], rows[1]):
            stamp(g, k, xl, y, 's', only='.')
            stamp(g, k, mirror_x(xl), y, 'd', only='.')
        if tr['ears'] == 'big':
            for y in range(rows[0] + 1, rows[1] - 1):
                stamp(g, k, xl - 1, y, 's', only='.')
                stamp(g, k, mirror_x(xl) + 1, y, 'd', only='.')

    def beard(u, v):
        if beard_mask(spec, W, jaw, u, v):
            du = u - 16.0
            if 15.7 <= v < 17.6 and abs(du) <= 3.6:
                # moustache: a lit upper row, a darker lower row and a skin gap under the nose
                if v >= 17.0:
                    return 'd' if abs(du) < 1.9 else 'k'
                return 'j' if -3.6 <= du < -1.4 else 'h'
            l = -(0.7 * du / W + 0.8 * (v - 17.0) / 6.0)
            if l > 0.55:
                return 'j'
            if l < -0.35:
                return 'k'
            return 'h'
        return None

    each(beard)

    if H is not None:
        each(lambda u, v: hair_tone(H, W, u, v) if hair_mask(H, W, u, v) else None)
        despeckle(g, k, W, jaw)

    if spec.get('hat'):
        each(lambda u, v: hat_char(spec, u, v))
        if spec['hat'] == 'stetson':
            def shade(u, v):
                if g[int(v * k)][int(u * k)] in 'sdt' and v < 9.4 and abs(u - 16.0) <= W + 0.5:
                    return 'd'
                return None
            each(shade)

    # jewellery: studs on the ear lobes, a chain at the collar
    if tr.get('earring'):
        by = (rows[1] if rows else 15)
        for x in (xl, mirror_x(xl)):
            stamp(g, k, x, by, 'n', only='.sdt')
    if tr.get('chain') and spec['body'] in ('blouse', 'blouse_round', 'turtle'):
        for x in range(12, 20):
            y = int(24.6 + 0.1 * (x - 15.5) ** 2)
            stamp(g, k, x, y, 'n', only='dwace')

    if fine:
        return outline(g)

    eo = EYE_OFFSET[tr['eyes']] if not spec.get('glasses') else 0
    # facial lines for age, kept to a few clean marks
    lines = spec.get('lines', 0)
    if lines:
        for (x, y) in [(12, 16), (19, 16)]:
            stamp(g, k, x, y, 'd', only='st')
        if lines > 1:
            for (x, y) in [(12, 17), (19, 17), (14, 7), (17, 7)]:
                stamp(g, k, x, y, 'd', only='st')

    # nose: shade on the right and underneath, light on the bridge
    shade_px, light_px = NOSES[tr['nose']]
    for (x, y) in light_px:
        stamp(g, k, x, y, 't', only='s')
    for (x, y) in shade_px:
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

    # cheeks lifted and lit when pleased
    if expression == 'pleased':
        for x in range(11 + eo, 14 + eo):
            stamp(g, k, x, 14, 't', only='sd')
            stamp(g, k, mirror_x(x), 14, 't', only='sd')
    # brows over skin only: thin ones are drawn in the lighter hair tone, heavy ones add a second row
    brow = BROWS[expression]
    pix = [(x + eo, y) for (x, y) in brow]
    if tr['brow'] == 'heavy':
        pix += [(x + eo, y + 1) for (x, y) in brow[1:3]] if expression != 'displeased' else [(12 + eo, 10)]
    if tr['brow'] == 'thin' and expression != 'displeased':
        pix = pix[1:]
    bc = 'h' if tr['brow'] == 'thin' and expression != 'displeased' else 'k'
    for (x, y) in pix:
        stamp(g, k, x, y, bc, only='stds')
        stamp(g, k, mirror_x(x), y, bc, only='stds')
    if expression == 'displeased':  # shadow line between the brows
        for (x, y) in [(15, 9), (16, 9), (15, 10), (16, 10), (15, 11), (16, 11)]:
            stamp(g, k, x, y, 'd', only='st')
    # eyes (a closed smiling eye inside a lens is a flat line, arcs would read as a cross)
    eyes = EYES[expression]
    if spec.get('glasses') and expression == 'pleased':
        eyes = ['.l.', 'ooo']
    for dy, row in enumerate(eyes):
        for dx, c in enumerate(row):
            if c == '.':
                continue
            x = 11 + eo + dx
            stamp(g, k, x, 12 + dy, c)
            stamp(g, k, mirror_x(x), 12 + dy, c)
    if spec.get('lashes') and not spec.get('glasses'):  # one dark pixel at the outer corner
        stamp(g, k, 10 + eo, 12, 'o', only='stds')
        stamp(g, k, mirror_x(10 + eo), 12, 'o', only='stds')
    # mouth
    lip = 'm'
    if spec.get('beard') in ('beard', 'goatee'):
        hc = HAIRS[spec['hc']]
        lip = 'o' if lum(hc[0]) > 150 else 't'
    def chin(y):  # the pixel row is still on the face, not the neck
        return head_hw(y + 0.5, W, jaw) >= 3.0

    for (x, y) in MOUTH[expression]:
        stamp(g, k, x, y, lip)
    if expression == 'neutral' and tr.get('lips') == 'full':
        for x in (15, 16):
            stamp(g, k, x, 19, 'd', only='st')
    if expression == 'pleased':
        for x in (14, 15, 16, 17):
            stamp(g, k, x, 19, 'w')
        if chin(20):
            for x in (15, 16):
                stamp(g, k, x, 20, 't', only='stds' if lip == 'm' else 'stdsbhjk')
    if expression == 'displeased':
        if chin(20):
            for x in (14, 15, 16, 17):
                stamp(g, k, x, 20, 'd', only='st')
        if chin(21):
            for x in (15, 16):
                stamp(g, k, x, 21, 'd', only='st')
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

WEIGHT = {'o': 3.1}


def downscale16(rows, palette):
    """Hand-checked rule: each 2 by 2 block of the 32 by 32 bust becomes one pixel.

    1. A block is transparent when it holds no outline pixel and at most one filled pixel.
    2. Otherwise its colour is the highest scoring letter, where each pixel scores its weight:
       outline 3.1 (so one outline pixel always survives and the silhouette keeps its edge), every
       other letter 1.0.
    3. Ties go to the darker colour, so a two pixel brow or lash line stays a line.
    4. A single mouth pixel m (the corner of a smile or frown) is kept when its three neighbours are
       skin: a lone m pixel on a skin block beats the skin, and nothing else uses m alone.
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
            # mouth corner rule: one m pixel among skin pixels wins
            if filled.count('m') == 1 and len(set(filled) - {'m'}) == 1 and set(filled) - {'m'} <= {'s', 't', 'd'}:
                score['m'] = score['m'] + 2.1
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


# Head shape per named person: five or more shapes in every scenario, softer jaws for most women.
SHAPE_OF = {
    'walter_pruitt': 'square', 'marisol_ybarra': 'soft', 'desmond_broussard': 'broad', 'cassandra_rutledge': 'heart',
    'raymond_delgado': 'round', 'odell_tibbs': 'broad', 'evelyn_hollis': 'long', 'anil_deshpande': 'narrow',
    'lionel_abrams': 'round', 'teresa_villanueva': 'soft',
    'helen_kowalczyk': 'heart', 'kenji_watanabe_moore': 'square', 'gloria_abernathy': 'round', 'dennis_halloran': 'long',
    'carol_metzger': 'soft', 'darnell_pierce': 'broad', 'adaora_nwosu': 'heart', 'daniel_hsieh': 'narrow',
    'frank_pietrowski': 'broad', 'meredith_lahti': 'long',
    'priya_raman': 'soft', 'joel_rosenthal': 'round', 'linh_tran': 'heart', 'leonard_hirsch': 'narrow',
    'elena_montoya': 'round', 'delia_quintanilla': 'soft', 'jonah_whitcomb': 'long', 'tamika_greer': 'heart',
    'judith_aronson': 'round', 'darius_rahimi': 'square',
    'ingrid_vossberg': 'long', 'emre_kaplan': 'square', 'tobias_wendt': 'round', 'sabine_lohmeier': 'narrow',
    'johannes_rauhut': 'long', 'juergen_zielke': 'broad', 'elisabeth_maurer': 'heart', 'marek_dombrowski': 'square',
    'giuseppe_ferrante': 'round', 'birgit_nordmann': 'soft',
}
for _pid, _shape in SHAPE_OF.items():
    SPECS[_pid]['jaw'] = _shape
    if _shape == 'broad':
        SPECS[_pid]['W'] = 7


def fill_traits(key, spec):
    """Deterministic face traits from a key (a person id): nose, eye spacing, ears, neck, brows and
    jewellery. Explicit values in spec['traits'] win. Returns spec with a complete 'traits' dict."""
    h = zlib.crc32(key.encode())

    def pick(shift, opts):
        return opts[(h >> shift) % len(opts)]

    female = bool(spec.get('lashes'))
    w = eff_w(spec)
    t = dict(
        nose=pick(0, ['button', 'button', 'long', 'broad'] if female else ['long', 'broad', 'button', 'long', 'broad']),
        eyes=pick(4, ['close', 'mid', 'wide', 'mid']),
        ears=pick(8, ['small', 'flat', 'small', 'lobe'] if female else ['big', 'small', 'lobe', 'small', 'big']),
        neck=pick(12, ['slim', 'slim', 'mid'] if female else ['mid', 'thick', 'mid', 'thick']),
        brow=pick(16, ['thin', 'mid', 'thin'] if female else ['mid', 'heavy', 'heavy', 'mid']),
        iris='dark' if spec['skin'] >= 4 else pick(20, ['brown', 'blue', 'green', 'hazel', 'brown', 'blue']),
    )
    if t['eyes'] == 'wide' and w < 7:
        t['eyes'] = 'mid'
    if spec.get('glasses'):
        t['eyes'] = 'mid'
    if not female and spec.get('jaw') in ('broad', 'square') and t['neck'] == 'mid':
        t['neck'] = 'thick'
    if female:
        t['earring'] = pick(24, [True, False, True])
        t['lips'] = 'full'
        t['chain'] = pick(26, [True, False, False]) and not spec.get('pearls')
    t.update(spec.get('traits', {}))
    out = dict(spec)
    out['traits'] = t
    return out


def traits(spec):
    return spec['traits']


for _pid in list(SPECS):
    SPECS[_pid] = fill_traits(_pid, SPECS[_pid])

# Era clothing: 1981 wide lapels and broad ties, 1991 European slim cuts, 1995 open collars in tech,
# 2000 business casual. Applied to the plain 'tie' body of the named people.
ERA_TIE = {'texas-1981': 'tie_wide', 'ruhr-rhine-1991': 'tie_slim', 'silicon-valley-1995': 'open', 'great-lakes-2000': 'polo'}


def era_spec(spec, scenario):
    if spec['body'] == 'tie' and scenario in ERA_TIE:
        spec = dict(spec, body=ERA_TIE[scenario])
    return spec


def palette_all(spec):
    sk = SKINS[spec['skin']]
    hc = HAIRS[spec['hc']]
    co = COATS[spec['coat']]
    full = {
        'o': OUTLINE, 't': sk[0], 's': sk[1], 'd': sk[2],
        'm': blend(sk[2], '#4A1824', 0.6), 'p': blend(sk[1], '#A03A4A', 0.4),
        'h': hc[0], 'j': hc[1], 'k': hc[2],
        'c': co[0], 'e': co[1], 'a': ACCENTS[spec['acc']],
        'w': WHITE, 'l': LENS, 'f': FRAME, 'n': GOLD_JEWEL, 'i': IRISES[traits(spec)['iris']],
    }
    return full


def palette_for(spec, rows_list):
    used = {ch for rows in rows_list for row in rows for ch in row}
    return {k: v for k, v in palette_all(spec).items() if k in used}


# ---- Office portraits (64 by 64, repainted rather than upscaled) -------------------------------

TRADE = {  # what hangs or stands behind each chief executive
    'dennis_halloran': 'skyline', 'gloria_abernathy': 'skyline', 'helen_kowalczyk': 'chimneys',
    'kenji_watanabe_moore': 'chimneys', 'cassandra_rutledge': 'derrick', 'desmond_broussard': 'skyline',
    'marisol_ybarra': 'chimneys', 'walter_pruitt': 'derrick', 'joel_rosenthal': 'skyline',
    'leonard_hirsch': 'circuit', 'linh_tran': 'circuit', 'priya_raman': 'circuit',
    'emre_kaplan': 'chimneys', 'ingrid_vossberg': 'skyline', 'sabine_lohmeier': 'skyline',
    'tobias_wendt': 'headframe',
}
EYE64 = [  # left eye, six wide: whites with a two pixel iris under a dark lid
    '.oooo.',
    'owiiwo',
    '.wiiw.',
    '.dddd.',
]
BROW64 = [(0, 2), (1, 1), (2, 1), (3, 0), (4, 0), (5, 0), (6, 1), (7, 1), (8, 2)]
NOSE64 = {  # shade, light, tip light, base shade, nostrils: lists of (x, y)
    'button': dict(shade=[(33, y) for y in range(28, 32)], light=[(31, y) for y in range(28, 31)],
                   tip=[(31, 31), (32, 31)], base=[(x, 32) for x in range(30, 34)], nostril=[(30, 33), (33, 33)]),
    'long': dict(shade=[(33, y) for y in range(24, 33)], light=[(31, y) for y in range(24, 31)],
                 tip=[(31, 32), (32, 32)], base=[(x, 33) for x in range(30, 34)], nostril=[(30, 34), (33, 34)]),
    'broad': dict(shade=[(33, y) for y in range(26, 32)], light=[(31, y) for y in range(26, 31)],
                  tip=[(31, 31), (32, 31)], base=[(x, 32) for x in range(29, 35)], nostril=[(29, 33), (30, 33), (33, 33), (34, 33)]),
}
SKIN_L = 'st'
SKIN_ALL = 'stds'


def put(g, x, y, ch, only=None):
    if 0 <= y < len(g) and 0 <= x < len(g) and (only is None or g[y][x] in only):
        g[y][x] = ch


def put2(g, x, y, ch, only=None):
    """Put at x and its mirror 63 - x."""
    put(g, x, y, ch, only)
    put(g, 63 - x, y, ch, only)


def paint_face64(g, spec):
    tr = traits(spec)
    eo = 2 * (EYE_OFFSET[tr['eyes']] if not spec.get('glasses') else 0)
    x0 = 22 + eo
    if spec.get('beard') == 'moustache':  # shape the block into a moustache with lighter strands and trimmed ends
        for y in range(31, 37):
            for x in range(24, 40):
                if g[y][x] == 'k' and y >= 33:
                    g[y][x] = 'h'
                if g[y][x] in 'hjk' and y == 35 and (x <= 26 or x >= 37):
                    g[y][x] = 's' if x < 32 else 'd'
    # cheeks and forehead planes first so features sit on top of them
    for y in range(28, 40):
        for x in range(17, 33):
            if ((x - 23) / 4.0) ** 2 + ((y - 33) / 3.0) ** 2 <= 1.0:
                put(g, x, y, 't', 's')
            if ((x - 41) / 4.0) ** 2 + ((y - 35) / 3.5) ** 2 <= 1.0:
                put(g, x, y, 'd', 's')
    for y in range(11, 17):
        for x in range(19, 34):
            if ((x - 26) / 6.0) ** 2 + ((y - 13) / 2.0) ** 2 <= 1.0:
                put(g, x, y, 't', 's')
    if tr['ears'] != 'flat':
        pass
    lines = spec.get('lines', 0)
    if lines:
        for (x, y) in [(27, 34), (26, 35), (26, 36), (26, 37), (26, 38)]:
            put2(g, x, y, 'd', 'st')
        if lines > 1:
            for (x, y) in [(19, 27), (19, 28), (18, 29)]:
                put2(g, x, y, 'd', 'stds')
            for x in range(26, 30):
                put2(g, x, 15, 'd', 'st')
    # nose
    nz = NOSE64[tr['nose']]
    for (x, y) in nz['light']:
        put(g, x, y, 't', 's')
    for (x, y) in nz['shade'] + nz['base']:
        put(g, x, y, 'd', SKIN_ALL)
    for (x, y) in nz['tip']:
        put(g, x, y, 't', 'sd')
    for (x, y) in nz['nostril']:
        put(g, x, y, 'm', SKIN_ALL)
    # glasses
    if spec.get('glasses'):
        rnd = spec['glasses'] == 'round'
        for y in range(22, 32):
            for x in range(20, 30):
                edge = x in (20, 29) or y in (22, 31)
                corner = x in (20, 29) and y in (22, 31)
                if rnd and corner:
                    continue
                if edge:
                    put2(g, x, y, 'f', SKIN_ALL)
                else:
                    put2(g, x, y, 'l', SKIN_ALL)
        for x in (30, 31):
            put2(g, x, 24, 'f')
        put2(g, 19, 24, 'f', SKIN_ALL)
    # brows
    brow = list(BROW64)
    if tr['brow'] == 'thin':
        brow = brow[1:-1]
    colour = 'h' if tr['brow'] == 'thin' else 'k'
    for (dx, dy) in brow:
        put2(g, x0 - 2 + dx, 18 + dy, colour, SKIN_ALL)
    if tr['brow'] == 'heavy':
        for (dx, dy) in brow[2:7]:
            put2(g, x0 - 2 + dx, 19 + dy, 'k', SKIN_ALL)
    # eyes
    ch_iris = 'i'
    for dy, row in enumerate(EYE64):
        for dx, c in enumerate(row):
            if c == '.':
                continue
            if c == 'd':
                put2(g, x0 + dx, 24 + dy, 'd', SKIN_ALL)
            else:
                put2(g, x0 + dx, 24 + dy, 'i' if c == 'i' else c)
    put2(g, x0 + 1, 23, 'd', SKIN_ALL)
    put2(g, x0 + 4, 23, 'd', SKIN_ALL)
    if spec.get('lashes') and not spec.get('glasses'):
        put2(g, x0 - 1, 24, 'o', SKIN_ALL)
        put2(g, x0 - 1, 23, 'o', SKIN_ALL)
        put2(g, x0, 23, 'o', SKIN_ALL)
    # mouth: lips with a dark line between, a lit lower lip, a shadow under it
    for x in range(29, 35):
        put(g, x, 37, 'p', 'stds')
        put(g, x, 39, 'p', 'stds')
    for x in (31, 32):
        put(g, x, 39, 't', 'stdsp')
    for x in range(30, 34):
        put(g, x, 40, 'p', 'stds')
        put(g, x, 41, 'd', 'st')
    for x in range(28, 36):
        put(g, x, 38, 'm', 'stdspjhk')


def paint_hair64(g, spec):
    """Strands in up to three tones: radial on top of the head, vertical below."""
    for y in range(64):
        for x in range(64):
            c = g[y][x]
            if c not in 'hjk':
                continue
            if y < 26:
                key = int(math.atan2(y - 34, x - 32) * 8.0)
            else:
                key = x
            r = noise(x, y)
            if r > 0.82:
                continue
            if c == 'h':
                if key % 4 == 0:
                    g[y][x] = 'j'
                elif key % 7 == 3:
                    g[y][x] = 'k'
            elif c == 'j' and key % 5 == 2:
                g[y][x] = 'h'
            elif c == 'k' and key % 6 == 1 and r > 0:
                g[y][x] = 'h'


def paint_clothes64(g, spec):
    style = spec['body']
    if style in ('tie', 'tie_wide', 'tie_slim', 'bow', 'open'):
        spread, lap = {'tie_wide': (6.9, 2.3), 'tie_slim': (4.5, 0.7)}.get(style, (5.2, 0.9))
        for y in range(48, 64):
            v = (y + 0.5) / 2.0
            t = v - 23.0
            edge = spread - 0.85 * t + lap + 1.1
            if style == 'open':
                edge = 5.8 - 0.75 * t + 0.9 + 1.2
            for x in range(32):
                adu = 16.0 - (x + 0.5) / 2.0
                if abs(adu - edge) < 0.5:
                    put2(g, x, y, 'e', 'c')
        # notch where the lapel meets the collar
        nx = int(2 * (16.0 - (spread - 0.85 * 1.2 + lap + 0.3)))
        put2(g, nx, 49, 'o', 'ce')
    if style in ('tie', 'tie_wide', 'tie_slim'):
        for y in range(52, 64):
            for x in range(26, 38):
                if g[y][x] == 'a' and (x + y) % 4 == 0:
                    g[y][x] = 'e'
        for y in (57, 58):  # pocket square
            for x in (41, 42, 43):
                put(g, x, y, 'w', 'ce')
        put(g, 42, 58, 'a', 'w')
    if style in ('blouse', 'blouse_round', 'scarf'):
        for y in range(48, 64):  # a seam down the front
            put(g, 32, y, 'e', 'a')


def paint_bg64(scenario, trade, pid):
    wall_l, wall_d, _, _ = WALLS[scenario]
    light = blend(wall_l, '#D0D6DC', 0.32)
    dark = blend(wall_d, '#000000', 0.38)
    mid = blend(wall_d, wall_l, 0.55)
    bg = [['y' if ((x - 18) / 36.0) ** 2 + ((y - 20) / 36.0) ** 2 < 0.45 else 'Y' for x in range(64)] for y in range(64)]
    flip = zlib.crc32(pid.encode()) & 1  # which side gets the object

    def px(x, y, ch):
        if flip:
            x = 63 - x
        if 4 <= x < 60 and 4 <= y < 60:
            bg[y][x] = ch

    def rect(x0, y0, x1, y1, ch):
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                px(x, y, ch)

    if trade == 'skyline':
        rect(5, 6, 24, 40, 'q')
        rect(7, 8, 22, 38, 'u')
        for bx, bw, bh in ((7, 4, 14), (11, 3, 22), (14, 5, 17), (19, 4, 26)):
            rect(bx, 38 - bh, bx + bw - 1, 38, 'U')
            for wy in range(38 - bh + 2, 37, 3):
                for wx in range(bx + 1, bx + bw - 1, 2):
                    px(wx, wy, 'u')
        rect(14, 8, 15, 38, 'q')
        rect(7, 22, 22, 22, 'q')
    elif trade == 'derrick':
        for y in range(8, 41):
            half = (y - 8) * 0.42
            px(int(14 - half), y, 'U')
            px(int(14 + half), y, 'U')
            if (y - 8) % 7 == 3:
                for x in range(int(14 - half), int(14 + half) + 1):
                    px(x, y, 'U')
            if (y - 8) % 7 == 0 and y > 8:
                for x in range(int(14 - half) + 1, int(14 + half)):
                    px(x + ((y - 8) // 7) % 2 * 0, y, 'U') if (x + y) % 3 == 0 else None
        rect(12, 5, 16, 7, 'U')
        rect(13, 3, 15, 4, 'U')
        rect(4, 41, 26, 43, 'U')
        rect(5, 38, 8, 40, 'U')
    elif trade == 'chimneys':
        for cx, cw, ch_, band in ((6, 5, 30, 8), (13, 5, 24, 6), (20, 4, 18, 5)):
            top = 42 - ch_
            rect(cx, top, cx + cw - 1, 43, 'U')
            for by in range(top, top + 8, 4):
                rect(cx, by, cx + cw - 1, by + 1, 'u')
        for sx, sy, sr in ((10, 8, 3), (14, 5, 3), (18, 3, 2), (24, 14, 3)):
            for y in range(sy - sr, sy + sr + 1):
                for x in range(sx - sr - 1, sx + sr + 2):
                    if ((x - sx) / (sr + 1.0)) ** 2 + ((y - sy) / float(sr)) ** 2 <= 1.0:
                        px(x, y, 'u')
        rect(4, 42, 28, 47, 'U')
        for sx in range(5, 28, 6):  # saw tooth roof
            for i in range(5):
                px(sx + i, 41 - i // 2, 'U')
    elif trade == 'circuit':
        rect(5, 7, 25, 41, 'q')
        rect(7, 9, 23, 39, 'U')
        for y, x0_, x1_ in ((12, 8, 19), (17, 12, 22), (30, 8, 16), (35, 10, 22)):
            for x in range(x0_, x1_ + 1):
                px(x, y, 'u')
        for x, y0_, y1_ in ((19, 12, 17), (12, 17, 30), (22, 17, 24), (16, 30, 35)):
            for y in range(y0_, y1_ + 1):
                px(x, y, 'u')
        rect(11, 21, 18, 27, 'q')
        rect(13, 23, 16, 25, 'U')
        for y in range(20, 29, 2):
            px(10, y, 'u')
            px(19, y, 'u')
        for (x, y) in ((8, 12), (22, 17), (8, 30), (22, 35), (20, 11)):
            rect(x, y, x + 1, y + 1, 'q')
    elif trade == 'headframe':
        for y in range(10, 44):
            half = (y - 10) * 0.28
            px(int(15 - half), y, 'U')
            px(int(15 + half), y, 'U')
            if (y - 10) % 8 == 5:
                for x in range(int(15 - half), int(15 + half) + 1):
                    px(x, y, 'U')
        for y in range(4, 16):
            for x in range(8, 23):
                d = ((x - 15) ** 2 + (y - 9) ** 2) ** 0.5
                if 5.0 <= d <= 6.2 or d <= 1.2:
                    px(x, y, 'U')
        for i in range(12):
            px(15 + i, 9 + i * 2 // 1 if 9 + i * 2 < 44 else 43, 'U')
        rect(4, 44, 26, 48, 'U')
    pal = {'u': light, 'U': dark, 'q': mid}
    return bg, pal


def office_portrait(spec, scenario, pid):
    """64 by 64 framed portrait, repainted at full size: finer eyes, nose and cheek planes, hair
    strands in three tones, lapel and collar detail, and an object that hints at the trade."""
    n = OFFICE
    g = [list(r) for r in render(spec, 'neutral', k=2, fine=True)]
    paint_face64(g, spec)
    paint_hair64(g, spec)
    paint_clothes64(g, spec)
    bg, extra_pal = paint_bg64(scenario, TRADE[pid], pid)
    for y in range(n):
        for x in range(n):
            if g[y][x] == '.':
                g[y][x] = bg[y][x]
    for y in range(n):
        for x in range(n):
            e = min(x, y, n - 1 - x, n - 1 - y)
            if e == 0 or e == 1:
                g[y][x] = 'G'
            elif e == 2:
                g[y][x] = 'g'
            elif e == 3 and g[y][x] in 'yY':
                g[y][x] = 'Y'
    return [''.join(r) for r in g], extra_pal


def build_person(pid, spec, scenario, chief):
    spec = era_spec(spec, scenario)
    shapes = {e: render(spec, e) for e in EXPRESSIONS}
    pal = palette_for(spec, list(shapes.values()))
    small = {e: downscale16(shapes[e], pal) for e in EXPRESSIONS}
    entry = {'palette': pal, 'expressions': shapes, 'small': small}
    office = None
    if chief:
        orows, extra_pal = office_portrait(spec, scenario, pid)
        wl, wd, fl, fd = WALLS[scenario]
        opal = palette_all(spec)
        opal.update({'y': wl, 'Y': wd, 'g': fl, 'G': fd})
        opal.update(extra_pal)
        used = {ch for row in orows for ch in row}
        office = {'palette': {k: v for k, v in opal.items() if k in used}, 'rows': orows}
    return entry, office


GIVEN = [
    'Adaeze', 'Alistair', 'Amara', 'Anders', 'Anita', 'Armando', 'Beatrix', 'Benedikt', 'Bronwen',
    'Callum', 'Carmen', 'Cedric', 'Chidi', 'Dalia', 'Dario', 'Delphine', 'Dmitri', 'Eleanor',
    'Emeka', 'Esperanza', 'Farid', 'Felix', 'Fiona', 'Gideon', 'Greta', 'Hamid', 'Harriet',
    'Hiroshi', 'Ines', 'Isaac', 'Jasper', 'Joanna', 'Jun', 'Kalinda', 'Karl', 'Lars', 'Leila',
    'Lorenzo', 'Lucia', 'Magnus', 'Marguerite', 'Mateus', 'Mei', 'Nadia', 'Nikolai', 'Noor',
    'Oliver', 'Olga', 'Omar', 'Paloma', 'Patrick', 'Quentin', 'Rafael', 'Renata', 'Rohan',
    'Rosalind', 'Samir', 'Sigrid', 'Silvia', 'Stellan', 'Tamsin', 'Teodor', 'Thandi', 'Ulrich',
    'Valentina', 'Viktor', 'Wanjiru', 'Xavier', 'Yusuf', 'Zofia',
]
FAMILY = [
    'Abara', 'Achterberg', 'Aldana', 'Andersen', 'Baptiste', 'Bellamy', 'Brandvold', 'Calloway',
    'Carvalho', 'Castellano', 'Chaudhry', 'Dellacroce', 'Donnelly', 'Eriksson', 'Faulkner',
    'Fitzgerald', 'Gallagher', 'Grabowski', 'Haddad', 'Halvorsen', 'Hargreaves', 'Ibarra',
    'Jankowski', 'Kasprzak', 'Kowalski', 'Lindqvist', 'Lombardi', 'Maldonado', 'Marchetti',
    'Mbeki', 'Montgomery', 'Nakagawa', 'Novak', 'Okafor', 'Oyelaran', 'Pellegrini', 'Pemberton',
    'Quinlan', 'Radcliffe', 'Rasmussen', 'Salazar', 'Sandoval', 'Schreiber', 'Sokolov',
    'Stanford', 'Takahashi', 'Thibodeaux', 'Underhill', 'Valdez', 'Vandenberg', 'Wallenberg',
    'Whitlock', 'Yamamoto', 'Zielinski',
]


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
ERA_BODY = {  # (body, weight, shoulder pads): wide lapels 1981, slim European cuts 1991, open collars 1995, casual 2000
    1981: [('tie_wide', 0.5, True), ('open', 0.2, True), ('bow', 0.1, True), ('work', 0.1, False), ('tie_wide', 0.1, True)],
    1991: [('tie_slim', 0.55, False), ('knit', 0.2, False), ('open', 0.1, False), ('scarf', 0.05, False), ('tie_slim', 0.1, False)],
    1995: [('open', 0.35, False), ('polo', 0.25, False), ('turtle', 0.15, False), ('tie_slim', 0.1, False), ('knit', 0.15, False)],
    2000: [('open', 0.3, False), ('polo', 0.3, False), ('tie', 0.15, False), ('knit', 0.15, False), ('work', 0.1, False)],
}
FEMALE_BODY = {
    1981: ['blouse', 'blouse_round', 'scarf', 'blouse'],
    1991: ['blouse', 'scarf', 'blouse_round', 'turtle'],
    1995: ['blouse_round', 'turtle', 'blouse', 'knit'],
    2000: ['blouse_round', 'polo', 'turtle', 'knit'],
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


def extra_spec(rng, key, name, home, age):
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
        skin=skin, hair=hair, hc=hc, W=rng.choice([6, 7, 7, 8]), jaw=rng.choice(['round', 'square', 'narrow', 'long', 'broad'] if not female else ['soft', 'heart', 'round', 'narrow', 'long']),
        body=body, coat=rng.choice(sorted(COATS)), acc=rng.choice(sorted(ACCENTS)), pad=pad or None,
    )
    spec = {k: v for k, v in spec.items() if v is not None}
    if female:
        spec['lashes'] = True
        if body in ('blouse', 'blouse_round', 'polo') and rng.random() < 0.4:
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
    if spec['jaw'] == 'broad':
        spec['W'] = 7
    return fill_traits(key, spec)


def extras_specs():
    """Every generated chief executive in content/cast.json: (record, spec). The face follows the
    person: the first name decides the presentation (see FEMALE) and the age decides grey hair and
    lines, so a name never sits on an obviously mismatched face."""
    extras = json.loads(ROSTER.read_text())['extras']
    rng = Lcg(0x32B17)
    return [(e, extra_spec(rng, e['id'], e['name'], e['home'], e['age'])) for e in extras]


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
    for e, spec in extras_specs():
        shapes = {x: render(spec, x) for x in EXPRESSIONS}
        pal = palette_for(spec, list(shapes.values()))
        portraits[e['id']] = {'palette': pal, 'expressions': shapes,
                              'small': {x: downscale16(shapes[x], pal) for x in EXPRESSIONS}}
    return {
        'size': SIZE,
        'small_size': SMALL,
        'office_size': OFFICE,
        'note': 'Cast portraits: 32 by 32 busts with three expressions and their 16 by 16 derivation for the named people '
                'and every generated chief executive, and 64 by 64 office portraits for the chiefs. '
                '"." is transparent, each letter maps to that portrait\'s palette. Generated by tools/cast_portraits.py.',
        'portraits': portraits,
        'office': dict(sorted(office.items())),
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
    d.text((pad + 330, 22), 'neutral, pleased, displeased at 4x, then the 16 by 16 neutral.', font=font(14), fill=DIM)
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
    d.text((pad, 12), 'Office portraits, 64 by 64 at 3x.', font=font(20, True), fill=GOLD)
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
    sample = extras_specs()[:EXTRAS_SAMPLE]
    scale = 4
    side = SIZE * scale
    cols = 8
    rows_n = (len(sample) + cols - 1) // cols
    pad = 14
    label = 34
    img = Image.new('RGB', (cols * (side + pad) + pad, 44 + rows_n * (side + label + pad) + pad), BG)
    d = ImageDraw.Draw(img)
    d.text((pad, 10), 'Generated chief executives, the first 24 of 96 at 4x.', font=font(18, True), fill=GOLD)
    for i, (e, _) in enumerate(sample):
        ox = pad + (i % cols) * (side + pad)
        oy = 44 + (i // cols) * (side + label + pad)
        a = atlas['portraits'][e['id']]
        draw_rows(img, a['expressions']['neutral'], a['palette'], ox, oy, scale)
        d.text((ox, oy + side + 3), e['name'].split()[0], font=font(12, True), fill=TEXT)
        d.text((ox, oy + side + 18), f"{e['age']}, {e['home'].split(', ')[-1]}", font=font(11), fill=DIM)
    img.save(path)


COMPARE = ['walter_pruitt', 'cassandra_rutledge', 'desmond_broussard', 'odell_tibbs', 'priya_raman', 'ingrid_vossberg']


def compare_sheet(path, before, atlas, roster):
    """Six people, neutral, pleased and displeased at 6x: pass 1 on the left, pass 2 on the right."""
    from PIL import Image, ImageDraw
    scale = 6
    side = SIZE * scale
    gap, pad = 8, 20
    block = 3 * side + 2 * gap
    names = {p['id']: p['name'] for p in roster['people']}
    img = Image.new('RGB', (2 * block + 3 * pad, 60 + len(COMPARE) * (side + 28 + pad) + pad), BG)
    d = ImageDraw.Draw(img)
    d.text((pad, 12), 'Pass 1 (left) against pass 2 (right): neutral, pleased, displeased at 6x', font=font(20, True), fill=GOLD)
    for r, pid in enumerate(COMPARE):
        oy = 50 + r * (side + 28 + pad)
        d.text((pad, oy), names[pid], font=font(14, True), fill=TEXT)
        for col, src in enumerate((before, atlas)):
            e = src['portraits'][pid]
            x = pad + col * (block + pad)
            for name in EXPRESSIONS:
                draw_rows(img, e['expressions'][name], e['palette'], x, oy + 22, scale)
                x += side + gap
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
    OUT.write_text(json.dumps(atlas, separators=(',', ':')) + '\n')
    print(f'{len(atlas["portraits"])} people, {len(atlas["office"])} office portraits -> {OUT}')
    if len(argv) >= 3 and argv[1] == '--sheets':
        out = Path(argv[2])
        out.mkdir(parents=True, exist_ok=True)
        roster = json.loads(ROSTER.read_text())
        for sc in ('great-lakes-2000', 'texas-1981', 'silicon-valley-1995', 'ruhr-rhine-1991'):
            roster_sheet(out / f'roster-{sc}.png', sc, roster, atlas)
        office_sheet(out / 'office-portraits.png', roster, atlas)
        extras_sheet(out / 'extras-sample.png', atlas)
        summary_md(out / 'roster-summary.md', roster)
        if len(argv) == 5 and argv[3] == '--compare':
            compare_sheet(out / 'before-after-expressions.png', json.loads(Path(argv[4]).read_text()), atlas, roster)
        print('sheets ->', out)


if __name__ == '__main__':
    main(sys.argv)
