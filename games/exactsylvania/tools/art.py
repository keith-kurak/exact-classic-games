# Exactsylvania's starting art, drawn as text pixel art (every pixel a colour
# number 0-3 in its palette), exported as the PNG sprite sheets the game loads
# (assets/sprites/*.png) with their manifest (gpu/src/sheets.rs). The sheets
# are the source once they exist: edit them in a paint program. This script
# writes a sheet only when it is missing, or for all of them with --export
# (which overwrites your edits). It always writes the palettes
# (gpu/src/data.rs) and tools/preview.png.
import os, zlib, struct

HERE = os.path.dirname(os.path.abspath(__file__))

# ── Palettes (2C02 indices) ──
BG = [
    [0x0F, 0x2D, 0x00, 0x10],  # 0 stone: blocks, bricks, pillars, fence
    [0x0F, 0x07, 0x17, 0x27],  # 1 wood and earth: trees, door, balcony beams
    [0x0F, 0x0C, 0x1C, 0x30],  # 2 night: sky, moon, far castle, windows
    [0x0F, 0x16, 0x27, 0x30],  # 3 red: curtains; the HUD (red and orange bars, white text)
]
SPR = [
    [0x0F, 0x17, 0x16, 0x37],  # 0 Vesper: brown hair and boots, crimson coat, skin
    [0x0F, 0x19, 0x2A, 0x3A],  # 1 ghoul: rags, rotten green, pale
    [0x0F, 0x00, 0x10, 0x30],  # 2 iron: knight, bat, dagger, whip chain
    [0x0F, 0x16, 0x27, 0x30],  # 3 fire: candles, hearts, flames
]


def grid(rows, w, h):
    out = []
    for r in rows:
        r = r.replace(' ', '.')
        assert len(r) <= w, (r, len(r), w)
        out.append(r + '.' * (w - len(r)))
    while len(out) < h:
        out.append('.' * w)
    assert len(out) == h, (len(out), h)
    return out


# ── Background metatiles, 16x16 ──
def tile(fn):
    return [''.join(str(fn(x, y)) for x in range(16)) for y in range(16)]


def stone(x, y):
    # Big blocks, 16 wide, offset each half; lit top-left edges, dark seams.
    bx = (x + (8 if y >= 8 else 0)) % 16
    yy = y % 8
    if yy == 7 or bx == 15:
        return 1
    if yy == 0 or bx == 0:
        return 3
    return 2 if (x * 3 + y * 5) % 7 else 1


def brick(x, y):
    # Hall wall: small bricks, dim (colours 1-2 only, so the room stays dark).
    bx = (x + (4 if (y // 4) % 2 else 0)) % 8
    if y % 4 == 3 or bx == 7:
        return 0
    return 2 if (x + y) % 5 == 0 else 1


def sky(x, y):
    return 0


def stars(x, y):
    return 3 if (x, y) in ((4, 5), (11, 11)) else (2 if (x, y) in ((5, 5), (4, 6), (12, 3)) else 0)


def moon_q(qx, qy):
    def f(x, y):
        X, Y = x + qx * 16 - 16, y + qy * 16 - 16
        d = (X + 0.5) ** 2 + (Y + 0.5) ** 2
        if d > 13.5 ** 2:
            return 0
        crater = (X + 5) ** 2 + (Y + 3) ** 2 < 10 or (X - 4) ** 2 + (Y - 5) ** 2 < 14 or (X - 6) ** 2 + (Y + 6) ** 2 < 5
        if d > 12.5 ** 2:
            return 2
        return 2 if crater else 3
    return f


def castle(x, y):
    return 1


def castle_top(x, y):
    return 1 if y >= 8 or (x % 8 < 4 and y >= 3) else 0


def castle_window(x, y):
    return 3 if 6 <= x <= 9 and 4 <= y <= 11 and not (y == 4 and x in (6, 9)) else 1


def tree_top(x, y):
    # Bare branches.
    pts = {(7, 15), (7, 14), (8, 13), (6, 12), (8, 12), (5, 11), (9, 11), (4, 10), (10, 10), (3, 9), (11, 9),
           (12, 8), (2, 8), (13, 7), (1, 7), (6, 9), (5, 8), (9, 8), (10, 7), (4, 6), (11, 6), (12, 5), (3, 5)}
    if (x, y) in pts:
        return 1
    if (x - 1, y) in pts and y > 8:
        return 2
    return 0


def tree_trunk(x, y):
    if 5 <= x <= 10:
        return 1 if x in (5, 10) else (2 if x in (6, 7) else 1)
    return 0


def fence_top(x, y):
    if x % 4 == 1:
        return 3 if y >= 5 else (3 if y >= 2 and y <= 4 and True else 0) if y >= 2 else 0
    if x % 4 in (0, 2) and 3 <= y <= 4:
        return 2
    if y >= 12:
        return 2 if y == 12 else 0
    return 0


def fence_bars(x, y):
    if x % 4 == 1:
        return 2
    if 4 <= y <= 5:
        return 1
    return 0


def outer_wall(x, y):
    bx = (x + (8 if (y // 8) % 2 else 0)) % 16
    if y % 8 == 7 or bx == 15:
        return 1
    if y % 8 == 0 or bx == 0:
        return 3
    return 2


def arch_top(x, y):
    # Voussoirs over a dark opening.
    import math
    d = math.hypot((x + 0.5) - (16 if x >= 8 else 0), 16 - (y + 0.5))
    return outer_wall(x, y) if y < 6 else (1 if y < 9 else 0)


def arch(x, y):
    return 0


def valance(x, y):
    # A swagged red curtain: folds, a scalloped hem, a gold cord.
    if y == 1:
        return 2
    hem = 11 + (3 if x % 8 in (3, 4) else (2 if x % 8 in (2, 5) else 0))
    if y > hem:
        return 0
    return 0 if x % 4 == 0 and y > 3 else (2 if y == hem else 1)


def drape(x, y):
    if x in (0, 15):
        return 0
    return 0 if x % 5 == 2 else (2 if x % 5 == 1 else 1)


def window_top(x, y):
    import math
    if math.hypot(x - 7.5, 15.5 - y) > 8:
        return brick(x, y)
    if math.hypot(x - 7.5, 15.5 - y) > 6.5:
        return 1
    return 3 if (x == 7 or x == 8) else 2


def window(x, y):
    if x < 1 or x > 14:
        return brick(x, y)
    if x in (1, 14):
        return 1
    if x in (7, 8) or y == 8:
        return 1
    return 3 if (x + y) % 9 == 0 else 2


def pillar(x, y):
    if x < 3 or x > 12:
        return brick(x, y)
    if x in (3, 12):
        return 1
    return 3 if x in (5, 6) else (2 if x < 10 else 1)


def balcony(x, y):
    if y < 4:
        return 3 if y == 0 else 2
    if y < 6:
        return 1
    # A railing beneath, posts every 4.
    if x % 4 == 1 and y < 14:
        return 2
    return 1 if y == 13 else 0


def stair_up(x, y):
    # Steps rising to the right: two 8x8 steps on the diagonal.
    step = 8 if x >= 8 else 16
    top = 8 if x >= 8 else 16
    edge = (0 if x >= 8 else 8)
    if y >= (0 if x >= 8 else 8):
        return 3 if y == (0 if x >= 8 else 8) else (1 if y % 8 == 7 else 2)
    return brick(x, y)


def stair_down(x, y):
    return stair_up(15 - x, y)


def door(left, top):
    def f(x, y):
        X = x + (0 if left else 16)
        Y = y + (0 if top else 16)
        import math
        if top and math.hypot(X - 15.5, 16 - Y) > 15:
            return brick(x, y)
        if X in (0, 31) or (top and math.hypot(X - 15.5, 16 - Y) > 14):
            return 1
        if X in (15, 16):
            return 1
        if Y % 6 == 0 and Y > 2:
            return 1
        if (X, Y) in ((13, 18), (18, 18)):
            return 3
        return 2
    return f


METATILES = [
    # char, palette, pixel fn
    (' ', 2, sky), ('`', 2, stars),
    ('m', 2, moon_q(0, 0)), ('n', 2, moon_q(1, 0)), ('o', 2, moon_q(0, 1)), ('q', 2, moon_q(1, 1)),
    ('k', 2, castle), ('K', 2, castle_top), ('j', 2, castle_window),
    ('T', 1, tree_top), ('t', 1, tree_trunk), ('F', 0, fence_top), ('f', 0, fence_bars),
    ('W', 0, outer_wall), ('A', 0, arch_top), ('a', 0, arch),
    ('c', 3, valance), ('C', 3, drape), ('b', 0, brick), ('w', 2, window_top), ('v', 2, window),
    ('P', 0, pillar), ('=', 1, balcony), ('/', 0, stair_up), ('z', 0, stair_down),
    ('D', 1, door(True, True)), ('E', 1, door(False, True)), ('d', 1, door(True, False)), ('e', 1, door(False, False)),
    ('#', 0, stone),
]

# ── Sprites ──
# Vesper Holt, 16x32, facing right. 1 hair, boots and outline; 2 coat; 3 skin.
HEAD = [
    "................",
    "......11111.....",
    ".....1111111....",
    "....111111111...",
    "....1111113331..",
    "....111113333...",
    "....11111313....",
    ".....1113333....",
    ".....11.333.....",
    "........33......",
]
TORSO = [
    "......222222....",
    ".....22222222...",
    "....2222232222..",
    "....2222222222..",
    "....222222222...",
    "....222222222...",
    "....322222222...",
    ".....2222222....",
    ".....1111111....",
    ".....2222222....",
    "....222222222...",
    "....2222.2222...",
]
TORSO_BACK = [  # whip raised behind the head
    "..33..222222....",
    "..322222222.....",
    "...222232222....",
    "....2222222222..",
    "....222222222...",
    "....222222222...",
    "....222222222...",
    ".....2222222....",
    ".....1111111....",
    ".....2222222....",
    "....222222222...",
    "....2222.2222...",
]
TORSO_FORE = [  # arm thrown forward
    "......222222....",
    ".....22222222...",
    "....222222222222",
    "....22222222223.",
    "....222222222...",
    "....222222222...",
    "....222222222...",
    ".....2222222....",
    ".....1111111....",
    ".....2222222....",
    "....222222222...",
    "....2222.2222...",
]
LEGS = [
    "....222...222...",
    ".....11...11....",
    ".....11...11....",
    ".....11...11....",
    ".....11...11....",
    ".....11...11....",
    "....111...111...",
    "....111...111...",
    "...1111...1111..",
    "................",
]
LEGS_A = [
    "....222...222...",
    "....11.....11...",
    "....11......11..",
    "...11.......11..",
    "...11........11.",
    "...11........11.",
    "..111........111",
    "..111.........11",
    ".1111.........11",
    "................",
]
LEGS_B = [
    ".....222222.....",
    "......1111......",
    "......111.......",
    "......111.......",
    "......111.......",
    ".....1111.......",
    ".....111........",
    ".....111........",
    "....1111........",
    "................",
]
LEGS_LUNGE = [
    "....222...222...",
    "....11.....11...",
    "...11.......11..",
    "...11.......11..",
    "..11.........11.",
    "..11.........11.",
    ".111.........111",
    ".111..........11",
    "1111..........11",
    "................",
]
# The walk: arms swing opposite the legs; on the passing frames the body
# rises a pixel.
TORSO_SWING_F = [
    "......222222....",
    ".....22222222...",
    "....2222232222..",
    "....222222222222",
    "....22222222.222",
    "....22222222..33",
    "....2222222.....",
    ".....2222222....",
    ".....1111111....",
    ".....2222222....",
    "....222222222...",
    "....2222.2222...",
]
TORSO_SWING_B = [
    "......222222....",
    ".....22222222...",
    "...22222232222..",
    "..222222222222..",
    "..22.22222222...",
    ".33..22222222...",
    "....222222222...",
    ".....2222222....",
    ".....1111111....",
    ".....2222222....",
    "....222222222...",
    "....2222.2222...",
]
LEGS_PASS_F = [
    "....22222222....",
    ".....111.111....",
    ".....111..111...",
    ".....111...11...",
    ".....111..11....",
    ".....111.11.....",
    ".....111........",
    ".....111........",
    "....1111........",
    "................",
]
LEGS_PASS_B = [
    "....22222222....",
    "....111.111.....",
    "...111...111....",
    "..111....111....",
    "..11.....111....",
    "..1......111....",
    ".........111....",
    ".........111....",
    ".........1111...",
    "................",
]
TUCK = [
    "....222222222...",
    "....11111111....",
    "...111....111...",
    "...11......11...",
    "...111....111...",
    "................",
    "................",
    "................",
    "................",
    "................",
]


def hero(torso, legs, drop=0, lift=0):
    rows = ['.' * 16] * drop + HEAD + torso + legs
    rows = rows[lift:] + ['.' * 16] * lift
    return grid(rows[:32], 16, 32)


def crouch(torso):
    t = torso[:6] + torso[8:10]
    legs = [
        "...2222222222...",
        "...111....1111..",
        "..1111....11111.",
        "................",
    ]
    return grid(['.' * 16] * 10 + HEAD + t + legs, 16, 32)[:32]


HURT = grid([r[1:] + '.' for r in HEAD] + [
    "...3..222222....",
    "...22222222.....",
    "..22222222222...",
    "...22222222222..",
    "....222222223...",
    "....22222222....",
    ".....2222222....",
    ".....2222222....",
    ".....1111111....",
    ".....2222222....",
    "....222222222...",
    "....2222.2222...",
] + LEGS_A, 16, 32)

# The whip: wound back behind her (16x16), and lashed out (40x8). 1 chain, 3 glint.
WHIP_BACK = grid([
    "..........2.....",
    ".........2......",
    "........2.......",
    ".......2........",
    "......2.........",
    ".....2..........",
    "....2...........",
    "...2............",
    "..23............",
    "..2.............",
    "................",
], 16, 16)
WHIP_OUT = grid([
    "........................................",
    "...............................23.......",
    "2222.2222.2222.2222.2222.2222.2222.2233.",
    ".2222.2222.2222.2222.2222.2222.2222.2333",
    "...............................23.......",
], 40, 8)

# Ghoul, 16x32, facing right, two steps. 1 rags, 2 rotten skin, 3 pale.
GHOUL_0 = grid([
    "................",
    "................",
    "......2222......",
    ".....222222.....",
    ".....2232232....",
    ".....2222222....",
    "......21212.....",
    "......2222......",
    "....11111111....",
    "...1111111111...",
    "...111111122222.",
    "...11111111..222",
    "...1111111......",
    "...11111111222..",
    "....1111111..222",
    "....111111......",
    "....1111111.....",
    "...11.1111.1....",
    "...1..1111..1...",
    "......2222......",
    "......2..2......",
    ".....22..22.....",
    ".....2....2.....",
    ".....2....2.....",
    "....22....22....",
    "....2......2....",
    "....2......2....",
    "...22......22...",
    "..222.....222...",
    "................",
], 16, 32)
GHOUL_1 = grid(GHOUL_0[:19] + [
    "......2222......",
    "......2..2......",
    "......2..2......",
    "......2..2......",
    "......2.22......",
    ".....22.2.......",
    ".....2..2.......",
    ".....2..2.......",
    "....22..22......",
    "...222..222.....",
    "................",
], 16, 32)

# Bat, 16x16, wings up and down. Iron palette: 1 body, 2 wing, 3 eyes.
BAT_0 = grid([
    "................",
    "11............11",
    ".11..........11.",
    ".2211......1122.",
    "..222111111222..",
    "..222213312222..",
    "...2221111222...",
    "....22.11.22....",
    "........1.......",
], 16, 16)
BAT_1 = grid([
    "................",
    "................",
    "................",
    "......1111......",
    "....113311.....",
    "..22211112222...",
    ".2222.11..2222..",
    "222....1.....222",
    "22............22",
    "2..............2",
], 16, 16)

# Knight, 16x32, facing right: plumed helm, round shield, short sword.
# Iron palette: 1 dark plate, 2 plate, 3 shine.
KNIGHT_0 = grid([
    "......3.........",
    ".....33.........",
    "....333.........",
    "....3122222.....",
    "....12222222....",
    "....122222222...",
    "....12212222....",
    "....122111111...",
    "....1222222.....",
    "...11111111.....",
    "..1222222222....",
    ".122332222223...",
    ".1233322222233..",
    ".12333222222.33.",
    ".1233322222...33",
    ".122332222.....3",
    "..12222222......",
    "...1111111......",
    "...1222221......",
    "...1222221......",
    "...122.122......",
    "...122.122......",
    "...122.122......",
    "...12...12......",
    "...12...12......",
    "...12...12......",
    "..112..112......",
    ".1112.1112......",
    ".1111.1111......",
    "................",
], 16, 32)
KNIGHT_1 = grid(KNIGHT_0[:20] + [
    "...122..122.....",
    "...122...122....",
    "..122....122....",
    "..12......12....",
    "..12......12....",
    ".12.......12....",
    ".112......112...",
    "1112......1112..",
    "1111......1111..",
    "................",
], 16, 32)

# Candle on a sconce, 8x16, two flickers. Fire palette: 1 red, 2 orange, 3 white.
CANDLE_0 = grid([
    "...2....",
    "..232...",
    "..232...",
    "...1....",
    "..333...",
    "..333...",
    "..333...",
    "..333...",
    "..333...",
    ".11111..",
    "..111...",
    "...1....",
    "...1....",
    "...1....",
    "..111...",
], 8, 16)
CANDLE_1 = grid([
    "....2...",
    "...22...",
    "..232...",
    "...1....",
] + CANDLE_0[4:], 8, 16)
HEART = grid([
    ".11.11..",
    "1311111.",
    "1111111.",
    ".11111..",
    "..111...",
    "...1....",
], 8, 8)
BIG_HEART = grid([
    "................",
    "..1111....1111..",
    ".113311..111111.",
    "11333111111111111"[:16],
    "1133111111111111",
    "1111111111111111",
    ".11111111111111.",
    "..111111111111..",
    "...1111111111...",
    "....11111111....",
    ".....111111.....",
    "......1111......",
    ".......11.......",
], 16, 16)
DAGGER = grid([
    "................",
    "..1.............",
    ".1121111111111..",
    "1112333333333331",
    ".1121111111111..",
    "..1.............",
], 16, 8)
PUFF_0 = grid([
    "................",
    "......2.........",
    ".....22...2.....",
    "....232..22.....",
    "....2332.232....",
    "...23332.2332...",
    "...233322333....",
    "..2333333332....",
    "..23333333332...",
    "..2333333333....",
    "...233333332....",
    "....2222222.....",
], 16, 16)
PUFF_1 = grid([
    "................",
    "................",
    "........1.......",
    "....1..11...1...",
    "...11..121..11..",
    "...121.1221.121.",
    "..12211222112221",
    "..122222222222..",
    "...1222222221...",
    "....11222211....",
    "......1111......",
], 16, 16)

# Vesper's frames, in the order of her sheet (assets/sprites/hero.png, 16x32 cells).
HERO = [
    ('stand', hero(TORSO, LEGS)),
    ('walk0', hero(TORSO_SWING_F, LEGS_A)),
    ('walk1', hero(TORSO, LEGS_PASS_F, lift=1)),
    ('walk2', hero(TORSO_SWING_B, LEGS_A)),
    ('walk3', hero(TORSO, LEGS_PASS_B, lift=1)),
    ('jump', hero(TORSO, TUCK, 0)),
    ('crouch', crouch(TORSO)),
    ('whipback', hero(TORSO_BACK, LEGS_LUNGE)),
    ('whipfore', hero(TORSO_FORE, LEGS_LUNGE)),
    ('cwhipback', crouch(TORSO_BACK)),
    ('cwhipfore', crouch(TORSO_FORE)),
    ('hurt', HURT),
]

# The boss: a giant bat, 48x24, two flaps. Fire palette: 1 red membrane,
# 2 orange bones and fur, 3 white eyes and fangs. Drawn as the left half,
# mirrored.
def giant_bat(up):
    half = [[0] * 24 for _ in range(24)]
    import math
    for y in range(24):
        for x in range(24):
            # Body (right edge of the half is the centre line).
            if (x - 23.5) ** 2 / 36 + (y - 13) ** 2 / 64 <= 1:
                half[y][x] = 2 if (x + y) % 3 else 1
            # Ears.
            if 18 <= x <= 20 and 2 <= y <= 6 and x - 18 >= (6 - y) // 2:
                half[y][x] = 1
    # Wing: a membrane between the arm and three finger bones, scalloped below.
    lift = -7 if up else 5
    for x in range(0, 19):
        t = (18 - x) / 18
        top = 9 + lift * t - 3 * math.sin(t * math.pi)
        bottom = 15 + lift * t + 3 * abs(math.sin(t * math.pi * 3))
        for y in range(24):
            if top <= y <= bottom:
                half[y][x] = 1
            if abs(y - top) < 0.8:
                half[y][x] = 2
    for fx in (3, 8, 13):
        t = (18 - fx) / 18
        top = 9 + lift * t - 3 * math.sin(t * math.pi)
        for y in range(24):
            if top <= y <= top + 6 + 3 * abs(math.sin(t * math.pi * 3)) and half[y][fx] == 1:
                half[y][fx] = 2
    for (x, y) in ((20, 11), (21, 11), (21, 12)):
        half[y][x] = 3
    for (x, y) in ((22, 17), (22, 18)):
        half[y][x] = 3
    rows = [''.join(str(c) for c in r) + ''.join(str(c) for c in reversed(r)) for r in half]
    return grid([r.replace('0', '.') for r in rows], 48, 24)


SPRITES = [
    # name, art, palette
    ('gbat0', giant_bat(True), 3), ('gbat1', giant_bat(False), 3),
    ('lashback', WHIP_BACK, 2),
    ('lash', WHIP_OUT, 2),
    ('ghoul0', GHOUL_0, 1), ('ghoul1', GHOUL_1, 1),
    ('bat0', BAT_0, 2), ('bat1', BAT_1, 2),
    ('knight0', KNIGHT_0, 2), ('knight1', KNIGHT_1, 2),
    ('candle0', CANDLE_0, 3), ('candle1', CANDLE_1, 3),
    ('heart', HEART, 3), ('bigheart', BIG_HEART, 3),
    ('dagger', DAGGER, 2),
    ('puff0', PUFF_0, 3), ('puff1', PUFF_1, 3),
]


def rust():
    return '\n'.join([
        '// Generated by tools/art.py: the palettes. The art is in assets/sprites/.', '',
        f'pub const BG_PALETTES: [[u8; 4]; 4] = {fmt_pal(BG)};',
        f'pub const SPRITE_PALETTES: [[u8; 4]; 4] = {fmt_pal(SPR)};', '']) + '\n'


def fmt_pal(p):
    return '[' + ', '.join('[' + ', '.join(f'0x{c:02X}' for c in row) + ']' for row in p) + ']'


NES = None


def preview(path):
    load_nes()
    S = 4
    # Sprites in a row on dark grey, metatiles in a grid below.
    W = 640
    items = []
    x = 4
    y = 4
    rowh = 0
    for name, art, pal in [(n, a, 0) for n, a in HERO] + SPRITES:
        w, h = len(art[0]), len(art)
        if x + w > W // S:
            x = 4
            y += rowh + 4
            rowh = 0
        items.append((x, y, art, SPR[pal]))
        x += w + 4
        rowh = max(rowh, h)
    y += rowh + 8
    x = 4
    for ch, pal, fn in METATILES:
        if x + 16 > W // S:
            x = 4
            y += 20
        items.append((x, y, tile(fn), BG[pal]))
        x += 18
    H = (y + 24) * S
    img = [[(24, 24, 24)] * W for _ in range(H)]
    for (ox, oy, art, pal) in items:
        for yy, row in enumerate(art):
            for xx, ch in enumerate(row):
                if ch in '123' or (ch == '0' and False):
                    c = NES[pal[int(ch)]]
                elif ch == '0':
                    c = (0, 0, 0)
                else:
                    continue
                for a in range(S):
                    for b in range(S):
                        py, px = (oy + yy) * S + a, (ox + xx) * S + b
                        if py < H and px < W:
                            img[py][px] = c
    raw = b''.join(b'\x00' + bytes(v for p in r for v in p) for r in img)
    def chunk(t, d): return struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t + d) & 0xffffffff)
    open(path, 'wb').write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', W, H, 8, 2, 0, 0, 0))
                           + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b''))


def load_nes():
    global NES
    import re
    src = open(os.path.join(HERE, '../../../engines/nes-ppu/src/lib.rs')).read()
    body = src[src.index('= [', src.index('pub const NES_PALETTE')):]
    body = body[:body.index('];')]
    NES = [tuple(int(v, 16) for v in m) for m in re.findall(r'\[0x(..), 0x(..), 0x(..)\]', body)]


# ── Sheets ──
# The HUD's tiles: bar cells (player orange, enemy red), a heart, a dagger.
def bar_cell(halves, colour):
    rows = []
    for y in range(8):
        r = ''
        for x in range(8):
            inside = x % 4 != 3
            if 1 <= y <= 6 and inside and x // 4 < halves:
                r += str(colour)
            elif inside and y in (1, 6):
                r += '3'
            else:
                r += '0'
        rows.append(r)
    return rows


HUD = [(f'bar{c}{h}', bar_cell(h, c)) for c in (2, 1) for h in (0, 1, 2)] + [
    ('heart', ["00000000", "01101100", "13111110", "11111110", "01111100", "00111000", "00010000", "00000000"]),
    ('dagger0', ["00000000", "00000000", "00000001", "00000013", "33333333", "00000013", "00000001", "00000000"]),
    ('dagger1', ["00000000", "00000000", "00000000", "33331000", "33333100", "33331000", "00000000", "00000000"]),
]

SPRITE_SHEETS = [
    # file, frames: (name, art, palette)
    ('hero.png', [(n, a, 0) for n, a in HERO]),
    ('enemies.png', [('ghoul0', GHOUL_0, 1), ('ghoul1', GHOUL_1, 1), ('knight0', KNIGHT_0, 2), ('knight1', KNIGHT_1, 2),
                     ('bat0', BAT_0, 2), ('bat1', BAT_1, 2)]),
    ('boss.png', [('gbat0', giant_bat(True), 3), ('gbat1', giant_bat(False), 3)]),
    ('items.png', [('lash', WHIP_OUT, 2), ('lashback', WHIP_BACK, 2), ('dagger', DAGGER, 2), ('candle0', CANDLE_0, 3),
                   ('candle1', CANDLE_1, 3), ('heart', HEART, 3), ('bigheart', BIG_HEART, 3), ('puff0', PUFF_0, 3), ('puff1', PUFF_1, 3)]),
]


def pack(frames, width):
    """Shelf packing: frames left to right, a new row when the next would
    pass `width`. Returns (x, y) for each, and the sheet's size."""
    x = y = shelf = 0
    spots = []
    sheet_w = 0
    for _, art, _ in frames:
        w, h = len(art[0]), len(art)
        if x + w > width and x > 0:
            x, y, shelf = 0, y + shelf, 0
        spots.append((x, y))
        x += w
        shelf = max(shelf, h)
        sheet_w = max(sheet_w, x)
    return spots, sheet_w, y + shelf


def write_png(path, w, h, pixels):
    """pixels: dict (x, y) -> (r, g, b, a); the rest transparent."""
    rows = []
    for y in range(h):
        r = bytearray(b'\x00')
        for x in range(w):
            r += bytes(pixels.get((x, y), (0, 0, 0, 0)))
        rows.append(bytes(r))
    def chunk(t, d): return struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t + d) & 0xffffffff)
    open(path, 'wb').write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 6, 0, 0, 0))
                           + chunk(b'IDAT', zlib.compress(b''.join(rows), 9)) + chunk(b'IEND', b''))


def export(force):
    load_nes()
    out = os.path.join(HERE, '../assets/sprites')
    os.makedirs(out, exist_ok=True)
    manifest = ['// The sprite sheets and where each frame is in them. Written by',
                '// tools/art.py --export; edit it when you move frames in a sheet.', '',
                'use crate::sheets_types::{Frame, Tiles};', '']
    entries = []
    for file, frames in SPRITE_SHEETS:
        width = 16 * len(frames) if file == 'hero.png' else 128
        spots, w, h = pack(frames, width)
        px = {}
        for (name, art, pal), (ox, oy) in zip(frames, spots):
            for y, row in enumerate(art):
                for x, ch in enumerate(row):
                    if ch in '123':
                        px[(ox + x, oy + y)] = NES[SPR[pal][int(ch)]] + (255,)
        path = os.path.join(out, file)
        if force or not os.path.exists(path):
            write_png(path, w, h, px)
            print('wrote', path)
        const = file.split('.')[0].upper()
        manifest.append(f'pub const {const}: &[Frame] = &[')
        for (name, art, pal), (ox, oy) in zip(frames, spots):
            manifest.append(f'    Frame {{ name: "{name}", x: {ox}, y: {oy}, w: {len(art[0])}, h: {len(art)}, palette: {pal} }},')
        manifest.append('];')
        entries.append((file, const))
    # Background tiles: 16x16 metatiles, 8 a row, each in its own palette (the
    # backdrop drawn black). The HUD's 8x8 tiles in the HUD palette (3).
    cols = 8
    px = {}
    for i, (ch, pal, fn) in enumerate(METATILES):
        ox, oy = (i % cols) * 16, (i // cols) * 16
        for y, row in enumerate(tile(fn)):
            for x, c in enumerate(row):
                px[(ox + x, oy + y)] = NES[BG[pal][int(c)]] + (255,)
    path = os.path.join(out, 'tiles.png')
    if force or not os.path.exists(path):
        write_png(path, cols * 16, ((len(METATILES) + cols - 1) // cols) * 16, px)
        print('wrote', path)
    manifest.append('/// Background tiles: map character and palette, 16×16 each, 8 a row.')
    manifest.append('pub const TILES: Tiles = Tiles { size: 16, per_row: 8, cells: &[')
    for ch, pal, fn in METATILES:
        c = f"'{ch}'"
        manifest.append(f'    ({c}, {pal}),')
    manifest.append('] };')
    px = {}
    for i, (name, rows) in enumerate(HUD):
        for y, row in enumerate(rows):
            for x, c in enumerate(row):
                px[(i * 8 + x, y)] = NES[BG[3][int(c)]] + (255,)
    path = os.path.join(out, 'hud.png')
    if force or not os.path.exists(path):
        write_png(path, 8 * len(HUD), 8, px)
        print('wrote', path)
    manifest.append('/// HUD tiles, 8×8 in a row, in the HUD palette (3).')
    manifest.append('pub const HUD: &[&str] = &[' + ', '.join(f'"{n}"' for n, _ in HUD) + '];')
    mpath = os.path.join(HERE, '../gpu/src/sheets.rs')
    if force or not os.path.exists(mpath):
        open(mpath, 'w').write('\n'.join(manifest) + '\n')
        print('wrote', mpath)


if __name__ == '__main__':
    import sys
    export('--export' in sys.argv)
    os.makedirs(os.path.join(HERE, '../gpu/src'), exist_ok=True)
    open(os.path.join(HERE, '../gpu/src/data.rs'), 'w').write(rust())
    preview(os.path.join(HERE, 'preview.png'))
    print('ok')
