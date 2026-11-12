//! The 2C02's front end: its registers ($2000–$2007), video memory, OAM and
//! timing (341 dots a line, 262 lines a frame), drawing each visible line as
//! the hardware would, from the name, attribute and pattern tables, into a
//! nes-ppu `Frame` of 2C02 colour indices. Scroll changes between lines
//! (split screens) and sprite-0 hits work; changes within a line do not.

use crate::cart::{Cart, Mirroring};
use nes_ppu::Frame;

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 240;

pub struct Ppu {
    pub ctrl: u8,
    pub mask: u8,
    pub status: u8,
    pub oam_addr: u8,
    pub oam: [u8; 256],
    vram: [u8; 2048],
    palette: [u8; 32],
    /// Loopy's registers: the current and temporary VRAM address, fine X,
    /// and the write toggle.
    v: u16,
    t: u16,
    x: u8,
    w: bool,
    buffer: u8,
    open_bus: u8,
    pub dot: u16,
    pub line: u16,
    pub frame: Frame,
    /// Set at the start of vertical blank: a frame is finished.
    pub frame_done: bool,
    /// An NMI to deliver to the CPU.
    pub nmi: bool,
}

impl Default for Ppu {
    fn default() -> Self {
        Ppu {
            ctrl: 0,
            mask: 0,
            status: 0,
            oam_addr: 0,
            oam: [0; 256],
            vram: [0; 2048],
            palette: [0x0F; 32],
            v: 0,
            t: 0,
            x: 0,
            w: false,
            buffer: 0,
            open_bus: 0,
            dot: 0,
            line: 0,
            frame: Frame { pixels: vec![0x0F; WIDTH * HEIGHT] },
            frame_done: false,
            nmi: false,
        }
    }
}

impl Ppu {
    fn rendering(&self) -> bool {
        self.mask & 0x18 != 0
    }

    fn nametable_index(&self, cart: &Cart, addr: u16) -> usize {
        let a = (addr - 0x2000) & 0x0FFF;
        let table = a / 0x400;
        let page = match cart.mirroring {
            Mirroring::Horizontal => table / 2,
            Mirroring::Vertical => table % 2,
        };
        (page * 0x400 + (a & 0x3FF)) as usize
    }

    fn palette_index(addr: u16) -> usize {
        let i = (addr & 0x1F) as usize;
        // $3F10/$14/$18/$1C are $3F00/$04/$08/$0C.
        if i >= 16 && i % 4 == 0 { i - 16 } else { i }
    }

    pub fn mem_read(&self, cart: &Cart, addr: u16) -> u8 {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => cart.chr_read(addr),
            0x2000..=0x3EFF => self.vram[self.nametable_index(cart, 0x2000 | (addr & 0x0FFF))],
            _ => self.palette[Self::palette_index(addr)],
        }
    }

    fn mem_write(&mut self, cart: &mut Cart, addr: u16, v: u8) {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => cart.chr_write(addr, v),
            0x2000..=0x3EFF => {
                let i = self.nametable_index(cart, 0x2000 | (addr & 0x0FFF));
                self.vram[i] = v;
            }
            _ => self.palette[Self::palette_index(addr)] = v & 0x3F,
        }
    }

    /// A CPU read of $2000–$3FFF.
    pub fn read_register(&mut self, cart: &Cart, reg: u16) -> u8 {
        let v = match reg & 7 {
            2 => {
                let s = (self.status & 0xE0) | (self.open_bus & 0x1F);
                self.status &= !0x80;
                self.w = false;
                s
            }
            4 => self.oam[self.oam_addr as usize],
            7 => {
                let addr = self.v & 0x3FFF;
                let value = if addr >= 0x3F00 {
                    // Palette reads are immediate; the buffer takes the name
                    // table byte underneath.
                    self.buffer = self.mem_read(cart, addr - 0x1000);
                    self.mem_read(cart, addr)
                } else {
                    let b = self.buffer;
                    self.buffer = self.mem_read(cart, addr);
                    b
                };
                self.v = self.v.wrapping_add(if self.ctrl & 0x04 != 0 { 32 } else { 1 });
                value
            }
            _ => self.open_bus,
        };
        self.open_bus = v;
        v
    }

    /// A CPU write of $2000–$3FFF.
    pub fn write_register(&mut self, cart: &mut Cart, reg: u16, v: u8) {
        self.open_bus = v;
        match reg & 7 {
            0 => {
                let was = self.ctrl & 0x80 != 0;
                self.ctrl = v;
                self.t = (self.t & 0xF3FF) | ((v as u16 & 3) << 10);
                // Turning NMI on during vertical blank raises one at once.
                if !was && v & 0x80 != 0 && self.status & 0x80 != 0 {
                    self.nmi = true;
                }
            }
            1 => self.mask = v,
            3 => self.oam_addr = v,
            4 => {
                self.oam[self.oam_addr as usize] = v;
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }
            5 => {
                if !self.w {
                    self.t = (self.t & 0xFFE0) | (v as u16 >> 3);
                    self.x = v & 7;
                } else {
                    self.t = (self.t & 0x8C1F) | ((v as u16 & 7) << 12) | ((v as u16 & 0xF8) << 2);
                }
                self.w = !self.w;
            }
            6 => {
                if !self.w {
                    self.t = (self.t & 0x00FF) | ((v as u16 & 0x3F) << 8);
                } else {
                    self.t = (self.t & 0xFF00) | v as u16;
                    self.v = self.t;
                }
                self.w = !self.w;
            }
            7 => {
                self.mem_write(cart, self.v, v);
                self.v = self.v.wrapping_add(if self.ctrl & 0x04 != 0 { 32 } else { 1 });
            }
            _ => {}
        }
    }

    fn increment_y(&mut self) {
        if self.v & 0x7000 != 0x7000 {
            self.v += 0x1000;
        } else {
            self.v &= !0x7000;
            let mut y = (self.v & 0x03E0) >> 5;
            if y == 29 {
                y = 0;
                self.v ^= 0x0800;
            } else if y == 31 {
                y = 0;
            } else {
                y += 1;
            }
            self.v = (self.v & !0x03E0) | (y << 5);
        }
    }

    /// Advance one dot (a third of a CPU cycle).
    pub fn tick(&mut self, cart: &Cart) {
        let visible = self.line < 240;
        let pre = self.line == 261;
        if self.rendering() && (visible || pre) {
            if self.dot == 256 {
                if visible {
                    self.draw_line(cart);
                }
                self.increment_y();
            } else if self.dot == 257 {
                self.v = (self.v & !0x041F) | (self.t & 0x041F);
            } else if pre && (280..=304).contains(&self.dot) {
                self.v = (self.v & !0x7BE0) | (self.t & 0x7BE0);
            }
        } else if visible && self.dot == 256 {
            // Rendering off: the backdrop colour.
            let c = self.palette[0];
            let row = self.line as usize * WIDTH;
            self.frame.pixels[row..row + WIDTH].fill(c);
        }
        if self.dot == 1 {
            if self.line == 241 {
                self.status |= 0x80;
                self.frame_done = true;
                if self.ctrl & 0x80 != 0 {
                    self.nmi = true;
                }
            } else if pre {
                self.status &= !0xE0;
            }
        }
        self.dot += 1;
        if self.dot > 340 {
            self.dot = 0;
            self.line = (self.line + 1) % 262;
        }
    }

    /// Draw the current line from v (and fine x), with its sprites.
    fn draw_line(&mut self, cart: &Cart) {
        let line = self.line as usize;
        let show_bg = self.mask & 0x08 != 0;
        let show_sp = self.mask & 0x10 != 0;
        let bg_left = self.mask & 0x02 != 0;
        let sp_left = self.mask & 0x04 != 0;
        let gray = if self.mask & 0x01 != 0 { 0x30 } else { 0x3F };
        // Background: 33 tiles' worth of pixels from the name table, starting
        // at v's coarse X and fine x.
        let mut bg = [0u8; WIDTH];
        let mut bg_pal = [0u8; WIDTH];
        if show_bg {
            let fine_y = (self.v >> 12) & 7;
            let coarse_y = (self.v >> 5) & 31;
            let nt_v = (self.v >> 11) & 1;
            let table = if self.ctrl & 0x10 != 0 { 0x1000 } else { 0 };
            for x in 0..WIDTH {
                let fx = x as u16 + self.x as u16;
                let cx_all = (self.v & 31) + (fx >> 3);
                let nt_h = ((self.v >> 10) & 1) ^ ((cx_all >> 5) & 1);
                let cx = cx_all & 31;
                let base = 0x2000 | (nt_v << 11) | (nt_h << 10);
                let tile = self.mem_read(cart, base | (coarse_y << 5) | cx) as u16;
                let attr = self.mem_read(cart, base | 0x3C0 | ((coarse_y >> 2) << 3) | (cx >> 2));
                let shift = ((coarse_y & 2) << 1) | (cx & 2);
                let addr = table | (tile << 4) | fine_y;
                let lo = cart.chr_read(addr);
                let hi = cart.chr_read(addr + 8);
                let bit = 7 - (fx & 7);
                bg[x] = ((hi >> bit) & 1) << 1 | ((lo >> bit) & 1);
                bg_pal[x] = (attr >> shift) & 3;
                if !bg_left && x < 8 {
                    bg[x] = 0;
                }
            }
        }
        // Sprites: the first eight on this line, in OAM order (as the
        // hardware evaluates them); the first opaque pixel wins.
        let tall = self.ctrl & 0x20 != 0;
        let height = if tall { 16 } else { 8 };
        let mut found = [0usize; 8];
        let mut count = 0;
        if show_sp {
            for i in 0..64 {
                let y = self.oam[i * 4] as usize;
                if line >= y + 1 && line < y + 1 + height {
                    if count == 8 {
                        self.status |= 0x20;
                        break;
                    }
                    found[count] = i;
                    count += 1;
                }
            }
        }
        let row = line * WIDTH;
        let universal = self.palette[0];
        for x in 0..WIDTH {
            let mut sp_px = 0u8;
            let mut sp_pal = 0u8;
            let mut sp_behind = false;
            for &i in &found[..count] {
                let sx = self.oam[i * 4 + 3] as usize;
                if x < sx || x >= sx + 8 || (!sp_left && x < 8) {
                    continue;
                }
                let attr = self.oam[i * 4 + 2];
                let tile = self.oam[i * 4 + 1] as u16;
                let mut r = (line - 1 - self.oam[i * 4] as usize) as u16;
                if attr & 0x80 != 0 {
                    r = height as u16 - 1 - r;
                }
                let addr = if tall {
                    ((tile & 1) << 12) | ((tile & 0xFE) << 4) + if r >= 8 { 16 } else { 0 } | (r & 7)
                } else {
                    (if self.ctrl & 0x08 != 0 { 0x1000 } else { 0 }) | (tile << 4) | r
                };
                let lo = cart.chr_read(addr);
                let hi = cart.chr_read(addr + 8);
                let mut col = (x - sx) as u8;
                if attr & 0x40 == 0 {
                    col = 7 - col;
                }
                let px = ((hi >> col) & 1) << 1 | ((lo >> col) & 1);
                if px == 0 {
                    continue;
                }
                if i == 0 && bg[x] != 0 && x != 255 && show_bg {
                    self.status |= 0x40;
                }
                sp_px = px;
                sp_pal = attr & 3;
                sp_behind = attr & 0x20 != 0;
                break;
            }
            let colour = if sp_px != 0 && (!sp_behind || bg[x] == 0) {
                self.palette[16 + sp_pal as usize * 4 + sp_px as usize]
            } else if bg[x] != 0 {
                self.palette[bg_pal[x] as usize * 4 + bg[x] as usize]
            } else {
                universal
            };
            self.frame.pixels[row + x] = colour & gray;
        }
    }
}
