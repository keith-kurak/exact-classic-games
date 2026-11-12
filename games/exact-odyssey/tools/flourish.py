# Usage: python3 tools/flourish.py gpu/src/art.rs out.rs preview.png (outlines, eyes, hair shine).
import re, sys, zlib, struct
ART = sys.argv[1]
src = open(ART).read()
NAMES = [f'{c}_{p}' for c in ('THUNDA', 'TOOTH', 'CHOCOLAT') for p in ('IDLE', 'STRIKE', 'CAST', 'WIN')]
SHINE = {'THUNDA': '3', 'TOOTH': '3', 'CHOCOLAT': '2'}

def get(name):
    m = re.search(r'const %s: &str = "\n(.*?)";' % name, src, re.S)
    return m, [list(r) for r in m.group(1).split('\n')]

def flourish(name, g):
    H, W = len(g), len(g[0])
    at = lambda x, y: g[y][x] if 0 <= x < W and 0 <= y < H else '.'
    # Skin rows in the head: the face, facing left.
    face = [(x, y) for y in range(4, 15) for x in range(W) if g[y][x] == '2']
    # Hair: colour-1 pixels above the face's lowest row with hair on both sides.
    if face:
        fy = min(y for _, y in face)
        fx = min(x for x, y in face if y in (fy + 2, fy + 3)) if any(y in (fy + 2, fy + 3) for _, y in face) else None
        # Hair shine: one curved highlight on the front of the crown (light
        # from the top left), two rows: four pixels, then three.
        hair_rows = [y for y in range(0, fy) if any(g[y][x] == '1' for x in range(W))]
        if hair_rows:
            ty = hair_rows[0]
            for dy, (off, n) in ((2, (2, 4)), (3, (1, 3))):
                y = ty + dy
                if y >= fy:
                    continue
                inner = [x for x in range(W) if g[y][x] == '1' and all(at(x + a, y + b) in '1' + SHINE[name.split('_')[0]]
                         for a, b in ((1, 0), (-1, 0), (0, 1), (0, -1)))]
                if inner:
                    x0 = inner[0]
                    for x in range(x0 + off, x0 + off + n):
                        if x in inner:
                            g[y][x] = SHINE[name.split('_')[0]]
        # Eyes: dark and two pixels tall, a pixel in from the face's front.
        if fx is not None:
            ex, ey = fx + 2, fy + 2
            for y in (ey, ey + 1):
                if at(ex, y) in '12' and at(ex - 1, y) in '12':
                    g[y][ex] = '1'
                    g[y][ex - 1] = '2'
    # Outline: transparent pixels touching skin or clothes turn dark.
    add = [(x, y) for y in range(H) for x in range(W) if g[y][x] == '.'
           and any(at(x + a, y + b) in '23' for a, b in ((1, 0), (-1, 0), (0, 1), (0, -1)))]
    for x, y in add:
        g[y][x] = '1'
    return g

out = src
grids = {}
for n in NAMES:
    m, g = get(n)
    g = flourish(n, g)
    grids[n] = g
    out = out.replace(m.group(0), 'const %s: &str = "\n%s";' % (n, '\n'.join(''.join(r) for r in g)))
open(sys.argv[2], 'w').write(out)

# Preview, 6x, on black.
NES = {0x17: (0x9D, 0x54, 0), 0x37: (0xFF, 0xD5, 0x9A), 0x30: (255, 255, 255), 0x01: (0, 0x1F, 0xB2),
       0x12: (0x4B, 0x30, 0xFF), 0x16: (0xC7, 0x2E, 0), 0x29: (0x7A, 0xE7, 0)}
PAL = {'THUNDA': [0x17, 0x37, 0x30], 'TOOTH': [0x01, 0x37, 0x12], 'CHOCOLAT': [0x16, 0x37, 0x29]}
S = 6
cols = 4
Wd, Hd = (24 * cols + 4 * (cols - 1)) * S, (32 * 3 + 8) * S
rows = []
for y in range(Hd):
    row = b'\x00'
    for x in range(Wd):
        c = (0, 0, 0)
        gx, gy = x // S, y // S
        ci, ri = gx // 28, gy // 36
        lx, ly = gx % 28, gy % 36
        if ci < cols and ri < 3 and lx < 24 and ly < 32:
            ch = ['THUNDA', 'TOOTH', 'CHOCOLAT'][ri]
            n = f'{ch}_{["IDLE","STRIKE","CAST","WIN"][ci]}'
            p = grids[n][ly][lx]
            if p in '123':
                c = NES[PAL[ch][int(p) - 1]]
        row += bytes(c)
    rows.append(row)
def chunk(t, d): return struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t + d) & 0xffffffff)
open(sys.argv[3], 'wb').write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', Wd, Hd, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(b''.join(rows))) + chunk(b'IEND', b''))
