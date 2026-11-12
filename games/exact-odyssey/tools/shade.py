# Usage: python3 tools/shade.py gpu/src/art.rs out.rs (outfit shading).
# Shading for the party: outfit (3) and skin (2) pixels on the back side
# (right, away from the light; they face left) and along the hem get a
# checker of the dark colour, as FF3 shades its clothes.
import re, sys
src = open(sys.argv[1]).read()
NAMES = [f'{c}_{p}' for c in ('THUNDA', 'TOOTH', 'CHOCOLAT') for p in ('IDLE', 'STRIKE', 'CAST', 'WIN')]
for n in NAMES:
    m = re.search(r'const %s: &str = "\n(.*?)";' % n, src, re.S)
    g = [list(r) for r in m.group(1).split('\n')]
    H, W = len(g), len(g[0])
    at = lambda x, y: g[y][x] if 0 <= x < W and 0 <= y < H else '.'
    orig = [r[:] for r in g]
    o = lambda x, y: orig[y][x] if 0 <= x < W and 0 <= y < H else '.'
    for y in range(14, H):
        for x in range(W):
            if orig[y][x] != '3':
                continue
            back = any(o(x + k, y) in '.1' for k in (1, 2))
            hem = o(x, y + 1) in '.1'
            if back and not hem and (x + y) % 2 == 0 and o(x - 1, y) == '3':
                g[y][x] = '1'
            elif hem and (x % 2 == 0) and o(x, y - 1) == '3':
                g[y][x] = '1'
    src = src.replace(m.group(0), 'const %s: &str = "\n%s";' % (n, '\n'.join(''.join(r) for r in g)))
open(sys.argv[2], 'w').write(src)
