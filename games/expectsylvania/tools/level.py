# The Gatehouse, stage 1: 64x13 metatiles (16x16), under a 2-metatile HUD.
# Writes the map once, for both level.contract and gpu/src/level.rs.
W, H = 64, 13
g = [[' '] * W for _ in range(H)]
def put(c, r, ch): g[r][c] = ch
# ── Courtyard, columns 0-21: night sky, stars, moon, a far castle, dead
# trees, an iron fence, flagstones.
for c in range(22):
    for r in range(9):
        if (c * 7 + r * 13) % 11 == 0 and r < 7:
            put(c, r, '`')
for c, r, ch in ((5, 1, 'm'), (6, 1, 'n'), (5, 2, 'o'), (6, 2, 'q')):
    put(c, r, ch)
for c in range(11, 22):
    put(c, 4, 'K' if c % 2 else 'k')
    for r in range(5, 9):
        put(c, r, 'j' if (c in (13, 17, 20) and r == 6) else 'k')
for c in range(22):
    put(c, 9, 'F'); put(c, 10, 'f')
for tc in (2, 17):
    for c in (tc - 1, tc, tc + 1):
        put(c, 5, 'T'); put(c, 6, 'T')
    for r in (7, 8, 9, 10):
        put(tc, r, 't')
# ── The gate, columns 22-25: the castle's front wall and its arch.
for c in range(22, 26):
    for r in range(0, 11):
        put(c, r, 'W')
for c in (23, 24):
    put(c, 5, 'A')
    for r in range(6, 11):
        put(c, r, 'a')
# ── The hall, columns 26-63: a valance of curtains, brick walls, pillars,
# tall moonlit windows, hanging drapes, a balcony on stairs, the door.
for c in range(26, 64):
    put(c, 0, 'c')
    for r in range(1, 11):
        put(c, r, 'b')
for c in (29, 35, 47, 59):
    put(c, 2, 'w')
    for r in (3, 4, 5):
        put(c, r, 'v')
for c in (27, 32, 42, 50, 57):
    for r in range(1, 11):
        put(c, r, 'P')
for c in (30, 31, 52, 53):
    for r in (1, 2, 3):
        put(c, r, 'C')
for c in range(39, 53):
    put(c, 7, '=')
for i, c in enumerate(range(35, 39)):
    put(c, 10 - i, '/')
for i, c in enumerate(range(53, 57)):
    put(c, 7 + i, 'z')
for c, r, ch in ((61, 9, 'D'), (62, 9, 'E'), (61, 10, 'd'), (62, 10, 'e')):
    put(c, r, ch)
for r in range(0, 11):
    put(63, r, '#')
for c in range(W):
    put(c, 11, '#'); put(c, 12, '#')
rows = [''.join(r) for r in g]
# Write both copies: the rules' (level.contract) and the picture's (gpu/src/level.rs).
import os, re
here = os.path.dirname(os.path.abspath(__file__))
lc = os.path.join(here, '../level.contract')
src = open(lc).read()
block = 'fn levelMap(): string[] = [\n' + ',\n'.join(f'  "{r}"' for r in rows) + '\n]'
src = re.sub(r'fn levelMap\(\): string\[\] = \[\n.*?\n\]', lambda m: block, src, flags=re.S)
open(lc, 'w').write(src)
open(os.path.join(here, '../gpu/src/level.rs'), 'w').write(
    '// The Gatehouse, stage 1: 64x13 metatiles under the HUD. Must match\n'
    '// level.contract (both written by tools/level.py).\n'
    'pub const LEVEL: [&str; 13] = [\n' + ',\n'.join(f'    "{r}"' for r in rows) + ',\n];\n')
print('ok')
