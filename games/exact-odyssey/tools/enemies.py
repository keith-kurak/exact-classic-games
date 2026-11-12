# Usage: python3 tools/enemies.py out.rs, then paste the consts into gpu/src/art.rs.
# Draws Exact Odyssey's enemies natively at 64x64 in NES style: shapes are
# filled with a material (three shades), lit from the top left, with fur or
# plate texture; then each 16x16 block gets one of three background palettes
# and every pixel becomes a colour number 0-3 in it. Emits Rust consts.
import math, sys

W = H = 64
# Logical colours -> (palette, index). Palettes (battle bank):
#  0 cave:   0C dark teal, 2A green, 2C cyan
#  1 beast:  00 dark grey, 10 light grey, 16 red
#  3 spirit: 03 violet, 13 purple, 30 white
COL = {
    't0': (0, 1), 't1': (0, 2), 't2': (0, 3),
    'g0': (1, 1), 'g1': (1, 2), 'r': (1, 3),
    'v0': (3, 1), 'v1': (3, 2), 'w': (3, 3),
}
# Brightness rank, to map a colour into a block's palette when it clashes.
RANK = {'t0': 1, 't1': 2, 't2': 3, 'g0': 1, 'g1': 2, 'r': 2, 'v0': 1, 'v1': 2, 'w': 3}

MAT = {
    'grey': ('g0', 'g1', 'g1'),
    'purple': ('v0', 'v1', 'w'),
    'teal': ('t0', 't2', 't2'),
    'red': ('g0', 'r', 'r'),
    'white': ('v1', 'w', 'w'),
}


def canvas():
    return [[None] * W for _ in range(H)]


def ellipse(cx, cy, rx, ry, rot=0.0):
    m = set()
    c, s = math.cos(rot), math.sin(rot)
    for y in range(H):
        for x in range(W):
            dx, dy = x + 0.5 - cx, y + 0.5 - cy
            u, v = dx * c + dy * s, -dx * s + dy * c
            if (u / rx) ** 2 + (v / ry) ** 2 <= 1:
                m.add((x, y))
    return m


def poly(pts):
    m = set()
    n = len(pts)
    for y in range(H):
        for x in range(W):
            px, py = x + 0.5, y + 0.5
            inside = False
            for i in range(n):
                x1, y1 = pts[i]
                x2, y2 = pts[(i + 1) % n]
                if (y1 > py) != (y2 > py) and px < (x2 - x1) * (py - y1) / (y2 - y1) + x1:
                    inside = not inside
            if inside:
                m.add((x, y))
    return m


def limb(x1, y1, x2, y2, r1, r2):
    m = set()
    L = math.hypot(x2 - x1, y2 - y1) or 1
    for y in range(H):
        for x in range(W):
            px, py = x + 0.5, y + 0.5
            t = max(0, min(1, ((px - x1) * (x2 - x1) + (py - y1) * (y2 - y1)) / L / L))
            qx, qy = x1 + t * (x2 - x1), y1 + t * (y2 - y1)
            if math.hypot(px - qx, py - qy) <= r1 + (r2 - r1) * t:
                m.add((x, y))
    return m


def paint(cv, mask, mat, texture=None, light=(-1, -1), seam=False):
    d, m, h = MAT[mat]
    lx, ly = light
    for (x, y) in mask:
        edge = any((x + a, y + b) not in mask for a, b in ((1, 0), (-1, 0), (0, 1), (0, -1)))
        # How far the shape runs on toward the light, and away from it.
        up = next((k for k in range(1, 9) if (x + lx * k, y + ly * k) not in mask), 9)
        down = next((k for k in range(1, 12) if (x - lx * k, y - ly * k) not in mask), 12)
        if edge:
            c = d
        elif up <= 2:
            c = h
        elif down <= 3 or down < up * 0.45:
            c = d
        elif down <= 5 or down < up * 0.7:
            c = d if (x + y) % 2 == 0 else m
        else:
            c = m
        if texture == 'fur' and c == m and (x * 7 + y * 3) % 11 == 0:
            c = d
        if texture == 'plate' and c == m and (y % 9 == 0 or x % 13 == 0):
            c = d
        cv[y][x] = c
    if seam:
        for (x, y) in mask:
            if any((x + a, y + b) not in mask and 0 <= x + a < W and 0 <= y + b < H and cv[y + b][x + a]
                   for a, b in ((1, 0), (0, 1), (-1, 0))) and (x + y) % 3 != 0:
                pass
    return mask


def dot(cv, pts, c):
    for x, y in pts:
        if 0 <= x < W and 0 <= y < H:
            cv[y][x] = c


RGB = {'t0': (0, 60, 118), 't1': (67, 246, 17), 't2': (44, 213, 246), 'g0': (98, 98, 98),
       'g1': (171, 171, 171), 'r': (199, 46, 0), 'v0': (82, 0, 178), 'v1': (138, 19, 255), 'w': (255, 255, 255)}


def nearest(c, p):
    opts = [k for k, (pp, _) in COL.items() if pp == p]
    best = min(opts, key=lambda k: sum((a - b) ** 2 for a, b in zip(RGB[k], RGB[c])))
    return COL[best][1]


def finish(cv, name):
    # One palette per 16x16 block: the one covering the most pixels.
    pals = []
    rows = [['.'] * W for _ in range(H)]
    for by in range(8):
        prow = []
        for bx in range(8):
            count = {0: 0, 1: 0, 3: 0}
            for y in range(by * 8, by * 8 + 8):
                for x in range(bx * 8, bx * 8 + 8):
                    c = cv[y][x]
                    if c:
                        count[COL[c][0]] += 1
            p = max(count, key=lambda k: (count[k], k == 1))
            prow.append(p)
            for y in range(by * 8, by * 8 + 8):
                for x in range(bx * 8, bx * 8 + 8):
                    c = cv[y][x]
                    if not c:
                        continue
                    cp, ci = COL[c]
                    rows[y][x] = str(ci if cp == p else nearest(c, p))
        pals.append(''.join(map(str, prow)))
    out = f'const {name}: &str = "\n' + '\n'.join(''.join(r) for r in rows) + '";\n'
    out += f'const {name}_PAL: [&str; 8] = [{", ".join(chr(34) + p + chr(34) for p in pals)}];\n'
    return out


# ── The hound: a lean grey wolf-beast facing right, a spiked violet mane
# along neck and back, red eyes and claws, a white-fanged jaw. ──
def hound():
    cv = canvas()
    # Tail, far legs, body, near legs, neck, head.
    paint(cv, limb(12, 30, 1, 20, 4, 2), 'grey', 'fur')
    paint(cv, limb(1, 20, 4, 11, 2, 1.5), 'grey')
    paint(cv, limb(17, 40, 13, 57, 4, 2.5), 'grey', 'fur')
    paint(cv, limb(40, 40, 44, 57, 4, 2.5), 'grey', 'fur')
    paint(cv, ellipse(28, 35, 19, 10, -0.08), 'grey', 'fur')
    paint(cv, limb(22, 40, 20, 59, 5, 3), 'grey', 'fur')
    paint(cv, limb(47, 37, 51, 59, 5, 3), 'grey', 'fur')
    paint(cv, ellipse(45, 30, 9, 10, 0.6), 'grey', 'fur')
    paint(cv, ellipse(51, 25, 10, 8), 'grey', 'fur')
    paint(cv, poly([(54, 19), (64, 23), (64, 29), (54, 32)]), 'grey')
    paint(cv, poly([(49, 20), (47, 9), (55, 18)]), 'grey')
    # Mane: locks sweeping back from the crown along the spine.
    locks = [(50, 19, 36, 6, 4), (46, 22, 30, 10, 4), (42, 25, 24, 14, 4), (36, 27, 16, 18, 4),
             (30, 28, 10, 22, 3), (24, 28, 6, 27, 3)]
    for bx, by, tx, ty, r in locks:
        paint(cv, limb(bx, by, tx, ty, r, 0.6), 'purple')
    # Jaw: red maw, white fangs.
    paint(cv, poly([(55, 27), (63, 28), (62, 32), (54, 31)]), 'red')
    dot(cv, [(57, 28), (59, 28), (61, 28), (58, 31), (60, 31)], 'g1')
    # Eye.
    dot(cv, [(52, 21), (53, 21), (54, 21), (55, 21), (56, 22)], 'g0')
    dot(cv, [(53, 22), (54, 22), (55, 22), (54, 23), (55, 23)], 'r')
    # Claws.
    for x, y in ((10, 58), (18, 60), (41, 58), (49, 60)):
        dot(cv, [(x, y), (x + 2, y), (x + 4, y), (x + 1, y - 1), (x + 3, y - 1)], 'r')
    return finish(cv, 'HOUND')


# ── The drone: a floating armoured eye: a grey plated shell, a red iris in
# a violet socket, teal energy fins, a cyan thruster glow beneath. ──
def drone():
    cv = canvas()
    # Fins behind the shell.
    paint(cv, poly([(14, 26), (0, 14), (2, 34), (14, 38)]), 'teal')
    paint(cv, poly([(50, 26), (64, 14), (62, 34), (50, 38)]), 'teal')
    paint(cv, poly([(20, 44), (8, 58), (24, 50)]), 'teal')
    paint(cv, poly([(44, 44), (56, 58), (40, 50)]), 'teal')
    # Antenna.
    paint(cv, limb(32, 12, 36, 1, 1.5, 1), 'grey')
    dot(cv, [(36, 0), (37, 0), (36, 1), (37, 1)], 'r')
    # Shell.
    paint(cv, ellipse(32, 31, 20, 19), 'grey', 'plate')
    # Socket and eye.
    paint(cv, ellipse(33, 31, 11, 10), 'red')
    for (x, y) in ellipse(34, 31, 3, 3):
        cv[y][x] = None
    dot(cv, [(30, 27), (31, 27), (30, 28)], 'g1')
    # Rivets around the rim.
    for k in range(12):
        a = k * math.pi / 6
        dot(cv, [(int(32 + 17 * math.cos(a)), int(31 + 16 * math.sin(a)))], 'g1')
    # Thruster glow.
    paint(cv, ellipse(32, 54, 7, 4), 'teal')
    paint(cv, ellipse(32, 59, 4, 4), 'teal')
    return finish(cv, 'DRONE')


open(sys.argv[1], 'w').write(hound() + '\n' + drone())
