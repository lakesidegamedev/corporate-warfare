#!/usr/bin/env python3
"""Source of the generated company logos (content/logos.json).

Every firm gets a 16 by 16 eight-bit logo chosen from its name alone: a plate shape, a colour
set and one symbol (a shape, star, arrow, wheat ear, drop, gear, tower, wave and so on) or a
two-letter monogram. The Rust engine composes logos at run time from content/logos.json with the
same steps as `compose()` here, so a firm looks identical on Compose and SwiftUI, and
tools/test_logo_gen.py pins a few results that the Rust tests pin too.

Symbols are drawn only from a short list of plain shapes. `--check` refuses any symbol (under
every rotation and mirror) that resembles a banned shape: crosses, hooked crosses, crescents,
the six-pointed star and four-fold pinwheels. Nothing religious, political, military or hate
related is in the library, and none of it is drawn to resemble a real brand.

Usage: logo_gen.py            regenerate content/logos.json
       logo_gen.py --check    run the banned-shape check
       logo_gen.py --sheet out.png   write a contact sheet (needs Pillow)
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'content/logos.json'
SIZE = 16
SYM = 10
MASK64 = (1 << 64) - 1

SYMBOLS = {
    'star': [
        '....##....',
        '....##....',
        '...####...',
        '##########',
        '.########.',
        '..######..',
        '..######..',
        '.####.###.',
        '.###...##.',
        '.##.....#.',
    ],
    'arrow_up': [
        '....##....',
        '...####...',
        '..######..',
        '.########.',
        '##########',
        '...####...',
        '...####...',
        '...####...',
        '...####...',
        '...####...',
    ],
    'wheat': [
        '....##....',
        '.#..##..#.',
        '..#.##.#..',
        '.#.####.#.',
        '..#.##.#..',
        '.#..##..#.',
        '..#.##.#..',
        '....##....',
        '....##....',
        '....##....',
    ],
    'drop': [
        '....##....',
        '....##....',
        '...####...',
        '...####...',
        '..######..',
        '.########.',
        '.########.',
        '.########.',
        '..######..',
        '...####...',
    ],
    'gear': [
        '...####...',
        '.#.####.#.',
        '.########.',
        '####..####',
        '####..####',
        '####..####',
        '####..####',
        '.########.',
        '.#.####.#.',
        '...####...',
    ],
    'tower': [
        '....##....',
        '...####...',
        '...#..#...',
        '...####...',
        '...#..#...',
        '...####...',
        '..######..',
        '..#.##.#..',
        '..#.##.#..',
        '..######..',
    ],
    'wave': [
        '..........',
        '.###..###.',
        '#...##...#',
        '..........',
        '.###..###.',
        '#...##...#',
        '..........',
        '.###..###.',
        '#...##...#',
        '..........',
    ],
    'mountain': [
        '..........',
        '....##....',
        '...####...',
        '...####...',
        '..######..',
        '.########.',
        '.########.',
        '##########',
        '##########',
        '##########',
    ],
    'leaf': [
        '......###.',
        '....#####.',
        '...######.',
        '..#######.',
        '.########.',
        '.########.',
        '.#######..',
        '##.####...',
        '#..##.....',
        '...#......',
    ],
    'bolt': [
        '.....###..',
        '....###...',
        '...###....',
        '..###.....',
        '.########.',
        '....###...',
        '...###....',
        '..###.....',
        '..##......',
        '.#........',
    ],
    'ring': [
        '..######..',
        '.########.',
        '###....###',
        '##......##',
        '##......##',
        '##......##',
        '##......##',
        '###....###',
        '.########.',
        '..######..',
    ],
    'flame': [
        '....#.....',
        '....##....',
        '...###.#..',
        '..####.##.',
        '..#######.',
        '.#########',
        '.#########',
        '.#########',
        '..#######.',
        '...#####..',
    ],
    'anchor': [
        '....##....',
        '...#..#...',
        '....##....',
        '..######..',
        '....##....',
        '....##....',
        '#...##...#',
        '##..##..##',
        '.########.',
        '..######..',
    ],
    'hex_nut': [
        '..######..',
        '.########.',
        '####..####',
        '###....###',
        '###....###',
        '###....###',
        '###....###',
        '####..####',
        '.########.',
        '..######..',
    ],
    'bars': [
        '.......##.',
        '.......##.',
        '...##..##.',
        '...##..##.',
        '.#.##..##.',
        '.#.##..##.',
        '.#.##..##.',
        '.#.##..##.',
        '##########',
        '##########',
    ],
    'arch': [
        '..######..',
        '.########.',
        '###....###',
        '##......##',
        '##......##',
        '#........#',
        '#...##...#',
        '#...##...#',
        '##########',
        '##########',
    ],
    'diamond': [
        '....##....',
        '...####...',
        '..######..',
        '.########.',
        '##########',
        '.########.',
        '..######..',
        '...####...',
        '....##....',
        '..........',
    ],
    'pine': [
        '....##....',
        '...####...',
        '..######..',
        '...####...',
        '..######..',
        '.########.',
        '##########',
        '....##....',
        '....##....',
        '....##....',
    ],
}

# The standard 5 by 7 capital letters, for monograms.
FONT = {
    'A': ['.###.', '#...#', '#...#', '#####', '#...#', '#...#', '#...#'],
    'B': ['####.', '#...#', '#...#', '####.', '#...#', '#...#', '####.'],
    'C': ['.###.', '#...#', '#....', '#....', '#....', '#...#', '.###.'],
    'D': ['####.', '#...#', '#...#', '#...#', '#...#', '#...#', '####.'],
    'E': ['#####', '#....', '#....', '####.', '#....', '#....', '#####'],
    'F': ['#####', '#....', '#....', '####.', '#....', '#....', '#....'],
    'G': ['.###.', '#...#', '#....', '#.###', '#...#', '#...#', '.###.'],
    'H': ['#...#', '#...#', '#...#', '#####', '#...#', '#...#', '#...#'],
    'I': ['#####', '..#..', '..#..', '..#..', '..#..', '..#..', '#####'],
    'J': ['..###', '...#.', '...#.', '...#.', '...#.', '#..#.', '.##..'],
    'K': ['#...#', '#..#.', '#.#..', '##...', '#.#..', '#..#.', '#...#'],
    'L': ['#....', '#....', '#....', '#....', '#....', '#....', '#####'],
    'M': ['#...#', '##.##', '#.#.#', '#.#.#', '#...#', '#...#', '#...#'],
    'N': ['#...#', '##..#', '#.#.#', '#..##', '#...#', '#...#', '#...#'],
    'O': ['.###.', '#...#', '#...#', '#...#', '#...#', '#...#', '.###.'],
    'P': ['####.', '#...#', '#...#', '####.', '#....', '#....', '#....'],
    'Q': ['.###.', '#...#', '#...#', '#...#', '#.#.#', '#..#.', '.##.#'],
    'R': ['####.', '#...#', '#...#', '####.', '#.#..', '#..#.', '#...#'],
    'S': ['.####', '#....', '#....', '.###.', '....#', '....#', '####.'],
    'T': ['#####', '..#..', '..#..', '..#..', '..#..', '..#..', '..#..'],
    'U': ['#...#', '#...#', '#...#', '#...#', '#...#', '#...#', '.###.'],
    'V': ['#...#', '#...#', '#...#', '#...#', '#...#', '.#.#.', '..#..'],
    'W': ['#...#', '#...#', '#...#', '#.#.#', '#.#.#', '##.##', '#...#'],
    'X': ['#...#', '#...#', '.#.#.', '..#..', '.#.#.', '#...#', '#...#'],
    'Y': ['#...#', '#...#', '.#.#.', '..#..', '..#..', '..#..', '..#..'],
    'Z': ['#####', '....#', '...#.', '..#..', '.#...', '#....', '#####'],
}

# Letter pairs that read badly as a monogram; those firms get one centred initial instead.
BANNED_INITIALS = {'SS', 'KK', 'NS', 'HH', 'SA'}

# plate, edge, symbol
PALETTES = [
    ('#1F3A5F', '#0F2038', '#F2C14E'),
    ('#2D5A3D', '#183323', '#F0E6C8'),
    ('#1E7F86', '#0F4A4F', '#FFFFFF'),
    ('#7A2E3A', '#47171F', '#F1DFC0'),
    ('#3C4658', '#20262F', '#F6A93B'),
    ('#3F3D8F', '#232257', '#BDF0D8'),
    ('#2B2B30', '#131316', '#FF8A3D'),
    ('#6B4423', '#3A2412', '#FFD76A'),
    ('#5B7C99', '#324A5E', '#FFFFFF'),
    ('#5E2F6B', '#351840', '#FFC9A8'),
    ('#6B7A2A', '#3B4515', '#F5F0D0'),
    ('#14507A', '#0A2E47', '#B6F26B'),
    ('#E3C98A', '#A68B4B', '#3A2E1A'),
    ('#E8EEF4', '#8A9AAE', '#1F3A5F'),
]

PLATES = ['rounded', 'circle', 'octagon', 'badge']

# Shapes the symbol library must never resemble. '#' is ink. Each is checked under all eight
# rotations and mirrors, as an overlap ratio against the symbol.
BANNED = {
    'plus_cross': [
        '....##....', '....##....', '....##....', '....##....', '##########',
        '##########', '....##....', '....##....', '....##....', '....##....',
    ],
    'latin_cross': [
        '....##....', '....##....', '..######..', '..######..', '....##....',
        '....##....', '....##....', '....##....', '....##....', '....##....',
    ],
    'crescent': [
        '..####....', '.###......', '###.......', '###.......', '###.......',
        '###.......', '###.......', '.###......', '..####....', '..........',
    ],
    'hexagram': [
        '....##....', '...####...', '##########', '.########.', '..######..',
        '..######..', '.########.', '##########', '...####...', '....##....',
    ],
}


def fnv1a(data):
    h = 0xcbf29ce484222325
    for b in data:
        h ^= b
        h = (h * 0x100000001b3) & MASK64
    return h


class Stream:
    def __init__(self, name):
        self.state = fnv1a(name.strip().lower().encode('utf-8'))

    def next(self):
        self.state = (self.state + 0x9E3779B97F4A7C15) & MASK64
        z = self.state
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK64
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK64
        return z ^ (z >> 31)


def plate_mask(kind):
    """16 rows of '.', 'p' (plate) and 'e' (edge ring)."""
    inside = [[False] * SIZE for _ in range(SIZE)]
    for y in range(SIZE):
        for x in range(SIZE):
            if kind == 'rounded':
                corner = min(x, SIZE - 1 - x) + min(y, SIZE - 1 - y)
                ok = corner >= 2
            elif kind == 'circle':
                dx, dy = 2 * x + 1 - SIZE, 2 * y + 1 - SIZE
                ok = dx * dx + dy * dy <= 15 * 15
            elif kind == 'octagon':
                corner = min(x, SIZE - 1 - x) + min(y, SIZE - 1 - y)
                ok = corner >= 4
            else:  # badge: a square with one pixel cut off each corner and a one pixel margin
                ok = 1 <= x <= SIZE - 2 and 1 <= y <= SIZE - 2 and not (
                    min(x - 1, SIZE - 2 - x) + min(y - 1, SIZE - 2 - y) < 1)
            inside[y][x] = ok
    rows = []
    for y in range(SIZE):
        row = ''
        for x in range(SIZE):
            if not inside[y][x]:
                row += '.'
                continue
            edge = any(
                not (0 <= x + dx < SIZE and 0 <= y + dy < SIZE) or not inside[y + dy][x + dx]
                for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)))
            row += 'e' if edge else 'p'
        rows.append(row)
    return rows


def rotate(rows):
    return [''.join(rows[len(rows) - 1 - r][c] for r in range(len(rows))) for c in range(len(rows[0]))]


def mirror(rows):
    return [row[::-1] for row in rows]


def orientations(rows):
    out = []
    cur = rows
    for _ in range(4):
        out.append(cur)
        out.append(mirror(cur))
        cur = rotate(cur)
    return out


def overlap(a, b):
    """Intersection over union of the ink of two equal-sized bitmaps."""
    inter = union = 0
    for ra, rb in zip(a, b):
        for ca, cb in zip(ra, rb):
            inter += ca == '#' and cb == '#'
            union += ca == '#' or cb == '#'
    return inter / union if union else 0.0


def library():
    symbols = dict(SYMBOLS)
    symbols['arrow_right'] = rotate(SYMBOLS['arrow_up'])
    return dict(sorted(symbols.items()))


def banned_matches(rows, threshold=0.7):
    """Names of banned shapes a 10 by 10 bitmap resembles, or a four-fold pinwheel."""
    hits = []
    for name, shape in BANNED.items():
        for variant in orientations(shape):
            if overlap(rows, variant) >= threshold:
                hits.append(name)
                break
    # A hooked cross is a pinwheel: it maps onto itself by a quarter turn but not by a mirror.
    turned = rotate(rows)
    if turned == rows and mirror(rows) != rows:
        hits.append('pinwheel')
    return hits


def check_library():
    errors = []
    for name, rows in library().items():
        if len(rows) != SYM or any(len(r) != SYM for r in rows):
            errors.append(f'{name}: not {SYM} by {SYM}')
            continue
        for hit in banned_matches(rows):
            errors.append(f'{name}: resembles banned shape {hit}')
    for letter, rows in FONT.items():
        if len(rows) != 7 or any(len(r) != 5 for r in rows):
            errors.append(f'font {letter}: not 5 by 7')
    return errors


def build():
    return {
        'size': SIZE,
        'note': 'Generated company logo parts, shared by every platform. Compose a logo from a firm name with the steps in tools/logo_gen.py compose(); the Rust engine does the same. Generated by tools/logo_gen.py.',
        'plates': {kind: plate_mask(kind) for kind in PLATES},
        'plate_order': PLATES,
        'palettes': [{'p': p, 'e': e, 's': s} for p, e, s in PALETTES],
        'symbols': library(),
        'font': FONT,
        'banned_initials': sorted(BANNED_INITIALS),
    }


def initials(name):
    words = [''.join(ch for ch in w.upper() if 'A' <= ch <= 'Z') for w in name.split()]
    words = [w for w in words if w]
    if not words:
        return 'O'
    if len(words) >= 2:
        pair = words[0][0] + words[1][0]
    else:
        pair = words[0][:2]
    return pair if pair not in BANNED_INITIALS and len(pair) == 2 else pair[0]


def compose(name, parts=None):
    """The logo of a firm: `(palette, rows)` with '.' transparent, 'p' plate, 'e' edge, 's' symbol."""
    parts = parts or build()
    stream = Stream(name)
    plate = parts['plate_order'][stream.next() % len(parts['plate_order'])]
    colours = parts['palettes'][stream.next() % len(parts['palettes'])]
    monogram = stream.next() % 100 < 22
    names = sorted(parts['symbols'])
    symbol = names[stream.next() % len(names)]
    grid = [list(row) for row in parts['plates'][plate]]
    if monogram:
        text = initials(name)
        glyphs = [parts['font'][ch] for ch in text]
        width = 5 * len(glyphs) + (len(glyphs) - 1)
        bitmap = []
        for y in range(7):
            row = ''
            for index, glyph in enumerate(glyphs):
                row += ('.' if index else '') + glyph[y]
            bitmap.append(row)
    else:
        bitmap = parts['symbols'][symbol]
        width = SYM
    ox = (SIZE - width) // 2
    oy = (SIZE - len(bitmap)) // 2
    for y, row in enumerate(bitmap):
        for x, ch in enumerate(row):
            if ch == '#':
                grid[oy + y][ox + x] = 's'
    rows = [''.join(r) for r in grid]
    return {'p': colours['p'], 'e': colours['e'], 's': colours['s']}, rows


def rows_digest(rows):
    return format(fnv1a(''.join(rows).encode('ascii')), '016x')


def sheet(path, names, scale=8, columns=8):
    from PIL import Image
    parts = build()
    pad = 6
    side = SIZE * scale
    count = len(names)
    lines = (count + columns - 1) // columns
    img = Image.new('RGB', (columns * (side + pad) + pad, lines * (side + pad) + pad), (30, 30, 40))
    for i, name in enumerate(names):
        palette, rows = compose(name, parts)
        ox = pad + (i % columns) * (side + pad)
        oy = pad + (i // columns) * (side + pad)
        for y, row in enumerate(rows):
            for x, ch in enumerate(row):
                if ch == '.':
                    continue
                c = palette[ch].lstrip('#')
                rgb = tuple(int(c[j:j + 2], 16) for j in (0, 2, 4))
                for dy in range(scale):
                    for dx in range(scale):
                        img.putpixel((ox + x * scale + dx, oy + y * scale + dy), rgb)
    img.save(path)


def firm_names():
    names = []
    for path in sorted((ROOT / 'content').glob('**/firms.json')):
        names += [f['name'] for f in json.loads(path.read_text())['firms']]
    return sorted(set(names))


if __name__ == '__main__':
    if '--check' in sys.argv:
        problems = check_library()
        print('\n'.join(problems) or f'{len(library())} symbols clear of banned shapes')
        sys.exit(1 if problems else 0)
    OUT.write_text(json.dumps(build(), indent=1) + '\n')
    print(f'{len(library())} symbols, {len(PALETTES)} palettes -> {OUT}')
    if len(sys.argv) == 3 and sys.argv[1] == '--sheet':
        sheet(sys.argv[2], firm_names())
