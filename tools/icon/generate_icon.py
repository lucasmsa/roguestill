#!/usr/bin/env python3
"""Procedural app icon: a seeded cave lit by the hero's torch, standing still at its centre.
Usage: generate_icon.py <seed> <out.png> [size]"""
import random
import sys
from collections import deque

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

N = 20  # dungeon cells per side
INK = np.array([10, 7, 6])
FRAME = np.array([92, 59, 46])
AMBER = np.array([255, 166, 61])
CREAM = np.array([240, 230, 217])
STONE = [np.array(c) for c in ([14, 12, 20], [30, 26, 38], [52, 46, 60], [78, 68, 74])]
FLOOR = [np.array(c) for c in ([24, 17, 15], [58, 38, 28], [112, 70, 38], [176, 108, 44], [236, 150, 58])]
BAYER = np.array([[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]) / 16.0
HERO = ["..###..", ".#####.", ".#####.", "..###..", ".#####.", "#.###.#", "..#.#.."]


def cave(rng):
    wall = np.array([[rng.random() < 0.44 for _ in range(N)] for _ in range(N)])
    for _ in range(4):
        nxt = wall.copy()
        for y in range(N):
            for x in range(N):
                n = sum(
                    wall[j][i] if 0 <= i < N and 0 <= j < N else True
                    for j in range(y - 1, y + 2)
                    for i in range(x - 1, x + 2)
                )
                nxt[y][x] = n >= 5
        wall = nxt
    c = N // 2
    wall[c - 2 : c + 3, c - 2 : c + 3] = False
    seen, queue = {(c, c)}, deque([(c, c)])
    while queue:
        y, x = queue.popleft()
        for dy, dx in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            y2, x2 = y + dy, x + dx
            if 0 <= y2 < N and 0 <= x2 < N and not wall[y2][x2] and (y2, x2) not in seen:
                seen.add((y2, x2))
                queue.append((y2, x2))
    for y in range(N):
        for x in range(N):
            if (y, x) not in seen:
                wall[y][x] = True
    wall[0, :] = wall[-1, :] = wall[:, 0] = wall[:, -1] = True
    return wall


def torches(wall, rng):
    spots = [
        (y, x)
        for y in range(2, N - 2)
        for x in range(1, N - 1)
        if wall[y][x] and not wall[y + 1][x] and abs(y - N // 2) + abs(x - N // 2) > 5
    ]
    rng.shuffle(spots)
    return spots[:3]


AT = [
    "..#####..",
    ".#######.",
    "##.....##",
    "##.###.##",
    "##.#.#.##",
    "##.####.#",
    "##.......",
    ".##....##",
    "..#####..",
]


def render(seed, size):
    rng = random.Random(seed)
    wall = cave(rng)
    c = N // 2
    lamps = [(c + 0.5, c + 0.5, 4.2, 1.0)] + [(y + 1.0, x + 0.5, 2.2, 0.75) for y, x in torches(wall, rng)]
    px = 8
    ys, xs = np.mgrid[0 : N * px, 0 : N * px]
    fy, fx = (ys + 0.5) / px, (xs + 0.5) / px
    light = np.zeros((N * px, N * px))
    for ly, lx, sigma, power in lamps:
        light += power * np.exp(-(((fy - ly) ** 2 + (fx - lx) ** 2) / (2 * sigma**2)))
    light = np.clip(light, 0, 1)
    dither = np.tile(BAYER, (N * px // 4, N * px // 4))
    tiers = np.clip(((light + (dither - 0.5) * 0.16) * 5).astype(int), 0, 4)
    cy, cx = (ys // px), (xs // px)
    is_wall = wall[cy, cx]
    below_open = np.zeros_like(is_wall)
    below_open[:, :] = np.array([[y + 1 < N and not wall[y + 1][x] for x in range(N)] for y in range(N)])[cy, cx]
    face = is_wall & below_open
    img = np.zeros((N * px, N * px, 3))
    for y in range(N * px):
        for x in range(N * px):
            t = tiers[y][x]
            if face[y][x]:
                col = FLOOR[min(t, 3)] * 0.8
            elif is_wall[y][x]:
                col = STONE[min(t, 3)]
                brick = ((x // px) + ((y // px) % 2) * 2 + (x % px) // 4) % 4 == 0 and y % px == px - 1
                col = col * 0.7 if brick else col
            else:
                col = FLOOR[t]
                if x % px == 0 or y % px == 0:
                    col = col * 0.72
            img[y][x] = col
    out = Image.fromarray(img.clip(0, 255).astype("uint8")).resize((size, size), Image.NEAREST).convert("RGB")
    glyph = Image.new("L", out.size, 0)
    gd = ImageDraw.Draw(glyph)
    unit = int(size * 0.05)
    left = (size - unit * len(AT[0])) // 2
    top = (size - unit * len(AT)) // 2
    for r, row in enumerate(AT):
        for k, ch in enumerate(row):
            if ch == "#":
                gd.rectangle([left + k * unit, top + r * unit, left + (k + 1) * unit - 1, top + (r + 1) * unit - 1], fill=255)
    bloom = glyph.filter(ImageFilter.GaussianBlur(size * 0.035))
    shadow = glyph.filter(ImageFilter.GaussianBlur(size * 0.012)).point(lambda v: int(v * 0.9))
    out = Image.composite(Image.new("RGB", out.size, tuple(int(v) for v in INK)), out, shadow.transform(out.size, Image.AFFINE, (1, 0, -size * 0.012, 0, 1, -size * 0.012)))
    out = Image.composite(Image.new("RGB", out.size, tuple(int(v) for v in AMBER)), out, bloom.point(lambda v: int(v * 0.8)))
    out = Image.composite(Image.new("RGB", out.size, tuple(int(v) for v in CREAM)), out, glyph)
    radius = int(size * 0.225)
    frame = max(4, size // 48)
    alpha = Image.new("L", out.size, 0)
    ImageDraw.Draw(alpha).rounded_rectangle([0, 0, size - 1, size - 1], radius, fill=255)
    border = alpha.copy()
    ImageDraw.Draw(border).rounded_rectangle([frame, frame, size - 1 - frame, size - 1 - frame], radius - frame, fill=0)
    out = Image.composite(Image.new("RGB", out.size, tuple(int(v) for v in FRAME)), out, border)
    out.putalpha(alpha)
    return out


if __name__ == "__main__":
    seed, path = sys.argv[1], sys.argv[2]
    render(seed, int(sys.argv[3]) if len(sys.argv) > 3 else 1024).save(path)
