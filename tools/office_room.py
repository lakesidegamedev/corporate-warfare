#!/usr/bin/env python3
"""The CEO Office room art, the source of apple/Resources/office-room.json.

One 8-bit boardroom, 288 by 176 logical pixels, drawn from rectangles in the game's palette (see
docs/rules/VISUAL.md: hard edges, no glow, no blur). Both platforms draw the same JSON with nearest
neighbour scaling: Compose on desktop and Android, SwiftUI on iPhone, iPad and Mac.

The file holds three things:
  room     the still picture (palette plus rows, '.' transparent), walls and furniture behind the people
  front    the desk and the board table, drawn over the portraits so the people sit behind them
  sprites  small frames the app swaps in: the phone with and without its light, a trophy earned and
           not, a board member's empty seat
  objects  every tappable thing: id, the words a screen reader says, the rectangle it lives in and
           the rectangle where live figures (a portrait, a number, a line) are drawn over the art

    tools/office_room.py                    write the JSON
    tools/office_room.py --preview FILE     also write a 3x PNG of the room, for looking at
"""
import json
import sys
from pathlib import Path

W, H = 288, 176
OUT = Path(__file__).resolve().parents[1] / 'apple/Resources/office-room.json'

# Game palette (docs/rules/VISUAL.md) plus the wood and brass the room needs.
INK = '#0B1622'
PANEL = '#13212F'
RAISED = '#1A2B3C'
STROKE = '#22364A'
GOLD = '#E2B04A'
GOLD_DK = '#9A7428'
POS = '#3FBF8A'
NEG = '#E5605E'
COPPER = '#C9804A'
TEXT = '#E8EEF5'
SECOND = '#93A4B8'
SEA = '#163555'
WOOD_L = '#8C6444'
WOOD = '#6E4B33'
WOOD_D = '#4A3123'
WOOD_X = '#2E1E16'
FLOOR_A = '#3A2A1E'
FLOOR_B = '#33251A'
FLOOR_LINE = '#241810'
WALL_A = '#1F3347'
WALL_B = '#1A2B3C'
WALL_HI = '#2A4259'
STEEL = '#5C6F85'
STEEL_L = '#8396AD'
STEEL_D = '#38475A'
PAPER = '#E8E0CC'
PAPER_D = '#BDB49C'
LEATHER = '#7A2E3C'
LEATHER_D = '#521E28'
LEATHER_L = '#9A4454'
GREEN = '#2F5A44'
GREEN_D = '#1F3A2C'


class Canvas:
    def __init__(self, w, h):
        self.w, self.h = w, h
        self.p = [[None] * w for _ in range(h)]

    def px(self, x, y, c):
        if 0 <= x < self.w and 0 <= y < self.h:
            self.p[y][x] = c

    def rect(self, x, y, w, h, c):
        for j in range(y, y + h):
            for i in range(x, x + w):
                self.px(i, j, c)

    def frame(self, x, y, w, h, c):
        self.rect(x, y, w, 1, c)
        self.rect(x, y + h - 1, w, 1, c)
        self.rect(x, y, 1, h, c)
        self.rect(x + w - 1, y, 1, h, c)

    def hline(self, x, y, n, c):
        self.rect(x, y, n, 1, c)

    def vline(self, x, y, n, c):
        self.rect(x, y, 1, n, c)

    def blit(self, other, x, y):
        for j in range(other.h):
            for i in range(other.w):
                if other.p[j][i]:
                    self.px(x + i, y + j, other.p[j][i])

    def encode(self):
        """palette letters in first-use order, then the rows."""
        letters = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789'
        pal, rows = {}, []
        for row in self.p:
            s = ''
            for c in row:
                if c is None:
                    s += '.'
                    continue
                if c not in pal:
                    pal[c] = letters[len(pal)]
                s += pal[c]
            rows.append(s)
        return {'palette': {v: k for k, v in pal.items()}, 'rows': rows}


def bevel_box(cv, x, y, w, h, fill, light, dark, outline=INK):
    """A solid box with a one pixel bevel: light top and left, dark bottom and right, hard outline."""
    cv.rect(x, y, w, h, fill)
    cv.hline(x + 1, y + 1, w - 2, light)
    cv.vline(x + 1, y + 1, h - 2, light)
    cv.hline(x + 1, y + h - 2, w - 2, dark)
    cv.vline(x + w - 2, y + 1, h - 2, dark)
    cv.frame(x, y, w, h, outline)


# ---- The room ------------------------------------------------------------------------------------

WALL_BOTTOM = 104


def wall_and_floor(cv):
    # Wall: panelled, light from the window side (left).
    cv.rect(0, 0, W, WALL_BOTTOM, WALL_A)
    for x in range(0, W, 48):
        cv.rect(x, 0, 24, WALL_BOTTOM - 28, WALL_B)
        cv.vline(x, 0, WALL_BOTTOM - 28, STROKE)
        cv.vline(x + 24, 0, WALL_BOTTOM - 28, STROKE)
    cv.hline(0, 0, W, WALL_HI)
    # Wainscot, dado rail and skirting.
    cv.rect(0, WALL_BOTTOM - 28, W, 28, WOOD_D)
    for x in range(0, W, 24):
        cv.vline(x, WALL_BOTTOM - 25, 22, WOOD_X)
        cv.hline(x + 3, WALL_BOTTOM - 25, 18, WOOD)
        cv.hline(x + 3, WALL_BOTTOM - 4, 18, WOOD_X)
    cv.hline(0, WALL_BOTTOM - 28, W, WOOD_L)
    cv.hline(0, WALL_BOTTOM - 27, W, WOOD)
    cv.hline(0, WALL_BOTTOM - 2, W, WOOD_X)
    cv.hline(0, WALL_BOTTOM - 1, W, INK)
    # Floor: planks running across, joints staggered.
    cv.rect(0, WALL_BOTTOM, W, H - WALL_BOTTOM, FLOOR_A)
    row = 0
    for y in range(WALL_BOTTOM, H, 8):
        if row % 2:
            cv.rect(0, y, W, 4, FLOOR_B)
        cv.hline(0, y, W, FLOOR_LINE)
        off = (row * 37) % 56
        for x in range(off, W, 56):
            cv.vline(x, y, 8, FLOOR_LINE)
        row += 1
    # A rug under the board table, a gold border and a dark field.
    cv.rect(116, 128, 168, 44, '#4A1F2A')
    cv.frame(116, 128, 168, 44, GOLD_DK)
    cv.frame(119, 131, 162, 38, '#6A2A38')


def window(cv, x, y, w, h):
    bevel_box(cv, x, y, w, h, WOOD, WOOD_L, WOOD_D)
    cv.rect(x + 3, y + 3, w - 6, h - 6, SEA)
    cv.frame(x + 2, y + 2, w - 4, h - 4, INK)
    # Sill.
    cv.rect(x - 3, y + h, w + 6, 3, WOOD_L)
    cv.hline(x - 3, y + h + 3, w + 6, WOOD_X)
    cv.frame(x - 3, y + h, w + 6, 4, INK)


def window_bars(cv, x, y, w, h):
    """The mullions drawn over the map so the view reads as a window."""
    mid = x + w // 2
    cv.rect(mid - 1, y + 3, 2, h - 6, WOOD)
    cv.vline(mid - 1, y + 3, h - 6, WOOD_L)
    cv.rect(x + 3, y + h // 2 - 1, w - 6, 2, WOOD)
    cv.hline(x + 3, y + h // 2 - 1, w - 6, WOOD_L)


def wall_chart(cv, x, y, w, h):
    bevel_box(cv, x, y, w, h, WOOD_D, WOOD, WOOD_X)
    cv.rect(x + 3, y + 3, w - 6, h - 6, '#0F1B28')
    cv.frame(x + 2, y + 2, w - 4, h - 4, STROKE)
    # Faint gridlines the live line is drawn over.
    for gy in range(y + 15, y + h - 4, 5):
        cv.hline(x + 4, gy, w - 8, '#17283A')
    # Hook and cord.
    cv.px(x + w // 2, y - 1, GOLD_DK)


def trophy_shelf(cv, x, y, w, h):
    bevel_box(cv, x, y, w, h, WOOD_D, WOOD, WOOD_X)
    cv.rect(x + 3, y + 3, w - 6, h - 7, '#0F1B28')
    cv.frame(x + 2, y + 2, w - 4, h - 6, INK)
    # Plank.
    cv.rect(x + 1, y + h - 8, w - 2, 3, WOOD_L)
    cv.hline(x + 1, y + h - 5, w - 2, WOOD_X)
    # Brackets.
    cv.rect(x + 4, y + h - 5, 2, 3, WOOD_D)
    cv.rect(x + w - 6, y + h - 5, 2, 3, WOOD_D)


def door(cv, x, y, w, h):
    bevel_box(cv, x - 3, y - 3, w + 6, h + 3, WOOD_D, WOOD, WOOD_X)
    bevel_box(cv, x, y, w, h, WOOD, WOOD_L, WOOD_D)
    # Panels.
    for (px_, py, pw, ph) in ((x + 5, y + 5, w - 10, 32), (x + 5, y + 43, w - 10, h - 49)):
        cv.rect(px_, py, pw, ph, WOOD_D)
        cv.frame(px_, py, pw, ph, WOOD_X)
        cv.hline(px_ + 1, py + 1, pw - 2, WOOD)
    # Brass handle and a small plate.
    cv.rect(x + w - 9, y + 48, 3, 8, GOLD)
    cv.vline(x + w - 9, y + 48, 8, '#F5D78A')
    cv.rect(x + w - 10, y + 46, 5, 2, GOLD_DK)
    cv.rect(x + 12, y + 8, 14, 5, GOLD_DK)
    cv.hline(x + 12, y + 8, 14, GOLD)


def desk(cv, x, y, w, h):
    # Top surface, then the front with two drawer banks and a modesty panel.
    cv.rect(x - 2, y, w + 4, 7, WOOD_L)
    cv.hline(x - 2, y, w + 4, '#B08458')
    cv.hline(x - 2, y + 6, w + 4, WOOD)
    cv.frame(x - 2, y, w + 4, 8, INK)
    cv.rect(x, y + 8, w, h - 8, WOOD)
    cv.frame(x, y + 8, w, h - 8, INK)
    cv.vline(x + 1, y + 9, h - 10, WOOD_L)
    cv.vline(x + w - 2, y + 9, h - 10, WOOD_D)
    # Drawer banks on each side.
    for dx in (x + 4, x + w - 26):
        for k in range(3):
            dy = y + 10 + k * 7
            cv.rect(dx, dy, 22, 7, WOOD_D)
            cv.frame(dx, dy, 22, 7, WOOD_X)
            cv.hline(dx + 1, dy + 1, 20, WOOD)
            cv.rect(dx + 9, dy + 3, 4, 2, GOLD)
    # Modesty panel behind the nameplate.
    cv.rect(x + 30, y + 10, w - 60, h - 12, WOOD_D)
    cv.frame(x + 30, y + 10, w - 60, h - 12, WOOD_X)
    # Legs and feet.
    cv.rect(x + 1, y + h, 5, 3, WOOD_X)
    cv.rect(x + w - 6, y + h, 5, 3, WOOD_X)


def nameplate(cv, x, y, w, h):
    cv.rect(x, y, w, h, GOLD_DK)
    cv.frame(x, y, w, h, INK)
    cv.hline(x + 1, y + 1, w - 2, GOLD)
    cv.rect(x + 2, y + 2, w - 4, h - 4, '#241810')


def cabinet(cv, x, y, w, h):
    bevel_box(cv, x, y, w, h, STEEL, STEEL_L, STEEL_D)
    n = 4
    dh = (h - 6) // n
    for k in range(n):
        dy = y + 3 + k * dh
        cv.rect(x + 3, dy, w - 6, dh - 1, '#4A5B70')
        cv.frame(x + 3, dy, w - 6, dh - 1, STEEL_D)
        cv.hline(x + 4, dy + 1, w - 8, STEEL_L)
        # Handle and a label card.
        cv.rect(x + w // 2 - 5, dy + dh - 6, 10, 2, '#B8C4D4')
        cv.rect(x + w // 2 - 6, dy + 3, 12, 5, PAPER)
        cv.frame(x + w // 2 - 6, dy + 3, 12, 5, STEEL_D)


def safe(cv, x, y, w, h):
    bevel_box(cv, x, y, w, h, '#3C4A5C', '#5E7088', '#232E3C')
    cv.rect(x + 4, y + 4, w - 8, h - 8, '#2E3A4A')
    cv.frame(x + 4, y + 4, w - 8, h - 8, '#232E3C')
    # A little display for the cash, a dial and a handle.
    cv.rect(x + 7, y + 7, w - 14, 9, '#0B1622')
    cv.frame(x + 7, y + 7, w - 14, 9, STEEL_D)
    cx, cy = x + w // 2 - 5, y + h - 11
    cv.rect(cx - 3, cy - 3, 7, 7, STEEL_L)
    cv.frame(cx - 3, cy - 3, 7, 7, INK)
    cv.rect(cx - 1, cy - 1, 3, 3, GOLD)
    cv.rect(x + w - 11, y + h - 15, 3, 9, GOLD)
    cv.rect(x + 3, y + h, 4, 2, INK)
    cv.rect(x + w - 7, y + h, 4, 2, INK)


def side_table(cv, x, y, w, h):
    cv.rect(x - 1, y, w + 2, 4, WOOD_L)
    cv.hline(x - 1, y, w + 2, '#B08458')
    cv.frame(x - 1, y, w + 2, 5, INK)
    cv.rect(x + 2, y + 5, w - 4, 3, WOOD_D)
    cv.rect(x + 2, y + 8, 3, h - 8, WOOD)
    cv.rect(x + w - 5, y + 8, 3, h - 8, WOOD)
    cv.vline(x + 2, y + 8, h - 8, WOOD_L)
    cv.vline(x + w - 5, y + 8, h - 8, WOOD_L)
    cv.hline(x + 2, y + h - 1, 3, INK)
    cv.hline(x + w - 5, y + h - 1, 3, INK)


def newspaper(cv, x, y, w, h):
    cv.rect(x, y, w, h, PAPER)
    cv.frame(x, y, w, h, INK)
    cv.hline(x + 1, y + h - 2, w - 2, PAPER_D)
    cv.rect(x + 2, y + 2, w - 4, 3, INK)
    cv.hline(x + 2, y + 3, w - 4, '#37312A')
    for k in range(4):
        cv.hline(x + 2, y + 8 + k * 3, w - 4 if k % 2 else w - 9, PAPER_D)


def chair(cv, x, y):
    """A leather chair, 34 wide and 54 tall, front on. Back at the top, seat, then legs."""
    w = 34
    bevel_box(cv, x + 2, y, w - 4, 30, LEATHER, LEATHER_L, LEATHER_D)
    cv.rect(x + 6, y + 4, w - 12, 22, LEATHER_D)
    cv.frame(x + 6, y + 4, w - 12, 22, INK)
    cv.hline(x + 7, y + 5, w - 14, LEATHER)
    # Arms.
    bevel_box(cv, x, y + 22, 6, 18, LEATHER, LEATHER_L, LEATHER_D)
    bevel_box(cv, x + w - 6, y + 22, 6, 18, LEATHER, LEATHER_L, LEATHER_D)
    # Seat.
    bevel_box(cv, x + 3, y + 32, w - 6, 10, LEATHER_L, '#B86070', LEATHER)
    # Legs.
    cv.rect(x + 5, y + 42, 4, 12, WOOD_D)
    cv.rect(x + w - 9, y + 42, 4, 12, WOOD_D)
    cv.vline(x + 5, y + 42, 12, WOOD)
    cv.hline(x + 5, y + 53, 4, INK)
    cv.hline(x + w - 9, y + 53, 4, INK)


def board_table(cv, x, y, w):
    # Top surface slab seen from slightly above, then the apron and legs.
    cv.rect(x, y, w, 14, WOOD_L)
    cv.hline(x, y, w, '#B08458')
    for gx in range(x + 8, x + w, 24):
        cv.vline(gx, y + 2, 10, WOOD)
    cv.rect(x, y + 11, w, 3, WOOD)
    cv.frame(x, y, w, 14, INK)
    cv.rect(x + 3, y + 14, w - 6, 8, WOOD_D)
    cv.frame(x + 3, y + 14, w - 6, 8, WOOD_X)
    for lx in (x + 6, x + w - 12):
        cv.rect(lx, y + 22, 6, 6, WOOD_D)
        cv.hline(lx, y + 27, 6, INK)


def plant(cv, x, y):
    cv.rect(x + 2, y + 18, 12, 10, COPPER)
    cv.frame(x + 2, y + 18, 12, 10, INK)
    cv.hline(x + 3, y + 19, 10, '#E0A070')
    for (dx, dy, c) in ((7, 0, GREEN), (3, 4, GREEN), (11, 5, GREEN_D), (6, 8, '#3C7556'), (1, 11, GREEN_D), (11, 12, GREEN), (5, 14, '#3C7556')):
        cv.rect(x + dx, y + dy, 4, 5, c)
        cv.frame(x + dx, y + dy, 4, 5, INK)
    cv.vline(x + 8, y + 8, 10, GREEN_D)


def lamp(cv, x, y):
    """A brass desk lamp; the light is flat colour, no glow."""
    cv.rect(x + 2, y + 18, 10, 3, STEEL_D)
    cv.frame(x + 2, y + 18, 10, 3, INK)
    cv.rect(x + 6, y + 8, 2, 10, GOLD_DK)
    cv.rect(x, y, 14, 8, GREEN)
    cv.frame(x, y, 14, 8, INK)
    cv.hline(x + 1, y + 1, 12, '#3C7556')


# ---- Sprites ---------------------------------------------------------------------------------------

def phone(on):
    cv = Canvas(22, 14)
    bevel_box(cv, 0, 6, 22, 8, '#2A3340', '#4A5B70', '#161C24')
    cv.rect(8, 8, 6, 4, '#0B1622')
    # Handset across the top, two wings.
    cv.rect(2, 2, 18, 5, '#2A3340')
    cv.frame(2, 2, 18, 5, INK)
    cv.hline(3, 3, 16, '#4A5B70')
    cv.rect(0, 1, 5, 6, '#1A2128')
    cv.frame(0, 1, 5, 6, INK)
    cv.rect(17, 1, 5, 6, '#1A2128')
    cv.frame(17, 1, 5, 6, INK)
    # The message light: flat red when lit, dark when not. No glow.
    if on:
        cv.rect(16, 8, 4, 3, NEG)
        cv.hline(16, 8, 4, '#FF9C98')
    else:
        cv.rect(16, 8, 4, 3, '#3A1E20')
    return cv


def trophy(earned):
    cv = Canvas(9, 11)
    main, light, dark = (GOLD, '#F5D78A', GOLD_DK) if earned else ('#2A3A4C', '#34475C', '#1A2634')
    cv.rect(1, 0, 7, 5, main)
    cv.hline(1, 0, 7, light)
    cv.rect(0, 1, 1, 3, dark)
    cv.rect(8, 1, 1, 3, dark)
    cv.rect(3, 5, 3, 3, main)
    cv.rect(1, 8, 7, 3, dark)
    cv.hline(1, 8, 7, light if earned else main)
    cv.vline(7, 1, 4, dark)
    return cv


def memo():
    cv = Canvas(26, 15)
    cv.rect(0, 1, 24, 13, PAPER)
    cv.frame(0, 1, 24, 13, INK)
    cv.rect(2, 3, 8, 3, NEG)
    cv.frame(2, 3, 8, 3, INK)
    for k in range(3):
        cv.hline(2, 8 + k * 2, 20 if k != 2 else 12, PAPER_D)
    cv.hline(1, 13, 22, PAPER_D)
    # A paper clip.
    cv.rect(20, 0, 3, 4, STEEL_L)
    cv.frame(20, 0, 3, 4, STEEL_D)
    return cv


# ---- Layout ---------------------------------------------------------------------------------------

# (id, spoken label, rect for the art and the tap target, rect for live figures drawn over it)
OBJECTS = []


def obj(oid, label, rect, live=None, **extra):
    OBJECTS.append({'id': oid, 'label': label, 'rect': list(rect), 'live': list(live) if live else None, **extra})


def build():
    room = Canvas(W, H)
    wall_and_floor(room)

    # Wall: door, window, chart, trophy shelf.
    door(room, 8, 14, 34, 90)
    obj('door', 'Research, the door', (5, 11, 40, 93))

    window(room, 56, 12, 60, 40)
    obj('window', 'Window onto the map at your headquarters', (53, 9, 66, 46), live=(59, 15, 54, 34))

    wall_chart(room, 126, 5, 50, 28)
    obj('chart', 'Wall chart, this quarter\'s profit', (126, 4, 50, 29), live=(129, 8, 44, 22))

    trophy_shelf(room, 190, 10, 52, 30)
    obj('trophies', 'Trophy shelf, achievements earned', (190, 10, 52, 30), live=(192, 13, 48, 22))
    shelf_slots = [(194 + k * 9, 21) for k in range(5)]

    # Left furniture: side table with the newspaper.
    side_table(room, 54, 82, 38, 26)
    newspaper(room, 58, 60, 30, 23)
    obj('paper', 'Newspaper, today\'s headline', (56, 58, 34, 26), live=(58, 60, 30, 23))

    # Right furniture: cabinet and safe.
    cabinet(room, 244, 34, 38, 76)
    obj('cabinet', 'Filing cabinet, financial statements', (244, 34, 38, 76))
    safe(room, 204, 76, 34, 34)
    obj('safe', 'Safe, company cash and credit', (204, 76, 34, 34), live=(211, 83, 20, 9))

    # The chief at the desk: the frame of the portrait sits on the wall behind it.
    obj('chief', 'Chief executive at the desk', (112, 36, 68, 56), live=(114, 34, 64, 64))
    front = Canvas(W, H)
    desk(front, 98, 92, 100, 32)
    nameplate(front, 128, 112, 40, 9)
    obj('nameplate', 'Nameplate', (128, 112, 40, 9), live=(129, 113, 38, 7))
    lamp(front, 184, 70)

    # The desk's two small objects: phone and memo.
    obj('phone', 'Phone, your letters', (98, 78, 26, 18), live=(99, 78, 22, 14))
    obj('memo', 'Memo, this week\'s brief', (150, 76, 30, 16), live=(152, 77, 26, 15))

    # Chairs for the three executives in front of the desk's left side.
    chair_x = [0, 33, 66]
    for k, cx in enumerate(chair_x):
        chair(room, cx, 118)
        obj(f'exec{k}', f'Executive chair {k + 1}', (cx, 112, 34, 60), live=(cx + 1, 114, 32, 32))

    # The board's table with a mandate card standing on it.
    plant(room, 270, 106)
    board_table(front, 124, 146, 156)
    obj('board', 'Board table', (124, 130, 156, 40), live=(130, 124, 100, 24))
    obj('mandate', 'Mandate card, the board\'s goal', (226, 122, 44, 34), live=(227, 123, 42, 32))

    # A framed notice board pinned above the cabinet is not needed; keep the walls quiet.
    sprites = {}
    ph_on, ph_off = phone(True), phone(False)
    sprites['phone_on'] = {'at': [99, 78], **ph_on.encode()}
    sprites['phone_off'] = {'at': [99, 78], **ph_off.encode()}
    sprites['trophy_on'] = trophy(True).encode()
    sprites['trophy_off'] = trophy(False).encode()
    sprites['memo'] = {'at': [152, 77], **memo().encode()}
    room_painted = room

    # The window's mullions are a separate sprite, drawn over the live map crop.
    bars = Canvas(60, 40)
    window_bars(bars, 0, 0, 60, 40)
    sprites['window_bars'] = {'at': [56, 12], **bars.encode()}
    # The window frame without its glass, so the map shows through; the room art is the frame only.
    for y in range(15, 49):
        for x in range(59, 113):
            room_painted.p[y][x] = None

    return room_painted, front, sprites, shelf_slots


def preview(path, room, front, sprites, shelf_slots):
    from PIL import Image
    im = Image.new('RGB', (W, H), SEA)

    def paint(cv, ox, oy):
        for y in range(cv.h):
            for x in range(cv.w):
                c = cv.p[y][x]
                if c:
                    im.putpixel((ox + x, oy + y), tuple(int(c[i:i + 2], 16) for i in (1, 3, 5)))

    paint(room, 0, 0)
    paint(front, 0, 0)
    paint(phone(True), 99, 78)
    paint(memo(), 152, 77)
    for k, (sx, sy) in enumerate(shelf_slots):
        paint(trophy(k < 3), sx, sy)
    bars = Canvas(60, 40)
    window_bars(bars, 0, 0, 60, 40)
    paint(bars, 56, 12)
    im.resize((W * 3, H * 3), 0).save(path)


def main(argv):
    room, front, sprites, shelf_slots = build()
    data = {
        'size': [W, H],
        'note': 'The CEO Office room. Generated by tools/office_room.py; do not edit by hand. '
                'Coordinates are in room pixels. room is the still picture; sprites are swapped in by the app; '
                'objects carry the tap rectangle, the spoken label and where live figures are drawn.',
        'room': room.encode(),
        'front': front.encode(),
        'sprites': sprites,
        'trophy_slots': [list(s) for s in shelf_slots],
        'objects': OBJECTS,
    }
    OUT.write_text(json.dumps(data, indent=1, sort_keys=False) + '\n')
    print(f'room {W}x{H}, {len(OBJECTS)} objects -> {OUT}')
    if '--preview' in argv:
        preview(argv[argv.index('--preview') + 1], room, front, sprites, shelf_slots)


if __name__ == '__main__':
    main(sys.argv[1:])
