#!/usr/bin/env python3
"""Source of the shared 8-bit UI icon set (apple/Resources/pixel-icons.json).

Every icon is a 16 by 16 grid with its own palette. '.' is transparent. Run this file to
regenerate the JSON; tools/test_pixel_icons.py checks the JSON and that every icon the apps
reference exists. Both apps draw the exact pixels with nearest-neighbour scaling.
"""
import json
from pathlib import Path

SIZE = 16
OUT = Path(__file__).resolve().parents[1] / 'apple/Resources/pixel-icons.json'

PAL = {
    'w': '#E8EEF4',  # main light
    's': '#8996A6',  # shade
    'g': '#E2B04A',  # gold accent
    'r': '#E54840',  # red accent
    'n': '#3FBF8A',  # green accent
    'b': '#4DA3F0',  # blue accent
}


class Grid:
    def __init__(self):
        self.g = [['.'] * SIZE for _ in range(SIZE)]

    def px(self, x, y, c):
        if 0 <= x < SIZE and 0 <= y < SIZE:
            self.g[y][x] = c
        return self

    def rect(self, x0, y0, x1, y1, c):
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                self.px(x, y, c)
        return self

    def frame(self, x0, y0, x1, y1, c):
        for x in range(x0, x1 + 1):
            self.px(x, y0, c)
            self.px(x, y1, c)
        for y in range(y0, y1 + 1):
            self.px(x0, y, c)
            self.px(x1, y, c)
        return self

    def line(self, x0, y0, x1, y1, c):
        dx, dy = abs(x1 - x0), -abs(y1 - y0)
        sx, sy = (1 if x0 < x1 else -1), (1 if y0 < y1 else -1)
        err = dx + dy
        while True:
            self.px(x0, y0, c)
            if x0 == x1 and y0 == y1:
                break
            e2 = 2 * err
            if e2 >= dy:
                err += dy
                x0 += sx
            if e2 <= dx:
                err += dx
                y0 += sy
        return self

    def disc(self, cx, cy, r, c, inner=None):
        for y in range(SIZE):
            for x in range(SIZE):
                d = ((x + .5 - cx) ** 2 + (y + .5 - cy) ** 2) ** .5
                if d <= r and (inner is None or d > inner):
                    self.px(x, y, c)
        return self

    def poly(self, pts, c):
        n = len(pts)
        for y in range(SIZE):
            for x in range(SIZE):
                px_, py_ = x + .5, y + .5
                inside = False
                j = n - 1
                for i in range(n):
                    xi, yi = pts[i]
                    xj, yj = pts[j]
                    if (yi > py_) != (yj > py_) and px_ < (xj - xi) * (py_ - yi) / (yj - yi) + xi:
                        inside = not inside
                    j = i
                if inside:
                    self.px(x, y, c)
        return self

    def art(self, x, y, rows):
        for dy, row in enumerate(rows):
            for dx, ch in enumerate(row):
                if ch != '.':
                    self.px(x + dx, y + dy, ch)
        return self

    def rows(self):
        return [''.join(r) for r in self.g]


ICONS = {}


def icon(name):
    def wrap(fn):
        g = Grid()
        fn(g)
        ICONS[name] = g
        return fn
    return wrap


# ---- navigation -------------------------------------------------------------------------

@icon('map')
def _(g):
    g.rect(1, 3, 4, 12, 'w').rect(6, 3, 9, 12, 's').rect(11, 3, 14, 12, 'w')
    g.line(5, 3, 5, 12, 's').line(10, 3, 10, 12, 'w')
    g.rect(7, 5, 8, 6, 'g').px(7, 7, 'g').px(8, 7, 'g').px(7, 8, 'g')


@icon('office')
def _(g):
    g.rect(2, 4, 8, 14, 'w').rect(9, 8, 13, 14, 's')
    for y in (6, 9, 12):
        g.rect(3, y, 4, y, 'g').rect(6, y, 7, y, 'g')
    g.rect(10, 10, 11, 10, 'w').rect(10, 12, 11, 12, 'w').rect(4, 2, 6, 3, 'w')


@icon('markets')
def _(g):
    g.rect(2, 10, 4, 14, 'w').rect(6, 6, 8, 14, 'w').rect(10, 3, 12, 14, 'w')
    g.line(1, 14, 14, 14, 's').rect(3, 8, 3, 9, 'g').rect(7, 4, 7, 5, 'g').rect(11, 1, 11, 2, 'g')


@icon('stats')
def _(g):
    g.line(2, 2, 2, 13, 's').line(2, 13, 14, 13, 's')
    g.line(3, 11, 6, 7, 'w').line(6, 7, 9, 9, 'w').line(9, 9, 13, 3, 'w')
    g.px(6, 7, 'g').px(9, 9, 'g').px(13, 3, 'g')


@icon('inbox')
def _(g):
    g.frame(1, 8, 14, 13, 'w').rect(2, 9, 13, 12, 's')
    g.poly([(1, 8.5), (3.5, 3), (12.5, 3), (15, 8.5)], 'w')
    g.rect(5, 5, 10, 5, 's').rect(5, 7, 10, 7, 's').rect(6, 9, 9, 10, 'w')


@icon('portfolio')
def _(g):
    g.poly([(8, 1.5), (14.5, 5.5), (1.5, 5.5)], 'w')
    g.rect(2, 6, 13, 6, 's')
    for x in (3, 7, 11):
        g.rect(x, 7, x + 1, 11, 'w')
    g.rect(1, 12, 14, 14, 'w').rect(1, 13, 14, 13, 's')


@icon('research')
def _(g):
    g.rect(6, 1, 9, 1, 'w').rect(7, 2, 8, 6, 'w')
    g.poly([(7, 6), (9, 6), (13.5, 14), (2.5, 14)], 'w')
    g.poly([(4.5, 11), (11.5, 11), (13, 14), (3, 14)], 'g')
    g.px(6, 9, 'b').px(9, 8, 'b')


@icon('command')
def _(g):
    g.poly([(9.5, 0.5), (3.5, 8.5), (7.5, 8.5), (6, 15.5), (12.5, 6.5), (8.5, 6.5)], 'g')


@icon('menu')
def _(g):
    for y in (3, 7, 11):
        g.rect(2, y, 13, y + 1, 'w')


# ---- file and window controls --------------------------------------------------------------

@icon('save')
def _(g):
    g.rect(2, 2, 13, 13, 'w').rect(4, 2, 11, 5, 's').rect(9, 3, 10, 4, 'w')
    g.rect(4, 8, 11, 13, 's').rect(5, 9, 10, 9, 'w').rect(5, 11, 10, 11, 'w')


@icon('load')
def _(g):
    g.rect(1, 4, 6, 5, 'w').rect(1, 5, 14, 13, 'w').rect(2, 7, 13, 12, 's')
    g.poly([(8, 8.5), (5.5, 11), (10.5, 11)], 'g')


@icon('settings')
def _(g):
    g.disc(7.5, 7.5, 5.2, 'w')
    for x, y in ((7, 1), (7, 13), (1, 7), (13, 7)):
        g.rect(x, y, x + 1, y + 1, 'w')
    for x, y in ((3, 3), (11, 3), (3, 11), (11, 11)):
        g.rect(x, y, x + 1, y + 1, 'w')
    g.disc(7.5, 7.5, 2.3, 's')


@icon('sliders')
def _(g):
    for y, knob in ((3, 4), (7, 10), (11, 6)):
        g.rect(1, y, 14, y + 1, 's').rect(knob - 1, y - 2, knob + 1, y + 3, 'w').rect(knob, y - 1, knob, y + 2, 'g')


@icon('search')
def _(g):
    g.disc(6.5, 6.5, 5, 'w', inner=3).line(10, 10, 14, 14, 'w').line(10, 11, 13, 14, 'w')
    g.px(4, 4, 'b').px(5, 4, 'b')


@icon('quit')
def _(g):
    g.frame(2, 2, 8, 13, 'w').rect(9, 2, 9, 2, 'w').rect(9, 13, 9, 13, 'w')
    g.rect(1, 7, 1, 8, 'w')
    g.rect(6, 7, 12, 8, 'g').poly([(15, 7.5 + .5), (11, 4), (11, 12)], 'g')


@icon('close')
def _(g):
    g.line(3, 3, 12, 12, 'w').line(4, 3, 12, 11, 'w').line(3, 4, 11, 12, 'w')
    g.line(12, 3, 3, 12, 'w').line(11, 3, 3, 11, 'w').line(12, 4, 4, 12, 'w')


@icon('back')
def _(g):
    g.poly([(1.5, 8), (7, 2.5), (7, 13.5)], 'w').rect(7, 6, 14, 9, 'w')


@icon('plus')
def _(g):
    g.rect(7, 2, 8, 13, 'w').rect(2, 7, 13, 8, 'w')


@icon('minus')
def _(g):
    g.rect(2, 7, 13, 8, 'w')


@icon('check')
def _(g):
    g.line(2, 8, 6, 12, 'n').line(2, 7, 6, 11, 'n').line(6, 12, 14, 3, 'n').line(6, 11, 13, 3, 'n')


@icon('check_circle')
def _(g):
    g.disc(8, 8, 7, 'n')
    g.line(4, 8, 7, 11, 'w').line(4, 7, 7, 10, 'w').line(7, 11, 12, 5, 'w').line(7, 10, 11, 5, 'w')


@icon('blocked')
def _(g):
    g.disc(8, 8, 7, 'r', inner=5).line(3, 12, 12, 3, 'r').line(4, 12, 12, 4, 'r')


@icon('circle')
def _(g):
    g.disc(8, 8, 6.5, 's', inner=4.8)


@icon('chevron_right')
def _(g):
    g.line(5, 2, 10, 7, 'w').line(5, 3, 10, 8, 'w').line(10, 8, 5, 13, 'w').line(10, 7, 5, 12, 'w')


@icon('chevron_left')
def _(g):
    g.line(10, 2, 5, 7, 'w').line(10, 3, 5, 8, 'w').line(5, 8, 10, 13, 'w').line(5, 7, 10, 12, 'w')


@icon('chevron_up')
def _(g):
    g.line(2, 10, 7, 5, 'w').line(3, 10, 8, 5, 'w').line(8, 5, 13, 10, 'w').line(7, 5, 12, 10, 'w')


@icon('chevron_down')
def _(g):
    g.line(2, 5, 7, 10, 'w').line(3, 5, 8, 10, 'w').line(8, 10, 13, 5, 'w').line(7, 10, 12, 5, 'w')


@icon('sort')
def _(g):
    g.poly([(8, 1.5), (12.5, 6.5), (3.5, 6.5)], 'w').poly([(8, 14.5), (12.5, 9.5), (3.5, 9.5)], 'w')


# ---- time and playback ---------------------------------------------------------------------

@icon('play')
def _(g):
    g.poly([(4, 2), (13, 8), (4, 14)], 'w')


@icon('pause')
def _(g):
    g.rect(3, 2, 6, 13, 'w').rect(9, 2, 12, 13, 'w')


@icon('speed1')
def _(g):
    g.poly([(5, 3), (12, 8), (5, 13)], 'w')


@icon('speed2')
def _(g):
    g.poly([(1, 3), (7.5, 8), (1, 13)], 'w').poly([(8, 3), (14.5, 8), (8, 13)], 'w')


@icon('speed3')
def _(g):
    g.poly([(0, 4), (5.2, 8), (0, 12)], 'w').poly([(5.4, 4), (10.6, 8), (5.4, 12)], 'w')
    g.poly([(10.8, 4), (16, 8), (10.8, 12)], 'w')


@icon('next_week')
def _(g):
    g.poly([(2, 3), (9, 8), (2, 13)], 'w').poly([(8, 3), (13, 8), (8, 13)], 'w').rect(13, 3, 14, 12, 'g')


@icon('calendar')
def _(g):
    g.rect(2, 3, 13, 14, 'w').rect(2, 3, 13, 5, 'r').rect(4, 1, 5, 4, 's').rect(10, 1, 11, 4, 's')
    for y in (8, 11):
        for x in (4, 7, 10):
            g.rect(x, y, x + 1, y, 's')


@icon('history')
def _(g):
    g.disc(8, 8, 6.5, 'w', inner=4.7).line(8, 8, 8, 4, 'g').line(8, 8, 11, 9, 'g')
    g.rect(0, 5, 3, 6, 'w').rect(1, 3, 1, 8, 'w')


@icon('refresh')
def _(g):
    g.disc(8, 8, 6.5, 'w', inner=4.5)
    g.rect(8, 1, 15, 6, '.').poly([(9, 0.5), (15, 3.5), (9, 6.5)], 'g').rect(9, 2, 10, 5, 'g')


# ---- map ------------------------------------------------------------------------------------

@icon('zoom_in')
def _(g):
    g.disc(6.5, 6.5, 5, 'w', inner=3.4).line(10, 10, 14, 14, 'w').line(10, 11, 13, 14, 'w')
    g.rect(6, 4, 6, 8, 'g').rect(4, 6, 8, 6, 'g')


@icon('zoom_out')
def _(g):
    g.disc(6.5, 6.5, 5, 'w', inner=3.4).line(10, 10, 14, 14, 'w').line(10, 11, 13, 14, 'w')
    g.rect(4, 6, 8, 6, 'g')


@icon('globe')
def _(g):
    g.disc(8, 8, 7, 'b')
    g.art(3, 3, ['..nnn', '.nnnn', '.nnn.', '..nn.', '...n.'])
    g.art(9, 7, ['nn.', 'nnn', '.nn', '..n'])
    g.disc(8, 8, 7, 'w', inner=6)


@icon('scope')
def _(g):
    g.disc(8, 8, 6.2, 'w', inner=4.6).rect(7, 1, 8, 4, 'w').rect(7, 11, 8, 14, 'w')
    g.rect(1, 7, 4, 8, 'w').rect(11, 7, 14, 8, 'w').rect(7, 7, 8, 8, 'g')


@icon('target')
def _(g):
    g.disc(8, 8, 7, 'w', inner=5.5).disc(8, 8, 4, 'w', inner=2.5).rect(7, 7, 8, 8, 'r')


@icon('layer_presence')
def _(g):
    g.rect(3, 6, 7, 14, 'w').rect(8, 3, 13, 14, 's').rect(4, 8, 5, 9, 'g').rect(10, 5, 11, 6, 'g')
    g.rect(10, 9, 11, 10, 'g')


@icon('layer_room')
def _(g):
    g.frame(1, 1, 14, 14, 's').poly([(2, 2), (6, 2), (2, 6)], 'w').poly([(14, 14), (10, 14), (14, 10)], 'w')
    g.line(3, 3, 7, 7, 'n').line(12, 12, 8, 8, 'n').rect(7, 7, 8, 8, 'n')


@icon('layer_wages')
def _(g):
    g.disc(8, 8, 7, 'g').disc(8, 8, 7, 'w', inner=6)
    g.rect(7, 3, 8, 12, 'w').rect(5, 5, 10, 5, 'w').rect(6, 7, 10, 7, 'w').rect(5, 9, 10, 9, 'w')


@icon('layer_skill')
def _(g):
    g.poly([(8, 2), (15, 6), (8, 10), (1, 6)], 'w')
    g.rect(4, 8, 11, 12, 's').poly([(8, 9), (12, 7), (12, 9), (8, 11), (4, 9), (4, 7)], 's')
    g.rect(14, 6, 14, 11, 'g')


# ---- movement, trade, alerts -------------------------------------------------------------

@icon('arrow_up')
def _(g):
    g.poly([(8, 1.5), (14, 8), (2, 8)], 'n').rect(6, 8, 9, 14, 'n')


@icon('arrow_down')
def _(g):
    g.poly([(8, 14.5), (14, 8), (2, 8)], 'r').rect(6, 2, 9, 8, 'r')


@icon('arrow_right')
def _(g):
    g.poly([(14.5, 8), (8, 2), (8, 14)], 'w').rect(1, 6, 8, 9, 'w')


@icon('trend_up')
def _(g):
    g.line(1, 12, 5, 8, 'n').line(5, 8, 8, 10, 'n').line(8, 10, 13, 4, 'n')
    g.line(1, 13, 5, 9, 'n').line(5, 9, 8, 11, 'n').line(8, 11, 13, 5, 'n')
    g.poly([(14.5, 2), (14.5, 8), (9, 2.5)], 'n')


@icon('trend_down')
def _(g):
    g.line(1, 3, 5, 7, 'r').line(5, 7, 8, 5, 'r').line(8, 5, 13, 11, 'r')
    g.line(1, 4, 5, 8, 'r').line(5, 8, 8, 6, 'r').line(8, 6, 13, 12, 'r')
    g.poly([(14.5, 14), (14.5, 8), (9, 13.5)], 'r')


@icon('buy')
def _(g):
    g.rect(1, 2, 3, 3, 'w').line(3, 3, 5, 10, 'w').rect(5, 10, 12, 11, 'w')
    g.poly([(4, 4), (14, 4), (12.5, 9), (5, 9)], 'w')
    g.disc(6.5, 13.5, 1.5, 's').disc(11.5, 13.5, 1.5, 's')
    g.rect(8, 5, 9, 8, 'n').rect(7, 6, 10, 7, 'n')


@icon('sell')
def _(g):
    g.poly([(1, 5), (8, 1.5), (15, 5)], 'w').rect(2, 5, 13, 14, 'w').rect(6, 9, 9, 14, 's')
    g.rect(3, 6, 4, 8, 'g').rect(11, 6, 12, 8, 'g')


@icon('alert')
def _(g):
    g.rect(7, 1, 8, 10, 'r').rect(6, 2, 9, 9, 'r').rect(7, 12, 8, 14, 'r').rect(6, 12, 9, 14, 'r')
    g.rect(7, 3, 8, 8, 'w')


@icon('warning')
def _(g):
    g.poly([(8, 1), (15.5, 14.5), (0.5, 14.5)], 'g')
    g.rect(7, 5, 8, 10, 'w').rect(7, 12, 8, 13, 'w')


@icon('info')
def _(g):
    g.disc(8, 8, 7, 'b').rect(7, 3, 8, 4, 'w').rect(7, 6, 8, 12, 'w').rect(6, 6, 7, 6, 'w').rect(6, 12, 9, 12, 'w')


@icon('hand')
def _(g):
    g.rect(3, 5, 4, 10, 'w').rect(5, 2, 6, 9, 'w').rect(7, 1, 8, 9, 'w').rect(9, 2, 10, 9, 'w').rect(11, 4, 12, 10, 'w')
    g.rect(3, 9, 12, 13, 'w').rect(5, 14, 10, 14, 'w')


@icon('bulb')
def _(g):
    g.disc(8, 6.5, 5, 'g').rect(5, 11, 10, 12, 'w').rect(6, 13, 9, 14, 's').rect(7, 4, 8, 8, 'w')


@icon('star')
def _(g):
    g.poly([(8, 0.8), (10, 5.6), (15.2, 6), (11.2, 9.4), (12.5, 14.5), (8, 11.7), (3.5, 14.5), (4.8, 9.4), (0.8, 6), (6, 5.6)], 'g')


@icon('flag')
def _(g):
    g.rect(3, 1, 4, 14, 'w').poly([(5, 2), (14, 4.5), (5, 8)], 'r')


@icon('filter')
def _(g):
    g.poly([(1, 2), (15, 2), (9.5, 8.5), (9.5, 14), (6.5, 12), (6.5, 8.5)], 'w')


@icon('hammer')
def _(g):
    g.rect(2, 2, 10, 5, 'w').rect(3, 6, 9, 6, 's').rect(6, 6, 7, 14, 'g')


@icon('tag')
def _(g):
    g.poly([(1, 2), (8, 2), (15, 9), (9, 15), (1, 8)], 'w').disc(4.5, 5.5, 1.3, 's')


@icon('trash')
def _(g):
    g.rect(2, 3, 13, 4, 'w').rect(6, 1, 9, 2, 'w').poly([(3, 5), (12, 5), (11, 15), (4, 15)], 'w')
    g.rect(6, 6, 6, 13, 's').rect(9, 6, 9, 13, 's')


@icon('share')
def _(g):
    g.frame(2, 6, 12, 14, 'w').rect(6, 6, 9, 6, '.')
    g.rect(7, 4, 8, 11, 'g').poly([(7.5, 0.5), (11, 4.5), (4, 4.5)], 'g')


@icon('swap')
def _(g):
    g.rect(2, 4, 11, 5, 'w').poly([(15, 4.5), (10, 1), (10, 8)], 'w')
    g.rect(4, 10, 13, 11, 's').poly([(1, 10.5), (6, 7), (6, 14)], 's')


@icon('indent')
def _(g):
    g.rect(3, 2, 4, 9, 'w').rect(3, 8, 11, 9, 'w').poly([(15, 8.5), (10, 4.5), (10, 12.5)], 'w')


# ---- people, money, objects -----------------------------------------------------------------

@icon('cash')
def _(g):
    g.rect(1, 4, 14, 12, 'n').frame(1, 4, 14, 12, 'w').disc(7.5, 8, 2.4, 'w').rect(4, 7, 4, 8, 'w').rect(11, 7, 11, 8, 'w')


@icon('coin')
def _(g):
    g.disc(8, 8, 7, 'g').disc(8, 8, 5, 'w', inner=4.2).rect(7, 4, 8, 11, 'w').rect(6, 5, 9, 5, 'w').rect(6, 8, 9, 8, 'w').rect(6, 11, 9, 11, 'w')


@icon('people')
def _(g):
    g.disc(5.5, 5, 2.5, 'w').poly([(1, 14), (2, 9), (9, 9), (10, 14)], 'w')
    g.disc(11.5, 5.5, 2.2, 's').poly([(10, 14), (10, 10), (14, 10), (15, 14)], 's')


@icon('user')
def _(g):
    g.disc(8, 4.5, 3, 'w').poly([(2, 15), (3, 9), (13, 9), (14, 15)], 'w')


@icon('user_add')
def _(g):
    g.disc(6, 4.5, 3, 'w').poly([(0, 15), (1, 9), (11, 9), (12, 15)], 'w')
    g.rect(13, 3, 14, 8, 'n').rect(11, 5, 16, 6, 'n')


@icon('box')
def _(g):
    g.poly([(8, 1), (15, 4.5), (8, 8), (1, 4.5)], 'w').poly([(1, 5.5), (7.5, 9), (7.5, 15), (1, 11.5)], 's')
    g.poly([(15, 5.5), (8.5, 9), (8.5, 15), (15, 11.5)], 'g')


@icon('storage')
def _(g):
    g.poly([(0.5, 7.5), (8, 1.5), (15.5, 7.5)], 'w').rect(2, 7, 13, 14, 's')
    g.rect(4, 9, 11, 14, 'g').line(4, 11, 11, 11, 's').line(4, 13, 11, 13, 's').line(7, 9, 7, 14, 's').line(8, 9, 8, 14, 's')


@icon('truck')
def _(g):
    g.rect(0, 3, 9, 10, 'w').rect(10, 6, 14, 10, 's').rect(11, 7, 13, 8, 'b').rect(0, 10, 15, 10, 's')
    g.disc(4, 12, 2, 's').disc(4, 12, 1, 'w').disc(12, 12, 2, 's').disc(12, 12, 1, 'w')


@icon('rail')
def _(g):
    g.rect(4, 1, 11, 11, 'w').rect(5, 3, 10, 6, 'b').px(5, 9, 'g').px(10, 9, 'g').rect(6, 8, 9, 8, 's')
    g.line(2, 14, 5, 12, 's').line(13, 14, 10, 12, 's').rect(1, 14, 14, 14, 's').rect(4, 15, 4, 15, 's').rect(11, 15, 11, 15, 's')


@icon('pipeline')
def _(g):
    g.rect(0, 6, 15, 9, 'w').rect(0, 7, 15, 8, 's').rect(3, 4, 4, 11, 's').rect(11, 4, 12, 11, 's')
    g.rect(7, 3, 8, 5, 'g').rect(5, 2, 10, 2, 'g')


@icon('ship')
def _(g):
    g.poly([(1, 9.5), (15, 9.5), (12, 13.5), (4, 13.5)], 'w').rect(4, 6, 11, 9, 's').rect(6, 3, 8, 5, 'g')
    g.rect(5, 7, 6, 8, 'b').rect(9, 7, 10, 8, 'b').line(0, 15, 15, 15, 'b')


@icon('lane')
def _(g):
    g.line(4, 11, 11, 4, 'w').disc(3.5, 12.5, 2.5, 'g').disc(12.5, 3.5, 2.5, 'g')


@icon('layer_freight')
def _(g):
    g.line(3, 12, 8, 4, 'w').line(8, 4, 13, 11, 'w').line(3, 12, 13, 11, 's')
    g.disc(3, 12, 2, 'g').disc(8, 4, 2, 'n').disc(13, 11, 2, 'r')


@icon('branch')
def _(g):
    g.rect(7, 1, 8, 5, 'w').rect(7, 6, 8, 7, 'w').line(7, 7, 3, 11, 'w').line(8, 7, 12, 11, 'w')
    g.rect(1, 11, 4, 14, 'g').rect(11, 11, 14, 14, 'g').rect(6, 1, 9, 3, 'w')


@icon('bank')
def _(g):
    g.poly([(8, 1), (15, 5), (1, 5)], 'w').rect(1, 5, 14, 5, 's')
    for x in (2, 6, 10):
        g.rect(x, 6, x + 2, 11, 'w')
    g.rect(1, 12, 14, 14, 'w').rect(1, 13, 14, 13, 's').rect(7, 3, 8, 4, 'g')


@icon('umbrella')
def _(g):
    g.poly([(0.5, 8), (2, 4), (5, 1.5), (11, 1.5), (14, 4), (15.5, 8)], 'r').rect(7, 8, 8, 13, 'w')
    g.rect(5, 13, 7, 14, 'w').rect(5, 8, 5, 8, 'w').rect(10, 8, 10, 8, 'w').rect(7, 0, 8, 1, 'w')


@icon('stack')
def _(g):
    g.poly([(8, 1.5), (15, 5), (8, 8.5), (1, 5)], 'w')
    g.line(1, 8, 8, 11, 's').line(8, 11, 14, 8, 's').line(1, 11, 8, 14, 's').line(8, 14, 14, 11, 's')
    g.line(1, 7, 8, 10, 's').line(8, 10, 14, 7, 's').line(1, 10, 8, 13, 's').line(8, 13, 14, 10, 's')


@icon('card')
def _(g):
    g.rect(1, 3, 14, 12, 'b').rect(1, 5, 14, 6, 'w').rect(3, 9, 6, 10, 'w')


@icon('doc')
def _(g):
    g.rect(3, 1, 12, 14, 'w').poly([(9, 1), (12, 1), (12, 4)], 's')
    for y in (5, 7, 9, 11):
        g.rect(5, y, 10, y, 's')


@icon('grad')
def _(g):
    g.poly([(8, 2), (15, 5.5), (8, 9), (1, 5.5)], 'w').rect(4, 8, 11, 12, 's')
    g.poly([(8, 9), (12, 7), (12, 9), (8, 12), (4, 9), (4, 7)], 's').rect(14, 6, 14, 11, 'g')


@icon('sparkle')
def _(g):
    g.poly([(7, 1), (9, 7), (14.5, 8), (9, 9), (7, 15), (5, 9), (-0.5, 8), (5, 7)], 'g')
    g.rect(12, 1, 13, 4, 'w').rect(11, 2, 14, 3, 'w')


@icon('percent')
def _(g):
    g.disc(4.5, 4.5, 2.8, 'w').disc(11.5, 11.5, 2.8, 'w').line(13, 2, 2, 13, 'g').line(14, 3, 3, 14, 'g')


@icon('thumb_up')
def _(g):
    g.rect(1, 7, 4, 14, 'w').poly([(5, 7), (8, 1), (10, 2), (9, 6), (14, 6), (13, 14), (5, 14)], 'w')


@icon('thumb_down')
def _(g):
    g.rect(1, 1, 4, 8, 's').poly([(5, 8), (8, 14), (10, 13), (9, 9), (14, 9), (13, 1), (5, 1)], 's')


@icon('pen')
def _(g):
    g.poly([(11, 1), (14.5, 4.5), (5, 14), (1, 15), (2, 11)], 'w').line(10, 3, 13, 6, 'g').rect(1, 14, 5, 14, 'g')


@icon('mail')
def _(g):
    g.rect(1, 3, 14, 12, 'w').line(1, 3, 7, 8, 's').line(14, 3, 8, 8, 's')


@icon('briefcase')
def _(g):
    g.frame(5, 1, 10, 4, 'w').rect(1, 4, 14, 13, 'w').rect(1, 8, 14, 8, 's').rect(7, 7, 8, 9, 'g')


@icon('gavel')
def _(g):
    g.poly([(7, 1), (11, 1), (13, 5), (9, 7)], 'w').poly([(3, 6), (6, 4), (10, 8), (8, 10)], 'w')
    g.line(7, 9, 2, 14, 'g').line(8, 9, 3, 14, 'g').rect(8, 14, 15, 14, 's')


# ---- Command Center categories --------------------------------------------------------------

@icon('cat_operations')
def _(g):
    g.disc(7.5, 7.5, 5.2, 'w')
    for x, y in ((7, 1), (7, 13), (1, 7), (13, 7), (3, 3), (11, 3), (3, 11), (11, 11)):
        g.rect(x, y, x + 1, y + 1, 'w')
    g.disc(7.5, 7.5, 2.3, 'g')


@icon('cat_finance')
def _(g):
    g.rect(1, 12, 6, 14, 'g').rect(1, 10, 6, 11, 'w').rect(1, 8, 6, 9, 'g')
    g.rect(9, 12, 14, 14, 'g').rect(9, 10, 14, 11, 'w').rect(9, 8, 14, 9, 'g').rect(9, 6, 14, 7, 'w').rect(9, 4, 14, 5, 'g')


@icon('cat_shares')
def _(g):
    g.disc(8, 8, 7, 'w').poly([(8, 8), (8, 0.5), (15.5, 8)], 'g').poly([(8, 8), (15.5, 8), (12, 14.5)], 's')


@icon('cat_deals')
def _(g):
    g.rect(0, 4, 3, 10, 's').rect(12, 4, 15, 10, 'w')
    g.poly([(3, 5), (8, 4), (12, 5), (12, 10), (8, 13), (4, 10), (3, 9)], 'g')
    g.line(5, 9, 8, 6, 'w').line(8, 6, 10, 8, 'w').line(6, 10, 8, 11, 'w')


@icon('cat_sales')
def _(g):
    g.poly([(1, 2), (8, 2), (15, 9), (9, 15), (1, 8)], 'g').disc(4.5, 5.5, 1.3, 'w').rect(8, 7, 9, 10, 'w')


@icon('cat_workforce')
def _(g):
    g.disc(8, 4, 2.6, 'w').poly([(3, 14), (4, 8), (12, 8), (13, 14)], 'w').disc(2.5, 6.5, 1.8, 's').disc(13.5, 6.5, 1.8, 's')
    g.rect(0, 10, 2, 14, 's').rect(14, 10, 15, 14, 's')


@icon('cat_supply')
def _(g):
    g.rect(0, 4, 9, 11, 'w').poly([(9, 6), (12, 6), (15, 9), (15, 11), (9, 11)], 'g')
    g.disc(3.5, 12.5, 2, 's').disc(12, 12.5, 2, 's')


@icon('cat_insurance')
def _(g):
    g.poly([(8, 1), (14.5, 3), (14, 9), (8, 15), (2, 9), (1.5, 3)], 'b').rect(7, 4, 8, 10, 'w').rect(5, 6, 10, 7, 'w')


@icon('cat_board')
def _(g):
    g.rect(1, 6, 14, 8, 'w').rect(2, 9, 3, 14, 's').rect(12, 9, 13, 14, 's')
    for x in (2, 6, 10):
        g.disc(x + 2, 3.5, 1.7, 'g')


@icon('cat_banking')
def _(g):
    g.poly([(8, 1), (15, 5), (1, 5)], 'g').rect(1, 5, 14, 5, 'w')
    for x in (2, 6, 10):
        g.rect(x, 6, x + 2, 11, 'w')
    g.rect(1, 12, 14, 14, 'g')


@icon('cat_company')
def _(g):
    g.rect(2, 3, 7, 14, 'w').rect(8, 7, 13, 14, 's').rect(4, 1, 5, 2, 'w')
    for y in (5, 8, 11):
        g.rect(3, y, 4, y, 'g').rect(5, y, 6, y, 'g')
    g.rect(10, 9, 11, 9, 'w').rect(10, 12, 11, 12, 'w')


@icon('cat_government')
def _(g):
    g.poly([(8, 1), (15, 5), (1, 5)], 'w').rect(1, 5, 14, 5, 's').rect(7, 0, 8, 1, 'g')
    for x in (2, 6, 10):
        g.rect(x, 6, x + 2, 11, 'w')
    g.rect(0, 12, 15, 14, 'w').rect(0, 13, 15, 13, 's').rect(6, 8, 9, 11, 'g')


@icon('cat_research')
def _(g):
    g.rect(6, 1, 9, 1, 'w').rect(7, 2, 8, 6, 'w')
    g.poly([(7, 6), (9, 6), (13.5, 14), (2.5, 14)], 'w')
    g.poly([(4.5, 11), (11.5, 11), (13, 14), (3, 14)], 'b')
    g.px(6, 9, 'g').px(9, 8, 'g')


def build():
    icons = {}
    for name in sorted(ICONS):
        rows = ICONS[name].rows()
        used = sorted({ch for r in rows for ch in r if ch != '.'})
        icons[name] = {'palette': {k: PAL[k] for k in used}, 'rows': rows}
    return {
        'size': SIZE,
        'note': '8-bit UI icons shared by every platform. 16 by 16, "." is transparent, each letter maps to that icon\'s palette. Generated by tools/pixel_icons.py.',
        'icons': icons,
    }


if __name__ == '__main__':
    data = build()
    OUT.write_text(json.dumps(data, indent=1) + '\n')
    print(f'{len(data["icons"])} icons -> {OUT}')
