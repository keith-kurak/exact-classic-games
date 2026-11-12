# Exactulator's demo cartridge: an original mapper-0 NES program, assembled
# here, so the app (and its tests) have a ROM that is ours to ship. It draws a
# title in the shared nes-ppu font (tiles at their ASCII codes), and a star
# sprite the D-pad moves; A turns it green, B gold, Start resets it. Each
# press also plays a sound on the APU: A a pulse beep, B a burst of noise,
# Start a rising sweep.
# Run: python3 tools/democart.py   (writes assets/demo.nes)
import os, re

HERE = os.path.dirname(os.path.abspath(__file__))

# ── A tiny two-pass assembler: (mnemonic, mode, operand) lines and labels ──
OPS = {
    ('lda', 'imm'): 0xA9, ('lda', 'zp'): 0xA5, ('lda', 'abs'): 0xAD, ('lda', 'absx'): 0xBD,
    ('sta', 'zp'): 0x85, ('sta', 'abs'): 0x8D, ('sta', 'absx'): 0x9D,
    ('ldx', 'imm'): 0xA2, ('ldy', 'imm'): 0xA0, ('stx', 'abs'): 0x8E,
    ('inx', 'imp'): 0xE8, ('dex', 'imp'): 0xCA, ('dey', 'imp'): 0x88, ('iny', 'imp'): 0xC8,
    ('cpx', 'imm'): 0xE0, ('cmp', 'imm'): 0xC9,
    ('bne', 'rel'): 0xD0, ('beq', 'rel'): 0xF0, ('bpl', 'rel'): 0x10, ('bcc', 'rel'): 0x90,
    ('jmp', 'abs'): 0x4C, ('bit', 'abs'): 0x2C, ('and', 'imm'): 0x29, ('lsr', 'acc'): 0x4A,
    ('rol', 'zp'): 0x26, ('inc', 'abs'): 0xEE, ('dec', 'abs'): 0xCE, ('txs', 'imp'): 0x9A,
    ('sei', 'imp'): 0x78, ('cld', 'imp'): 0xD8, ('rti', 'imp'): 0x40, ('pha', 'imp'): 0x48,
    ('pla', 'imp'): 0x68, ('txa', 'imp'): 0x8A, ('eor', 'imm'): 0x49, ('and', 'zp'): 0x25, ('tax', 'imp'): 0xAA, ('tya', 'imp'): 0x98, ('tay', 'imp'): 0xA8,
}
SIZE = {'imp': 1, 'acc': 1, 'imm': 2, 'zp': 2, 'rel': 2, 'abs': 3, 'absx': 3}


def assemble(lines, origin):
    labels, pc = {}, origin
    for l in lines:
        if isinstance(l, str):
            labels[l] = pc
        elif l[0] == '.byte':
            pc += len(l[1])
        else:
            pc += SIZE[l[1]]
    out, pc = bytearray(), origin
    for l in lines:
        if isinstance(l, str):
            continue
        if l[0] == '.byte':
            out += bytes(l[1]); pc += len(l[1]); continue
        op, mode, arg = l[0], l[1], (l[2] if len(l) > 2 else None)
        val = labels[arg] if isinstance(arg, str) else arg
        out.append(OPS[(op, mode)])
        if mode == 'rel':
            off = val - (pc + 2)
            assert -128 <= off <= 127, (l, off)
            out.append(off & 0xFF)
        elif SIZE[mode] == 2:
            out.append(val & 0xFF)
        elif SIZE[mode] == 3:
            out += bytes([val & 0xFF, val >> 8])
        pc += SIZE[mode]
    return out, labels


def text(s):
    return [ord(c) for c in s] + [0]


PALETTE = [0x0F, 0x30, 0x21, 0x11, 0x0F, 0x38, 0x27, 0x17, 0x0F, 0x2A, 0x1A, 0x0A, 0x0F, 0x10, 0x00, 0x2D,
           0x0F, 0x38, 0x27, 0x30, 0x0F, 0x2A, 0x1A, 0x30, 0x0F, 0x28, 0x18, 0x30, 0x0F, 0x16, 0x06, 0x30]


def ppu_addr(a):
    return [('lda', 'imm', a >> 8), ('sta', 'abs', 0x2006), ('lda', 'imm', a & 0xFF), ('sta', 'abs', 0x2006)]


def print_at(row, col, label, palette_line=None):
    return ppu_addr(0x2000 + row * 32 + col) + [
        ('ldx', 'imm', 0), f'{label}_loop', ('lda', 'absx', label), ('beq', 'rel', f'{label}_done'),
        ('sta', 'abs', 0x2007), ('inx', 'imp'), ('jmp', 'abs', f'{label}_loop'), f'{label}_done']


BUTTONS, PREVIOUS, PRESSED, STAR = 0x10, 0x11, 0x12, 0x0200
code = [
    'reset',
    ('sei', 'imp'), ('cld', 'imp'), ('ldx', 'imm', 0xFF), ('txs', 'imp'),
    ('ldx', 'imm', 0), ('stx', 'abs', 0x2000), ('stx', 'abs', 0x2001),
    # Sound: pulses, triangle and noise on; the 4-step frame counter.
    ('lda', 'imm', 0x0F), ('sta', 'abs', 0x4015), ('lda', 'imm', 0x40), ('sta', 'abs', 0x4017),
    'wait1', ('bit', 'abs', 0x2002), ('bpl', 'rel', 'wait1'),
    'wait2', ('bit', 'abs', 0x2002), ('bpl', 'rel', 'wait2'),
    # The palette.
    *ppu_addr(0x3F00), ('ldx', 'imm', 0),
    'pal', ('lda', 'absx', 'palette'), ('sta', 'abs', 0x2007), ('inx', 'imp'), ('cpx', 'imm', 32), ('bne', 'rel', 'pal'),
    # Clear name table 0 and its attributes (1 KB of tile 0).
    *ppu_addr(0x2000), ('lda', 'imm', 0), ('ldy', 'imm', 4), ('ldx', 'imm', 0),
    'clr', ('sta', 'abs', 0x2007), ('inx', 'imp'), ('bne', 'rel', 'clr'), ('dey', 'imp'), ('bne', 'rel', 'clr'),
    # A band of attribute palette 1 behind the title (rows 4-7: attribute row 1).
    *ppu_addr(0x23C8), ('lda', 'imm', 0x55), ('ldx', 'imm', 8),
    'att', ('sta', 'abs', 0x2007), ('dex', 'imp'), ('bne', 'rel', 'att'),
    *print_at(5, 10, 'title'),
    *print_at(6, 7, 'subtitle'),
    *print_at(14, 9, 'hint1'),
    *print_at(16, 9, 'hint2'),
    *print_at(18, 9, 'hint3'),
    *print_at(20, 5, 'hint4'),
    *print_at(24, 6, 'credit'),
    # Hide every sprite, then place the star.
    ('lda', 'imm', 0xFF), ('ldx', 'imm', 0),
    'hide', ('sta', 'absx', STAR), ('inx', 'imp'), ('bne', 'rel', 'hide'),
    'center', ('lda', 'imm', 0x6F), ('sta', 'abs', STAR), ('lda', 'imm', 0x01), ('sta', 'abs', STAR + 1),
    ('lda', 'imm', 0), ('sta', 'abs', STAR + 2), ('lda', 'imm', 0x7C), ('sta', 'abs', STAR + 3),
    # Scroll home; NMI on; background and sprites on, left columns too.
    ('lda', 'imm', 0), ('sta', 'abs', 0x2005), ('sta', 'abs', 0x2005),
    ('lda', 'imm', 0x80), ('sta', 'abs', 0x2000), ('lda', 'imm', 0x1E), ('sta', 'abs', 0x2001),
    'forever', ('jmp', 'abs', 'forever'),

    'nmi',
    ('pha', 'imp'), ('txa', 'imp'), ('pha', 'imp'),
    # Read the pad: A, B, Select, Start, Up, Down, Left, Right into BUTTONS
    # (A ends up in bit 7).
    ('lda', 'imm', 1), ('sta', 'abs', 0x4016), ('lda', 'imm', 0), ('sta', 'abs', 0x4016), ('ldx', 'imm', 8),
    'pad', ('lda', 'abs', 0x4016), ('lsr', 'acc'), ('rol', 'zp', BUTTONS), ('dex', 'imp'), ('bne', 'rel', 'pad'),
    # Buttons new this frame: held now and not before.
    ('lda', 'zp', PREVIOUS), ('eor', 'imm', 0xFF), ('and', 'zp', BUTTONS), ('sta', 'zp', PRESSED),
    ('lda', 'zp', BUTTONS), ('sta', 'zp', PREVIOUS),
    # A: a beep on pulse 1 (50%, decaying envelope, a C5 that the length counter ends).
    ('lda', 'zp', PRESSED), ('and', 'imm', 0x80), ('beq', 'rel', 'nobeep'),
    ('lda', 'imm', 0x84), ('sta', 'abs', 0x4000), ('lda', 'imm', 0x08), ('sta', 'abs', 0x4001),
    ('lda', 'imm', 0xD5), ('sta', 'abs', 0x4002), ('lda', 'imm', 0x08), ('sta', 'abs', 0x4003), 'nobeep',
    # B: a burst of noise, its envelope decaying.
    ('lda', 'zp', PRESSED), ('and', 'imm', 0x40), ('beq', 'rel', 'nohiss'),
    ('lda', 'imm', 0x03), ('sta', 'abs', 0x400C), ('lda', 'imm', 0x06), ('sta', 'abs', 0x400E),
    ('lda', 'imm', 0x08), ('sta', 'abs', 0x400F), 'nohiss',
    # Start: a sweep up on pulse 2.
    ('lda', 'zp', PRESSED), ('and', 'imm', 0x10), ('beq', 'rel', 'nosweep'),
    ('lda', 'imm', 0x9A), ('sta', 'abs', 0x4004), ('lda', 'imm', 0xAB), ('sta', 'abs', 0x4005),
    ('lda', 'imm', 0x40), ('sta', 'abs', 0x4006), ('lda', 'imm', 0x99), ('sta', 'abs', 0x4007), 'nosweep',
    ('lda', 'zp', BUTTONS), ('and', 'imm', 0x01), ('beq', 'rel', 'noright'), ('inc', 'abs', STAR + 3), 'noright',
    ('lda', 'zp', BUTTONS), ('and', 'imm', 0x02), ('beq', 'rel', 'noleft'), ('dec', 'abs', STAR + 3), 'noleft',
    ('lda', 'zp', BUTTONS), ('and', 'imm', 0x04), ('beq', 'rel', 'nodown'), ('inc', 'abs', STAR), 'nodown',
    ('lda', 'zp', BUTTONS), ('and', 'imm', 0x08), ('beq', 'rel', 'noup'), ('dec', 'abs', STAR), 'noup',
    # A: green (palette 1), B: gold (palette 2), else palette 0.
    ('ldx', 'imm', 0),
    ('lda', 'zp', BUTTONS), ('and', 'imm', 0x80), ('beq', 'rel', 'noa'), ('ldx', 'imm', 1), 'noa',
    ('lda', 'zp', BUTTONS), ('and', 'imm', 0x40), ('beq', 'rel', 'nob'), ('ldx', 'imm', 2), 'nob',
    ('txa', 'imp'), ('sta', 'abs', STAR + 2),
    # Start: back to the middle.
    ('lda', 'zp', BUTTONS), ('and', 'imm', 0x10), ('beq', 'rel', 'nostart'),
    ('lda', 'imm', 0x6F), ('sta', 'abs', STAR), ('lda', 'imm', 0x7C), ('sta', 'abs', STAR + 3), 'nostart',
    # Sprites up by DMA, scroll home.
    ('lda', 'imm', 0), ('sta', 'abs', 0x2003), ('lda', 'imm', 0x02), ('sta', 'abs', 0x4014),
    ('lda', 'imm', 0), ('sta', 'abs', 0x2005), ('sta', 'abs', 0x2005),
    ('pla', 'imp'), ('tax', 'imp'), ('pla', 'imp'),
    'irq', ('rti', 'imp'),

    'palette', ('.byte', PALETTE),
    'title', ('.byte', text('EXACTULATOR')),
    'subtitle', ('.byte', text('THE EXACT NES EMULATOR')),
    'hint1', ('.byte', text('D-PAD   MOVE')),
    'hint2', ('.byte', text('A B     COLOUR')),
    'hint3', ('.byte', text('START   CENTRE')),
    'hint4', ('.byte', text('EACH ONE MAKES A SOUND')),
    'credit', ('.byte', text('A DEMO CART FOR MAPPER 0')),
]


def font():
    src = open(os.path.join(HERE, '../../../engines/nes-ppu/src/font.rs')).read()
    glyphs = {}
    for ch, rows in re.findall(r"\('(.)', &\[(.*?)\]\)", src):
        glyphs[ch] = re.findall(r'"([#.]+)"', rows)
    return glyphs


def tile(rows):
    """8 rows of 8 colour numbers as 16 bytes of CHR (low plane, then high)."""
    lo = bytes(sum(((r[x] & 1) << (7 - x)) for x in range(8)) for r in rows)
    hi = bytes(sum((((r[x] >> 1) & 1) << (7 - x)) for x in range(8)) for r in rows)
    return lo + hi


def main():
    prg, labels = assemble(code, 0xC000)
    assert len(prg) <= 0x3FFA, len(prg)
    prg = prg + bytes(0x4000 - len(prg))
    prg = bytearray(prg)
    for at, name in ((0x3FFA, 'nmi'), (0x3FFC, 'reset'), (0x3FFE, 'irq')):
        prg[at] = labels[name] & 0xFF
        prg[at + 1] = labels[name] >> 8
    chr_ = bytearray(0x2000)
    for ch, rows in font().items():
        grid = [[1 if c == '#' else 0 for c in (r + '........')[:8]] for r in rows] + [[0] * 8] * (8 - len(rows))
        chr_[ord(ch) * 16: ord(ch) * 16 + 16] = tile(grid[:8])
    star = ["...3....", "...3....", "..323...", "3332333.", ".32123..", "..323...", ".3.3.3..", "3.....3."]
    chr_[16:32] = tile([[int(c) if c != '.' else 0 for c in r] for r in star])
    header = b'NES\x1A' + bytes([1, 1, 0, 0]) + bytes(8)
    out = os.path.join(HERE, '../assets/demo.nes')
    os.makedirs(os.path.dirname(out), exist_ok=True)
    open(out, 'wb').write(header + prg + chr_)
    print('wrote', out, len(header + prg + chr_), 'bytes;', len(code), 'lines')


main()
