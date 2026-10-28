#!/usr/bin/env python3
"""Source of the shared character cast portraits (apple/Resources/cast-portraits.json).

Twelve hand-drawn 16 by 16 busts, one per person in content/cast.json, plus generated busts
for the "extras" there: fictional chief executives who run the firms outside the twelve-person
cast, drawn deterministically from a small kit of hair, skin, spectacles, beards and suits. Each
bust is in the same palette-and-rows
format as pixel-icons.json: '.' is transparent and each letter maps to that portrait's own
palette. Glasses are a dark frame ('f') around a light lens ('l') so they read as clear
spectacles. Run this file to regenerate the JSON; `--sheet out.png` also writes a contact
sheet (needs Pillow) for review. tools/test_cast_portraits.py checks the JSON against this
file and against the cast content.
"""
import json
import sys
from pathlib import Path

SIZE = 16
ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'apple/Resources/cast-portraits.json'
CAST = ROOT / 'content/cast.json'
EXTRAS = 96

ART = {
    'walter_pruitt': {
        'palette': {
            's': '#F1C7A4',
            'd': '#D39F7A',
            'm': '#A9604A',
            'x': '#1B1B22',
            'h': '#8E8E90',
            'j': '#B8B8BC',
            'c': '#5A4A33',
            'e': '#3F3324',
            'a': '#2F5E8C',
            'w': '#E8EEF4',
            'g': '#7C8AA6',
            'b': '#D8D2C0',
        },
        'rows': [
            '................',
            '.....abbaaa.....',
            '....aaaaaaaa....',
            '..aaaaaaaaaaa...',
            '....hssssssh....',
            '....hssssssh....',
            '....ssxssxsd....',
            '....sshhhhsd....',
            '.....ssmmsdd....',
            '......sssd......',
            '......sssd......',
            '....ccaaaace....',
            '...ccccaaccce...',
            '..ccccccccceee..',
            '..ccccccccceee..',
            '..ccccccccceee..',
        ],
    },
    'marisol_ybarra': {
        'palette': {
            's': '#D99E70',
            'd': '#B87A52',
            'm': '#8C4A38',
            'x': '#1B1B22',
            'h': '#2A1810',
            'j': '#52352A',
            'c': '#233C5A',
            'e': '#172940',
            'a': '#C9A24A',
            'w': '#E8EEF4',
            'g': '#7C8AA6',
            'b': '#D8D2C0',
        },
        'rows': [
            '.......hh.......',
            '......jjhh......',
            '.....hjhhhh.....',
            '....hhhhhhhh....',
            '....hssssssh....',
            '....hssssssh....',
            '....ssxssxsd....',
            '....asssdssa....',
            '.....ssmmsdd....',
            '......sssd......',
            '......sssd......',
            '....ccwaawce....',
            '...ccccaaccce...',
            '..cccccaacceee..',
            '..cccccaacceee..',
            '..cccccaacceee..',
        ],
    },
    'desmond_broussard': {
        'palette': {
            's': '#A66A44',
            'd': '#83502F',
            'm': '#5A2E24',
            'x': '#1B1B22',
            'h': '#1E1A18',
            'j': '#3E3733',
            'c': '#3A3F4B',
            'e': '#262A33',
            'a': '#8B2F2F',
            'w': '#E8EEF4',
            'b': '#D8D2C0',
            'l': '#E4F2FB',
            'f': '#9AA4B8',
        },
        'rows': [
            '................',
            '................',
            '.....hjjhhh.....',
            '....hhhhhhhh....',
            '....hssssssh....',
            '....sffssffd....',
            '....flxffxlf....',
            '....sshhhhdd....',
            '.....ssmmsdd....',
            '......sssd......',
            '......sssd......',
            '....ccwaawce....',
            '...ccccaaccce...',
            '..cccccaacceee..',
            '..cccccaacceee..',
            '..cccccaacceee..',
        ],
    },
    'helen_kowalczyk': {
        'palette': {
            's': '#F6D4B8',
            'd': '#DDAE8C',
            'm': '#B4705A',
            'x': '#1B1B22',
            'h': '#B9B9BE',
            'j': '#DADADF',
            'c': '#4B2F55',
            'e': '#331F3B',
            'a': '#C9C0A8',
            'w': '#E8EEF4',
            'b': '#D8D2C0',
            'l': '#E4F2FB',
            'f': '#9AA4B8',
        },
        'rows': [
            '................',
            '.....hhhhhh.....',
            '....hhjjhhhh....',
            '...hhhhhhhhhh...',
            '...hhhhssshhh...',
            '...hhffssffhh...',
            '...hhlxffxlhh...',
            '...hhsssdsdhh...',
            '...hhssmmsdhh...',
            '...hh.sssd.hh...',
            '......sssd......',
            '....ccaaaace....',
            '...ccccaaccce...',
            '..ccccccccceee..',
            '..ccccccccceee..',
            '..ccccccccceee..',
        ],
    },
    'kenji_watanabe_moore': {
        'palette': {
            's': '#E8BE8F',
            'd': '#C99A6A',
            'm': '#9A5A48',
            'x': '#1B1B22',
            'h': '#14110F',
            'j': '#352E29',
            'c': '#2E4A3A',
            'e': '#1F3327',
            'a': '#C4572B',
            'w': '#E8EEF4',
            'g': '#7C8AA6',
            'b': '#D8D2C0',
        },
        'rows': [
            '................',
            '.....hhhhhh.....',
            '....hjjjhhhh....',
            '....hhhhhhhh....',
            '....hhsssssh....',
            '....hssssssh....',
            '....ssxssxsd....',
            '....ssssdssd....',
            '.....ssmmsdd....',
            '......sssd......',
            '......sssd......',
            '....ccwaawce....',
            '...ccccaaccce...',
            '..cccccaacceee..',
            '..cccccaacceee..',
            '..cccccaacceee..',
        ],
    },
    'gloria_abernathy': {
        'palette': {
            's': '#7A4A30',
            'd': '#5C3524',
            'm': '#3E1F1C',
            'x': '#1B1B22',
            'h': '#2A2A2E',
            'j': '#6B6B72',
            'c': '#8A2F3D',
            'e': '#601F2B',
            'a': '#E8D8A8',
            'w': '#E8EEF4',
            'b': '#D8D2C0',
            'l': '#E4F2FB',
            'f': '#9AA4B8',
        },
        'rows': [
            '......hhhh......',
            '....hjjhhhhh....',
            '...hhjhhhhhhh...',
            '...hhhhhhhhhh...',
            '...hhsssssshh...',
            '...hhffssffhh...',
            '...hhlxffxlhh...',
            '....asssdsda....',
            '.....ssmmsdd....',
            '......sssd......',
            '......sssd......',
            '....ccaaaace....',
            '...ccccaaccce...',
            '..ccccccccceee..',
            '..ccccccccceee..',
            '..ccccccccceee..',
        ],
    },
    'priya_raman': {
        'palette': {
            's': '#C8905F',
            'd': '#A66F44',
            'm': '#7A3E30',
            'x': '#1B1B22',
            'h': '#17100E',
            'j': '#3A2B26',
            'c': '#2F6F6B',
            'e': '#1F4B48',
            'a': '#D8B24A',
            'w': '#E8EEF4',
            'g': '#7C8AA6',
            'b': '#D8D2C0',
        },
        'rows': [
            '................',
            '.....hhhhhh.....',
            '....hhjjhhhh....',
            '...hhhhhhhhhh...',
            '...hhhhhssshh...',
            '...hhsssssshh...',
            '...hhsxssxshh...',
            '...hhsssdsshh...',
            '...hhssmmsdhh...',
            '...hh.sssd.hh...',
            '...hh.sssd.hh...',
            '...hhcaaaachh...',
            '...hhccaacchh...',
            '..ccccccccceee..',
            '..ccccccccceee..',
            '..ccccccccceee..',
        ],
    },
    'joel_rosenthal': {
        'palette': {
            's': '#F1C7A4',
            'd': '#D39F7A',
            'm': '#A9604A',
            'x': '#1B1B22',
            'h': '#4A3A33',
            'j': '#6A584E',
            'c': '#1F2430',
            'e': '#12151D',
            'a': '#5A7AB0',
            'w': '#E8EEF4',
            'b': '#D8D2C0',
            'l': '#E4F2FB',
            'f': '#9AA4B8',
        },
        'rows': [
            '................',
            '................',
            '......ssssd.....',
            '.....sssssd.....',
            '....hssssssh....',
            '....hffssffh....',
            '....hlxffxlh....',
            '....shhsdhhd....',
            '.....hhmmhhd....',
            '......hhhh......',
            '......sssd......',
            '....ccwaawce....',
            '...ccccaaccce...',
            '..cccccaacceee..',
            '..cccccaacceee..',
            '..cccccaacceee..',
        ],
    },
    'linh_tran': {
        'palette': {
            's': '#E8BE8F',
            'd': '#C99A6A',
            'm': '#9A5A48',
            'x': '#1B1B22',
            'h': '#0F0D0D',
            'j': '#332C2C',
            'c': '#B0472E',
            'e': '#7F3220',
            'a': '#E8EEF4',
            'w': '#E8EEF4',
            'g': '#7C8AA6',
            'b': '#D8D2C0',
        },
        'rows': [
            '................',
            '................',
            '.....hjjhhh.....',
            '....hhhhhhhh....',
            '....hssssssh....',
            '....sssssssd....',
            '....ssxssxsd....',
            '....ssssdssd....',
            '.....ssmmsdd....',
            '......sssd......',
            '......sssd......',
            '....ccaaaace....',
            '...ccccaaccce...',
            '..ccccccccceee..',
            '..ccccccccceee..',
            '..ccccccccceee..',
        ],
    },
    'ingrid_vossberg': {
        'palette': {
            's': '#F6D4B8',
            'd': '#DDAE8C',
            'm': '#B4705A',
            'x': '#1B1B22',
            'h': '#C9A25C',
            'j': '#E3C98A',
            'c': '#3A3F44',
            'e': '#25292D',
            'a': '#7A1F2B',
            'w': '#E8EEF4',
            'g': '#7C8AA6',
            'b': '#D8D2C0',
        },
        'rows': [
            '................',
            '.....hhhhhh.....',
            '....hhjjhhhh....',
            '...hhhhhhhhhh...',
            '...hhhhssshhh...',
            '...hhsssssshh...',
            '...hhsxssxshh...',
            '...hasssdssah...',
            '...hhssmmsdhh...',
            '...hh.sssd.hh...',
            '......sssd......',
            '....ccwaawce....',
            '...ccccaaccce...',
            '..cccccaacceee..',
            '..cccccaacceee..',
            '..cccccaacceee..',
        ],
    },
    'emre_kaplan': {
        'palette': {
            's': '#C8905F',
            'd': '#A66F44',
            'm': '#7A3E30',
            'x': '#1B1B22',
            'h': '#1C1512',
            'j': '#41342D',
            'c': '#4A5568',
            'e': '#323A47',
            'a': '#9A2A2A',
            'w': '#E8EEF4',
            'g': '#7C8AA6',
            'b': '#D8D2C0',
        },
        'rows': [
            '................',
            '......hhhh......',
            '.....hjjhhh.....',
            '....hhhhhhhh....',
            '....hssssssh....',
            '....hssssssh....',
            '....ssxssxsd....',
            '....sshhhhsd....',
            '.....ssmmsdd....',
            '......sssd......',
            '......sssd......',
            '....ccwaawce....',
            '...ccccaaccce...',
            '..cccccaacceee..',
            '..cccccaacceee..',
            '..cccccaacceee..',
        ],
    },
    'tobias_wendt': {
        'palette': {
            's': '#F1C7A4',
            'd': '#D39F7A',
            'm': '#A9604A',
            'x': '#1B1B22',
            'h': '#9A9A9E',
            'j': '#C4C4C8',
            'c': '#5C5240',
            'e': '#40382B',
            'a': '#2F4A7A',
            'w': '#E8EEF4',
            'b': '#D8D2C0',
            'l': '#E4F2FB',
            'f': '#9AA4B8',
        },
        'rows': [
            '................',
            '.....hhhhhh.....',
            '....hjjjhhhh....',
            '....hhhhhhhh....',
            '....hhsssssh....',
            '....hffssffh....',
            '....flxffxlf....',
            '....ssssdsdd....',
            '.....ssmmsdd....',
            '......sssd......',
            '......sssd......',
            '....ccwaawce....',
            '...ccccaaccce...',
            '..cccccaacceee..',
            '..cccccaacceee..',
            '..cccccaacceee..',
        ],
    },
}


# ---- Generated chief executives -------------------------------------------------------------

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
CITIES = [
    'Midland, Texas', 'Dallas, Texas', 'Austin, Texas', 'Lafayette, Louisiana', 'Shreveport, Louisiana',
    'Oklahoma City, Oklahoma', 'Sunnyvale, California', 'Palo Alto, California', 'Fresno, California',
    'Sacramento, California', 'Dortmund, Germany', 'Bochum, Germany', 'Cologne, Germany',
    'Bonn, Germany', 'Gelsenkirchen, Germany', 'Toledo, Ohio', 'Columbus, Ohio', 'Gary, Indiana',
    'Milwaukee, Wisconsin', 'Rockford, Illinois', 'Grand Rapids, Michigan', 'Erie, Pennsylvania',
    'Buffalo, New York', 'St. Louis, Missouri', 'Memphis, Tennessee', 'Denver, Colorado',
]
TRADES = [
    'a family wholesaler', 'a regional bank', 'a freight forwarder', 'a tool-and-die shop',
    'a grain elevator', 'a municipal utility', 'an engineering consultancy', 'a chemical plant',
    'a shipping broker', 'a steel mill', 'a textile mill', 'a trade union staff office',
]
TEMPER_LINE = {
    'integrator': 'likes to own the supplier before the supplier owns the price',
    'raider': 'watches rivals for a weak quarter and a cheap plant',
    'financier': 'prefers a clever balance sheet to a new furnace',
    'speculator': 'backs new technology and big swings',
    'builder': 'adds one plant at a time and rarely sells',
    'cautious': 'keeps a deep cash cushion and waits for others to blink',
}
SKINS = [
    ('#F6D4B8', '#DDAE8C', '#B4705A'), ('#F1C7A4', '#D39F7A', '#A9604A'),
    ('#E8BE8F', '#C99A6A', '#9A5A48'), ('#D99E70', '#B87A52', '#8C4A38'),
    ('#C8905F', '#A66F44', '#7A3E30'), ('#A66A44', '#83502F', '#5A2E24'),
    ('#7A4A30', '#5C3524', '#3E1F1C'),
]
HAIRS = [
    ('#14110F', '#352E29'), ('#2A1810', '#52352A'), ('#4A3A33', '#6A584E'), ('#7A5232', '#A07848'),
    ('#C9A25C', '#E3C98A'), ('#8E8E90', '#B8B8BC'), ('#B9B9BE', '#DADADF'), ('#6B2E1E', '#8E4A30'),
]
SUITS = [
    ('#2F5E8C', '#1E3F5F'), ('#3A3F4B', '#262A33'), ('#5A4A33', '#3F3324'), ('#233C5A', '#172940'),
    ('#2E4A3A', '#1F3327'), ('#4B2F55', '#331F3B'), ('#8A2F3D', '#601F2B'), ('#4A5568', '#323A47'),
    ('#2F6F6B', '#1F4B48'), ('#B0472E', '#7F3220'),
]
TIES = ['#8B2F2F', '#C9A24A', '#2F4A7A', '#D8B24A', '#7A1F2B', '#C4572B', '#5A7AB0', '#E8D8A8']

# Row 0 to 10 per hair style; the face rows fill 's' for skin. Glasses and beards overlay later.
HEAD = {
    'short': [
        '................', '......hhhh......', '.....hjjhhh.....', '....hhhhhhhh....',
        '....hssssssh....', '....hssssssh....', '....ssxssxsd....', '....ssssdssd....',
        '.....ssmmsdd....', '......sssd......', '......sssd......',
    ],
    'crop': [
        '................', '................', '.....hjjhhh.....', '....hhhhhhhh....',
        '....hssssssh....', '....sssssssd....', '....ssxssxsd....', '....ssssdssd....',
        '.....ssmmsdd....', '......sssd......', '......sssd......',
    ],
    'bald': [
        '................', '................', '.....dsssd......', '....ssssssss....',
        '....hssssssh....', '....sssssssd....', '....ssxssxsd....', '....ssssdssd....',
        '.....ssmmsdd....', '......sssd......', '......sssd......',
    ],
    'long': [
        '................', '.....hhhhhh.....', '....hhjjhhhh....', '...hhhhhhhhhh...',
        '...hhhhssshhh...', '...hhsssssshh...', '...hhsxssxshh...', '...hhsssdsshh...',
        '...hhssmmsdhh...', '...hh.sssd.hh...', '...hh.sssd.hh...',
    ],
    'bob': [
        '................', '.....hhhhhh.....', '....hhjjhhhh....', '...hhhhhhhhhh...',
        '...hhhhssshhh...', '...hhsssssshh...', '...hhsxssxshh...', '...hhsssdsshh...',
        '...hhssmmsdhh...', '...hh.sssd.hh...', '......sssd......',
    ],
    'bun': [
        '.......hh.......', '......jjhh......', '.....hjhhhh.....', '....hhhhhhhh....',
        '....hssssssh....', '....hssssssh....', '....ssxssxsd....', '....ssssdssd....',
        '.....ssmmsdd....', '......sssd......', '......sssd......',
    ],
}
BODY = {
    'tie': ['....ccwaawce....', '...ccccaaccce...', '..cccccaacceee..', '..cccccaacceee..', '..cccccaacceee..'],
    'open': ['....ccwwwwce....', '...ccwwwwwcce...', '..ccccwwwwceee..', '..cccccwwcceee..', '..cccccaacceee..'],
    'plain': ['....ccaaaace....', '...ccccaaccce...', '..ccccccccceee..', '..ccccccccceee..', '..ccccccccceee..'],
}


def generate_portrait(rng):
    style = rng.choice(sorted(HEAD))
    rows = list(HEAD[style])
    skin = rng.choice(SKINS)
    hair = rng.choice(HAIRS)
    suit = rng.choice(SUITS)
    glasses = rng.random() < 0.35
    beard = style != 'bun' and style != 'bob' and style != 'long' and rng.random() < 0.3
    body = 'plain' if style in ('long', 'bob', 'bun') and rng.random() < 0.5 else rng.choice(['tie', 'tie', 'open'])
    if glasses:
        rows[5] = rows[5][:5] + 'ffssff' + rows[5][11:]
        rows[6] = rows[6][:5] + 'lxffxl' + rows[6][11:]
    if beard:
        rows[7] = rows[7][:4] + 'shhhhd' + rows[7][10:]
        rows[9] = rows[9][:6] + 'hhhh' + rows[9][10:]
        rows[10] = rows[10][:6] + 'sssd' + rows[10][10:]
    rows += BODY[body]
    palette = {
        's': skin[0], 'd': skin[1], 'm': skin[2], 'x': '#1B1B22',
        'h': hair[0], 'j': hair[1], 'c': suit[0], 'e': suit[1],
        'a': rng.choice(TIES), 'w': '#E8EEF4', 'b': '#D8D2C0',
    }
    if glasses:
        palette['l'] = '#E4F2FB'
        palette['f'] = '#9AA4B8'
    used = {ch for row in rows for ch in row} | {'.'}
    palette = {k: v for k, v in palette.items() if k in used}
    return {'palette': palette, 'rows': rows}


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


def generate_extras(existing_names):
    """The generated chief executives: (people records, portraits), deterministic."""
    rng = Lcg(0xC0FFEE)
    temperaments = ['integrator', 'raider', 'financier', 'speculator', 'builder', 'cautious']
    taken = set(existing_names)
    people, art = [], {}
    index = 0
    while len(people) < EXTRAS:
        index += 1
        name = f'{rng.choice(GIVEN)} {rng.choice(FAMILY)}'
        if name in taken:
            continue
        taken.add(name)
        key = f'x_{len(people) + 1:03d}'
        temperament = temperaments[len(people) % len(temperaments)]
        home = rng.choice(CITIES)
        trade = rng.choice(TRADES)
        age = 38 + rng.below(31)
        people.append({
            'id': key,
            'name': name,
            'age': age,
            'home': home,
            'temperament': temperament,
            'background': f'Came up through {trade} and was brought in to run the firm. '
                          f'{TEMPER_LINE[temperament].capitalize()}.',
        })
        art[key] = generate_portrait(rng)
    return people, art


def extras_art():
    cast = json.loads(CAST.read_text())
    names = [p['name'] for p in cast['people']]
    return generate_extras(names)


def build():
    _, extra_art = extras_art()
    portraits = {k: {'palette': v['palette'], 'rows': v['rows']} for k, v in ART.items()}
    portraits.update(extra_art)
    return {
        'size': SIZE,
        'note': 'Character cast portraits shared by every platform. 16 by 16, "." is transparent, each letter maps to that portrait\'s palette. Generated by tools/cast_portraits.py.',
        'portraits': dict(sorted(portraits.items())),
    }


def sheet(path, scale=8, columns=12):
    from PIL import Image
    art = dict(ART)
    art.update(extras_art()[1])
    people = sorted(art)
    rows = (len(people) + columns - 1) // columns
    pad = 8
    side = SIZE * scale
    img = Image.new('RGB', (columns * (side + pad) + pad, rows * (side + pad) + pad), (30, 30, 40))
    for i, key in enumerate(people):
        ox = pad + (i % columns) * (side + pad)
        oy = pad + (i // columns) * (side + pad)
        palette = art[key]['palette']
        for y, row in enumerate(art[key]['rows']):
            for x, ch in enumerate(row):
                if ch == '.':
                    continue
                c = palette[ch].lstrip('#')
                rgb = tuple(int(c[j:j + 2], 16) for j in (0, 2, 4))
                for dy in range(scale):
                    for dx in range(scale):
                        img.putpixel((ox + x * scale + dx, oy + y * scale + dy), rgb)
    img.save(path)


if __name__ == '__main__':
    cast = json.loads(CAST.read_text())
    cast['extras'] = extras_art()[0]
    CAST.write_text(json.dumps(cast, indent=2, ensure_ascii=False) + '\n')
    data = build()
    OUT.write_text(json.dumps(data, indent=1) + '\n')
    print(f'{len(data["portraits"])} portraits -> {OUT}')
    if len(sys.argv) == 3 and sys.argv[1] == '--sheet':
        sheet(sys.argv[2])
