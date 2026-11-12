//! The 2A03's 6502 core: every official opcode and the unofficial ones games
//! and nestest use (LAX, SAX, DCP, ISB, SLO, RLA, SRE, RRA, the extra NOPs
//! and SBC $EB), with cycle counts and page-crossing penalties. No decimal
//! mode: the 2A03 has none.

/// What the CPU reads and writes through.
pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, v: u8);
}

pub const C: u8 = 0x01;
pub const Z: u8 = 0x02;
pub const I: u8 = 0x04;
pub const D: u8 = 0x08;
pub const B: u8 = 0x10;
pub const U: u8 = 0x20;
pub const V: u8 = 0x40;
pub const N: u8 = 0x80;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Imp,
    Acc,
    Imm,
    Zp,
    Zpx,
    Zpy,
    Abs,
    Abx,
    Aby,
    Ind,
    Izx,
    Izy,
    Rel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Adc, And, Asl, Bcc, Bcs, Beq, Bit, Bmi, Bne, Bpl, Brk, Bvc, Bvs, Clc, Cld, Cli, Clv, Cmp, Cpx, Cpy,
    Dec, Dex, Dey, Eor, Inc, Inx, Iny, Jmp, Jsr, Lda, Ldx, Ldy, Lsr, Nop, Ora, Pha, Php, Pla, Plp, Rol,
    Ror, Rti, Rts, Sbc, Sec, Sed, Sei, Sta, Stx, Sty, Tax, Tay, Tsx, Txa, Txs, Tya,
    // Unofficial.
    Lax, Sax, Dcp, Isb, Slo, Rla, Sre, Rra, Kil,
}

/// An opcode's operation, addressing mode, base cycles, and whether
/// crossing a page costs a cycle.
pub fn decode(op: u8) -> (Op, Mode, u8, bool) {
    use Mode::*;
    use Op::*;
    match op {
        0x69 => (Adc, Imm, 2, false), 0x65 => (Adc, Zp, 3, false), 0x75 => (Adc, Zpx, 4, false), 0x6D => (Adc, Abs, 4, false),
        0x7D => (Adc, Abx, 4, true), 0x79 => (Adc, Aby, 4, true), 0x61 => (Adc, Izx, 6, false), 0x71 => (Adc, Izy, 5, true),
        0x29 => (And, Imm, 2, false), 0x25 => (And, Zp, 3, false), 0x35 => (And, Zpx, 4, false), 0x2D => (And, Abs, 4, false),
        0x3D => (And, Abx, 4, true), 0x39 => (And, Aby, 4, true), 0x21 => (And, Izx, 6, false), 0x31 => (And, Izy, 5, true),
        0x0A => (Asl, Acc, 2, false), 0x06 => (Asl, Zp, 5, false), 0x16 => (Asl, Zpx, 6, false), 0x0E => (Asl, Abs, 6, false), 0x1E => (Asl, Abx, 7, false),
        0x90 => (Bcc, Rel, 2, false), 0xB0 => (Bcs, Rel, 2, false), 0xF0 => (Beq, Rel, 2, false), 0x30 => (Bmi, Rel, 2, false),
        0xD0 => (Bne, Rel, 2, false), 0x10 => (Bpl, Rel, 2, false), 0x50 => (Bvc, Rel, 2, false), 0x70 => (Bvs, Rel, 2, false),
        0x24 => (Bit, Zp, 3, false), 0x2C => (Bit, Abs, 4, false),
        0x00 => (Brk, Imp, 7, false),
        0x18 => (Clc, Imp, 2, false), 0xD8 => (Cld, Imp, 2, false), 0x58 => (Cli, Imp, 2, false), 0xB8 => (Clv, Imp, 2, false),
        0xC9 => (Cmp, Imm, 2, false), 0xC5 => (Cmp, Zp, 3, false), 0xD5 => (Cmp, Zpx, 4, false), 0xCD => (Cmp, Abs, 4, false),
        0xDD => (Cmp, Abx, 4, true), 0xD9 => (Cmp, Aby, 4, true), 0xC1 => (Cmp, Izx, 6, false), 0xD1 => (Cmp, Izy, 5, true),
        0xE0 => (Cpx, Imm, 2, false), 0xE4 => (Cpx, Zp, 3, false), 0xEC => (Cpx, Abs, 4, false),
        0xC0 => (Cpy, Imm, 2, false), 0xC4 => (Cpy, Zp, 3, false), 0xCC => (Cpy, Abs, 4, false),
        0xC6 => (Dec, Zp, 5, false), 0xD6 => (Dec, Zpx, 6, false), 0xCE => (Dec, Abs, 6, false), 0xDE => (Dec, Abx, 7, false),
        0xCA => (Dex, Imp, 2, false), 0x88 => (Dey, Imp, 2, false),
        0x49 => (Eor, Imm, 2, false), 0x45 => (Eor, Zp, 3, false), 0x55 => (Eor, Zpx, 4, false), 0x4D => (Eor, Abs, 4, false),
        0x5D => (Eor, Abx, 4, true), 0x59 => (Eor, Aby, 4, true), 0x41 => (Eor, Izx, 6, false), 0x51 => (Eor, Izy, 5, true),
        0xE6 => (Inc, Zp, 5, false), 0xF6 => (Inc, Zpx, 6, false), 0xEE => (Inc, Abs, 6, false), 0xFE => (Inc, Abx, 7, false),
        0xE8 => (Inx, Imp, 2, false), 0xC8 => (Iny, Imp, 2, false),
        0x4C => (Jmp, Abs, 3, false), 0x6C => (Jmp, Ind, 5, false), 0x20 => (Jsr, Abs, 6, false),
        0xA9 => (Lda, Imm, 2, false), 0xA5 => (Lda, Zp, 3, false), 0xB5 => (Lda, Zpx, 4, false), 0xAD => (Lda, Abs, 4, false),
        0xBD => (Lda, Abx, 4, true), 0xB9 => (Lda, Aby, 4, true), 0xA1 => (Lda, Izx, 6, false), 0xB1 => (Lda, Izy, 5, true),
        0xA2 => (Ldx, Imm, 2, false), 0xA6 => (Ldx, Zp, 3, false), 0xB6 => (Ldx, Zpy, 4, false), 0xAE => (Ldx, Abs, 4, false), 0xBE => (Ldx, Aby, 4, true),
        0xA0 => (Ldy, Imm, 2, false), 0xA4 => (Ldy, Zp, 3, false), 0xB4 => (Ldy, Zpx, 4, false), 0xAC => (Ldy, Abs, 4, false), 0xBC => (Ldy, Abx, 4, true),
        0x4A => (Lsr, Acc, 2, false), 0x46 => (Lsr, Zp, 5, false), 0x56 => (Lsr, Zpx, 6, false), 0x4E => (Lsr, Abs, 6, false), 0x5E => (Lsr, Abx, 7, false),
        0xEA => (Nop, Imp, 2, false),
        0x09 => (Ora, Imm, 2, false), 0x05 => (Ora, Zp, 3, false), 0x15 => (Ora, Zpx, 4, false), 0x0D => (Ora, Abs, 4, false),
        0x1D => (Ora, Abx, 4, true), 0x19 => (Ora, Aby, 4, true), 0x01 => (Ora, Izx, 6, false), 0x11 => (Ora, Izy, 5, true),
        0x48 => (Pha, Imp, 3, false), 0x08 => (Php, Imp, 3, false), 0x68 => (Pla, Imp, 4, false), 0x28 => (Plp, Imp, 4, false),
        0x2A => (Rol, Acc, 2, false), 0x26 => (Rol, Zp, 5, false), 0x36 => (Rol, Zpx, 6, false), 0x2E => (Rol, Abs, 6, false), 0x3E => (Rol, Abx, 7, false),
        0x6A => (Ror, Acc, 2, false), 0x66 => (Ror, Zp, 5, false), 0x76 => (Ror, Zpx, 6, false), 0x6E => (Ror, Abs, 6, false), 0x7E => (Ror, Abx, 7, false),
        0x40 => (Rti, Imp, 6, false), 0x60 => (Rts, Imp, 6, false),
        0xE9 | 0xEB => (Sbc, Imm, 2, false), 0xE5 => (Sbc, Zp, 3, false), 0xF5 => (Sbc, Zpx, 4, false), 0xED => (Sbc, Abs, 4, false),
        0xFD => (Sbc, Abx, 4, true), 0xF9 => (Sbc, Aby, 4, true), 0xE1 => (Sbc, Izx, 6, false), 0xF1 => (Sbc, Izy, 5, true),
        0x38 => (Sec, Imp, 2, false), 0xF8 => (Sed, Imp, 2, false), 0x78 => (Sei, Imp, 2, false),
        0x85 => (Sta, Zp, 3, false), 0x95 => (Sta, Zpx, 4, false), 0x8D => (Sta, Abs, 4, false), 0x9D => (Sta, Abx, 5, false),
        0x99 => (Sta, Aby, 5, false), 0x81 => (Sta, Izx, 6, false), 0x91 => (Sta, Izy, 6, false),
        0x86 => (Stx, Zp, 3, false), 0x96 => (Stx, Zpy, 4, false), 0x8E => (Stx, Abs, 4, false),
        0x84 => (Sty, Zp, 3, false), 0x94 => (Sty, Zpx, 4, false), 0x8C => (Sty, Abs, 4, false),
        0xAA => (Tax, Imp, 2, false), 0xA8 => (Tay, Imp, 2, false), 0xBA => (Tsx, Imp, 2, false),
        0x8A => (Txa, Imp, 2, false), 0x9A => (Txs, Imp, 2, false), 0x98 => (Tya, Imp, 2, false),
        // Unofficial NOPs of every size.
        0x1A | 0x3A | 0x5A | 0x7A | 0xDA | 0xFA => (Nop, Imp, 2, false),
        0x80 | 0x82 | 0x89 | 0xC2 | 0xE2 => (Nop, Imm, 2, false),
        0x04 | 0x44 | 0x64 => (Nop, Zp, 3, false),
        0x14 | 0x34 | 0x54 | 0x74 | 0xD4 | 0xF4 => (Nop, Zpx, 4, false),
        0x0C => (Nop, Abs, 4, false),
        0x1C | 0x3C | 0x5C | 0x7C | 0xDC | 0xFC => (Nop, Abx, 4, true),
        0xA7 => (Lax, Zp, 3, false), 0xB7 => (Lax, Zpy, 4, false), 0xAF => (Lax, Abs, 4, false), 0xBF => (Lax, Aby, 4, true),
        0xA3 => (Lax, Izx, 6, false), 0xB3 => (Lax, Izy, 5, true), 0xAB => (Lax, Imm, 2, false),
        0x87 => (Sax, Zp, 3, false), 0x97 => (Sax, Zpy, 4, false), 0x8F => (Sax, Abs, 4, false), 0x83 => (Sax, Izx, 6, false),
        _ => {
            // The read-modify-write combinations, in the same eight columns.
            let op_kind = match op & 0xE0 {
                0x00 => Slo,
                0x20 => Rla,
                0x40 => Sre,
                0x60 => Rra,
                0xC0 => Dcp,
                0xE0 => Isb,
                _ => return (Kil, Imp, 2, false),
            };
            match op & 0x1F {
                0x07 => (op_kind, Zp, 5, false),
                0x17 => (op_kind, Zpx, 6, false),
                0x0F => (op_kind, Abs, 6, false),
                0x1F => (op_kind, Abx, 7, false),
                0x1B => (op_kind, Aby, 7, false),
                0x03 => (op_kind, Izx, 8, false),
                0x13 => (op_kind, Izy, 8, false),
                _ => (Kil, Imp, 2, false),
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Cpu {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub s: u8,
    pub p: u8,
    pub pc: u16,
    pub cycles: u64,
    /// Cycles to stand idle (OAM DMA).
    pub stall: u32,
    pub nmi: bool,
    /// Halted by a jammed (KIL) opcode.
    pub jammed: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Cpu { a: 0, x: 0, y: 0, s: 0xFD, p: I | U, pc: 0, cycles: 0, stall: 0, nmi: false, jammed: false }
    }
}

fn read16<B: Bus>(bus: &mut B, addr: u16) -> u16 {
    bus.read(addr) as u16 | (bus.read(addr.wrapping_add(1)) as u16) << 8
}

/// A 16-bit read whose high byte wraps within the page (JMP ($xxFF), and
/// zero-page pointers).
fn read16_wrap<B: Bus>(bus: &mut B, addr: u16) -> u16 {
    let hi = (addr & 0xFF00) | (addr.wrapping_add(1) & 0x00FF);
    bus.read(addr) as u16 | (bus.read(hi) as u16) << 8
}

impl Cpu {
    /// Power on and jump through the reset vector.
    pub fn reset<B: Bus>(&mut self, bus: &mut B) {
        self.s = self.s.wrapping_sub(3);
        self.p |= I;
        self.pc = read16(bus, 0xFFFC);
        self.cycles = 7;
    }

    fn set_zn(&mut self, v: u8) {
        self.p = (self.p & !(Z | N)) | if v == 0 { Z } else { 0 } | (v & N);
    }

    fn flag(&mut self, f: u8, on: bool) {
        if on {
            self.p |= f
        } else {
            self.p &= !f
        }
    }

    fn push<B: Bus>(&mut self, bus: &mut B, v: u8) {
        bus.write(0x0100 | self.s as u16, v);
        self.s = self.s.wrapping_sub(1);
    }

    fn pull<B: Bus>(&mut self, bus: &mut B) -> u8 {
        self.s = self.s.wrapping_add(1);
        bus.read(0x0100 | self.s as u16)
    }

    fn push16<B: Bus>(&mut self, bus: &mut B, v: u16) {
        self.push(bus, (v >> 8) as u8);
        self.push(bus, v as u8);
    }

    fn pull16<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let lo = self.pull(bus) as u16;
        lo | (self.pull(bus) as u16) << 8
    }

    /// The operand's address for a mode, and whether indexing crossed a page.
    fn address<B: Bus>(&mut self, bus: &mut B, mode: Mode) -> (u16, bool) {
        let pc = self.pc;
        match mode {
            Mode::Imp | Mode::Acc => (0, false),
            Mode::Imm => {
                self.pc = pc.wrapping_add(1);
                (pc, false)
            }
            Mode::Zp => {
                self.pc = pc.wrapping_add(1);
                (bus.read(pc) as u16, false)
            }
            Mode::Zpx => {
                self.pc = pc.wrapping_add(1);
                (bus.read(pc).wrapping_add(self.x) as u16, false)
            }
            Mode::Zpy => {
                self.pc = pc.wrapping_add(1);
                (bus.read(pc).wrapping_add(self.y) as u16, false)
            }
            Mode::Abs => {
                self.pc = pc.wrapping_add(2);
                (read16(bus, pc), false)
            }
            Mode::Abx => {
                self.pc = pc.wrapping_add(2);
                let base = read16(bus, pc);
                let a = base.wrapping_add(self.x as u16);
                (a, base & 0xFF00 != a & 0xFF00)
            }
            Mode::Aby => {
                self.pc = pc.wrapping_add(2);
                let base = read16(bus, pc);
                let a = base.wrapping_add(self.y as u16);
                (a, base & 0xFF00 != a & 0xFF00)
            }
            Mode::Ind => {
                self.pc = pc.wrapping_add(2);
                let ptr = read16(bus, pc);
                (read16_wrap(bus, ptr), false)
            }
            Mode::Izx => {
                self.pc = pc.wrapping_add(1);
                let zp = bus.read(pc).wrapping_add(self.x);
                (read16_wrap(bus, zp as u16), false)
            }
            Mode::Izy => {
                self.pc = pc.wrapping_add(1);
                let zp = bus.read(pc);
                let base = read16_wrap(bus, zp as u16);
                let a = base.wrapping_add(self.y as u16);
                (a, base & 0xFF00 != a & 0xFF00)
            }
            Mode::Rel => {
                self.pc = pc.wrapping_add(1);
                let off = bus.read(pc) as i8;
                (self.pc.wrapping_add(off as u16), false)
            }
        }
    }

    fn adc(&mut self, m: u8) {
        let sum = self.a as u16 + m as u16 + (self.p & C) as u16;
        let r = sum as u8;
        self.flag(C, sum > 0xFF);
        self.flag(V, (self.a ^ r) & (m ^ r) & 0x80 != 0);
        self.a = r;
        self.set_zn(r);
    }

    fn compare(&mut self, reg: u8, m: u8) {
        self.flag(C, reg >= m);
        self.set_zn(reg.wrapping_sub(m));
    }

    fn branch(&mut self, taken: bool, target: u16) -> u8 {
        if !taken {
            return 0;
        }
        let extra = if target & 0xFF00 != self.pc & 0xFF00 { 2 } else { 1 };
        self.pc = target;
        extra
    }

    /// Take a pending NMI: push PC and P, jump through $FFFA.
    fn interrupt<B: Bus>(&mut self, bus: &mut B, vector: u16, brk: bool) {
        self.push16(bus, self.pc);
        self.push(bus, self.p | U | if brk { B } else { 0 });
        self.p |= I;
        self.pc = read16(bus, vector);
    }

    /// Run one instruction (or a pending interrupt); returns the cycles it took.
    pub fn step<B: Bus>(&mut self, bus: &mut B) -> u32 {
        if self.stall > 0 {
            self.stall -= 1;
            self.cycles += 1;
            return 1;
        }
        if self.jammed {
            self.cycles += 1;
            return 1;
        }
        if self.nmi {
            self.nmi = false;
            self.interrupt(bus, 0xFFFA, false);
            self.cycles += 7;
            return 7;
        }
        let opcode = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        let (op, mode, base, penalty) = decode(opcode);
        let (addr, crossed) = self.address(bus, mode);
        let mut cycles = base as u32 + if penalty && crossed { 1 } else { 0 };
        use Op::*;
        match op {
            Adc => {
                let m = bus.read(addr);
                self.adc(m)
            }
            Sbc => {
                let m = bus.read(addr);
                self.adc(!m)
            }
            And => {
                self.a &= bus.read(addr);
                self.set_zn(self.a)
            }
            Ora => {
                self.a |= bus.read(addr);
                self.set_zn(self.a)
            }
            Eor => {
                self.a ^= bus.read(addr);
                self.set_zn(self.a)
            }
            Asl | Lsr | Rol | Ror => {
                let m = if mode == Mode::Acc { self.a } else { bus.read(addr) };
                let carry_in = self.p & C;
                let (r, c) = match op {
                    Asl => (m << 1, m & 0x80 != 0),
                    Lsr => (m >> 1, m & 1 != 0),
                    Rol => ((m << 1) | carry_in, m & 0x80 != 0),
                    _ => ((m >> 1) | (carry_in << 7), m & 1 != 0),
                };
                self.flag(C, c);
                self.set_zn(r);
                if mode == Mode::Acc {
                    self.a = r
                } else {
                    bus.write(addr, r)
                }
            }
            Bcc => cycles += self.branch(self.p & C == 0, addr) as u32,
            Bcs => cycles += self.branch(self.p & C != 0, addr) as u32,
            Beq => cycles += self.branch(self.p & Z != 0, addr) as u32,
            Bne => cycles += self.branch(self.p & Z == 0, addr) as u32,
            Bmi => cycles += self.branch(self.p & N != 0, addr) as u32,
            Bpl => cycles += self.branch(self.p & N == 0, addr) as u32,
            Bvc => cycles += self.branch(self.p & V == 0, addr) as u32,
            Bvs => cycles += self.branch(self.p & V != 0, addr) as u32,
            Bit => {
                let m = bus.read(addr);
                self.flag(Z, self.a & m == 0);
                self.p = (self.p & !(N | V)) | (m & (N | V));
            }
            Brk => {
                self.pc = self.pc.wrapping_add(1);
                self.interrupt(bus, 0xFFFE, true);
            }
            Clc => self.p &= !C,
            Cld => self.p &= !D,
            Cli => self.p &= !I,
            Clv => self.p &= !V,
            Sec => self.p |= C,
            Sed => self.p |= D,
            Sei => self.p |= I,
            Cmp => {
                let m = bus.read(addr);
                self.compare(self.a, m)
            }
            Cpx => {
                let m = bus.read(addr);
                self.compare(self.x, m)
            }
            Cpy => {
                let m = bus.read(addr);
                self.compare(self.y, m)
            }
            Dec => {
                let r = bus.read(addr).wrapping_sub(1);
                bus.write(addr, r);
                self.set_zn(r)
            }
            Inc => {
                let r = bus.read(addr).wrapping_add(1);
                bus.write(addr, r);
                self.set_zn(r)
            }
            Dex => {
                self.x = self.x.wrapping_sub(1);
                self.set_zn(self.x)
            }
            Dey => {
                self.y = self.y.wrapping_sub(1);
                self.set_zn(self.y)
            }
            Inx => {
                self.x = self.x.wrapping_add(1);
                self.set_zn(self.x)
            }
            Iny => {
                self.y = self.y.wrapping_add(1);
                self.set_zn(self.y)
            }
            Jmp => self.pc = addr,
            Jsr => {
                self.push16(bus, self.pc.wrapping_sub(1));
                self.pc = addr;
            }
            Rts => self.pc = self.pull16(bus).wrapping_add(1),
            Rti => {
                let p = self.pull(bus);
                self.p = (p & !B) | U;
                self.pc = self.pull16(bus);
            }
            Lda => {
                self.a = bus.read(addr);
                self.set_zn(self.a)
            }
            Ldx => {
                self.x = bus.read(addr);
                self.set_zn(self.x)
            }
            Ldy => {
                self.y = bus.read(addr);
                self.set_zn(self.y)
            }
            Nop => {
                if mode != Mode::Imp {
                    bus.read(addr);
                }
            }
            Pha => self.push(bus, self.a),
            Php => self.push(bus, self.p | B | U),
            Pla => {
                self.a = self.pull(bus);
                self.set_zn(self.a)
            }
            Plp => {
                let p = self.pull(bus);
                self.p = (p & !B) | U;
            }
            Sta => bus.write(addr, self.a),
            Stx => bus.write(addr, self.x),
            Sty => bus.write(addr, self.y),
            Tax => {
                self.x = self.a;
                self.set_zn(self.x)
            }
            Tay => {
                self.y = self.a;
                self.set_zn(self.y)
            }
            Tsx => {
                self.x = self.s;
                self.set_zn(self.x)
            }
            Txa => {
                self.a = self.x;
                self.set_zn(self.a)
            }
            Txs => self.s = self.x,
            Tya => {
                self.a = self.y;
                self.set_zn(self.a)
            }
            Lax => {
                let m = bus.read(addr);
                self.a = m;
                self.x = m;
                self.set_zn(m)
            }
            Sax => bus.write(addr, self.a & self.x),
            Dcp => {
                let r = bus.read(addr).wrapping_sub(1);
                bus.write(addr, r);
                self.compare(self.a, r)
            }
            Isb => {
                let r = bus.read(addr).wrapping_add(1);
                bus.write(addr, r);
                self.adc(!r)
            }
            Slo => {
                let m = bus.read(addr);
                self.flag(C, m & 0x80 != 0);
                let r = m << 1;
                bus.write(addr, r);
                self.a |= r;
                self.set_zn(self.a)
            }
            Rla => {
                let m = bus.read(addr);
                let r = (m << 1) | (self.p & C);
                self.flag(C, m & 0x80 != 0);
                bus.write(addr, r);
                self.a &= r;
                self.set_zn(self.a)
            }
            Sre => {
                let m = bus.read(addr);
                self.flag(C, m & 1 != 0);
                let r = m >> 1;
                bus.write(addr, r);
                self.a ^= r;
                self.set_zn(self.a)
            }
            Rra => {
                let m = bus.read(addr);
                let r = (m >> 1) | ((self.p & C) << 7);
                self.flag(C, m & 1 != 0);
                bus.write(addr, r);
                self.adc(r)
            }
            Kil => {
                self.jammed = true;
            }
        }
        self.cycles += cycles as u64;
        cycles
    }
}
