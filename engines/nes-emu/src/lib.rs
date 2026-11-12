//! A basic NES emulator core: the 6502 ([`cpu`]), the PPU ([`ppu`]), the APU
//! (nes-apu, run in step with the CPU), iNES cartridges with mapper 0
//! ([`cart`]), RAM, OAM DMA and two controllers, run a frame at a time. It
//! draws into nes-ppu's `Frame` (2C02 colour indices), so
//! `nes_ppu::Frame::to_rgba` and nes-screen show it, and makes samples
//! ([`Nes::take_samples`]) for whatever plays them.

pub mod cart;
pub mod cpu;
pub mod ppu;

use cart::Cart;
use cpu::{Bus, Cpu};
use ppu::Ppu;

/// Controller buttons, as the shift register reports them (bit 0 first).
pub mod button {
    pub const A: u8 = 0x01;
    pub const B: u8 = 0x02;
    pub const SELECT: u8 = 0x04;
    pub const START: u8 = 0x08;
    pub const UP: u8 = 0x10;
    pub const DOWN: u8 = 0x20;
    pub const LEFT: u8 = 0x40;
    pub const RIGHT: u8 = 0x80;
}

#[derive(Clone, Copy, Debug, Default)]
struct Pad {
    buttons: u8,
    shift: u8,
    strobe: bool,
}

impl Pad {
    fn read(&mut self) -> u8 {
        if self.strobe {
            return self.buttons & 1;
        }
        let bit = self.shift & 1;
        self.shift = (self.shift >> 1) | 0x80;
        bit
    }
}

/// Everything on the CPU's bus.
pub struct Board {
    pub ram: [u8; 2048],
    pub ppu: Ppu,
    pub cart: Cart,
    pub apu: nes_apu::Apu,
    pads: [Pad; 2],
    dma_stall: u32,
    open_bus: u8,
    cpu_cycles: u64,
}

impl Bus for Board {
    fn read(&mut self, addr: u16) -> u8 {
        let v = match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize],
            0x2000..=0x3FFF => self.ppu.read_register(&self.cart, addr),
            0x4015 => self.apu.status(),
            0x4016 => (self.open_bus & 0xE0) | self.pads[0].read(),
            0x4017 => (self.open_bus & 0xE0) | self.pads[1].read(),
            0x4000..=0x401F => self.open_bus,
            _ => self.cart.read(addr),
        };
        self.open_bus = v;
        v
    }

    fn write(&mut self, addr: u16, v: u8) {
        self.open_bus = v;
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize] = v,
            0x2000..=0x3FFF => self.ppu.write_register(&mut self.cart, addr, v),
            0x4014 => {
                // OAM DMA: 256 bytes from page v, while the CPU stands idle.
                let page = (v as u16) << 8;
                for i in 0..256u16 {
                    let b = self.read(page | i);
                    let at = self.ppu.oam_addr.wrapping_add(i as u8);
                    self.ppu.oam[at as usize] = b;
                }
                self.dma_stall += 513 + (self.cpu_cycles & 1) as u32;
            }
            0x4016 => {
                let strobe = v & 1 != 0;
                for p in &mut self.pads {
                    p.strobe = strobe;
                    if strobe {
                        p.shift = p.buttons;
                    }
                }
            }
            0x4000..=0x4013 | 0x4015 | 0x4017 => self.apu.write(addr, v),
            0x4018..=0x401F => {}
            _ => self.cart.write(addr, v),
        }
    }
}

/// The sample rate the APU makes sound at by default ([`Nes::set_sample_rate`]).
pub const SAMPLE_RATE: u32 = 48_000;

/// A console with a cartridge in it.
pub struct Nes {
    pub cpu: Cpu,
    pub board: Board,
}

impl Nes {
    /// Insert a cartridge (an iNES file's bytes) and press reset.
    pub fn new(rom: &[u8]) -> Result<Nes, String> {
        let cart = Cart::load(rom)?;
        let mut board = Board { ram: [0; 2048], ppu: Ppu::default(), cart, apu: nes_apu::Apu::new(SAMPLE_RATE), pads: [Pad::default(); 2], dma_stall: 0, open_bus: 0, cpu_cycles: 0 };
        let mut cpu = Cpu::default();
        cpu.reset(&mut board);
        Ok(Nes { cpu, board })
    }

    /// Press reset (the cartridge and RAM stay).
    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.board);
    }

    /// Player `n`'s held buttons (see [`button`]).
    pub fn set_buttons(&mut self, n: usize, buttons: u8) {
        self.board.pads[n & 1].buttons = buttons;
        if self.board.pads[n & 1].strobe {
            self.board.pads[n & 1].shift = buttons;
        }
    }

    /// One instruction, with the PPU kept in step (three dots a cycle).
    pub fn step(&mut self) -> u32 {
        self.board.cpu_cycles = self.cpu.cycles;
        let cycles = self.cpu.step(&mut self.board);
        self.board.apu.run(cycles);
        for _ in 0..cycles * 3 {
            self.board.ppu.tick(&self.board.cart);
        }
        if self.board.ppu.nmi {
            self.board.ppu.nmi = false;
            self.cpu.nmi = true;
        }
        if self.board.dma_stall > 0 {
            self.cpu.stall += self.board.dma_stall;
            self.board.dma_stall = 0;
        }
        cycles
    }

    /// Run until the PPU finishes a frame (the start of vertical blank).
    pub fn run_frame(&mut self) {
        self.board.ppu.frame_done = false;
        // A guard against a program that turned the PPU off and spins.
        let mut budget = 40_000u32;
        while !self.board.ppu.frame_done && budget > 0 {
            budget = budget.saturating_sub(self.step());
        }
    }

    /// Make sound at `rate` samples a second from now on.
    pub fn set_sample_rate(&mut self, rate: u32) {
        if self.board.apu.sample_rate() != rate {
            self.board.apu = nes_apu::Apu::new(rate);
        }
    }

    /// The samples the APU has made since last asked (about −1 to 1).
    pub fn take_samples(&mut self) -> Vec<f32> {
        self.board.apu.take_samples()
    }

    /// The last frame, as 2C02 colour indices.
    pub fn frame(&self) -> &nes_ppu::Frame {
        &self.board.ppu.frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// nestest.nes, from `NESTEST_ROM` or ~/Downloads (it is not kept in
    /// this repo).
    fn nestest() -> Option<Vec<u8>> {
        let path = std::env::var("NESTEST_ROM").ok().unwrap_or_else(|| format!("{}/Downloads/nestest.nes", std::env::var("HOME").unwrap_or_default()));
        std::fs::read(path).ok()
    }

    /// nestest's automated mode: start at $C000 and run every CPU test; it
    /// leaves the first failure's code in $02 (official) and $03 (unofficial).
    #[test]
    fn nestest_cpu() {
        let Some(rom) = nestest() else {
            eprintln!("nestest.nes not found: set NESTEST_ROM to run this test");
            return;
        };
        let mut nes = Nes::new(&rom).unwrap();
        nes.cpu.pc = 0xC000;
        nes.cpu.p = 0x24;
        nes.cpu.s = 0xFD;
        nes.cpu.cycles = 7;
        // nestest.log's last line: the 8991st instruction, at $C66E, with
        // 26554 cycles behind it.
        for _ in 0..8990 {
            nes.cpu.step(&mut nes.board);
        }
        assert_eq!(nes.board.ram[2], 0, "official opcodes: failure {:#04x}", nes.board.ram[2]);
        assert_eq!(nes.board.ram[3], 0, "unofficial opcodes: failure {:#04x}", nes.board.ram[3]);
        assert_eq!(nes.cpu.pc, 0xC66E);
        assert_eq!(nes.cpu.cycles, 26554, "cycle count");
    }

    /// nestest's menu: with the PPU running, it draws its title screen.
    #[test]
    fn nestest_draws_its_menu() {
        let Some(rom) = nestest() else { return };
        let mut nes = Nes::new(&rom).unwrap();
        for _ in 0..30 {
            nes.run_frame();
        }
        let distinct: std::collections::BTreeSet<u8> = nes.frame().pixels.iter().copied().collect();
        assert!(distinct.len() >= 2, "a blank screen: {distinct:?}");
    }

    /// Writes a ROM's screen after a second to `NES_PNG`: the ROM is
    /// `NES_ROM` (or nestest), `NES_BUTTONS` the buttons held, if any.
    #[test]
    #[ignore]
    fn screenshot() {
        let rom = std::env::var("NES_ROM").ok().and_then(|p| std::fs::read(p).ok()).or_else(nestest);
        let (Some(rom), Ok(out)) = (rom, std::env::var("NES_PNG")) else { return };
        let mut nes = Nes::new(&rom).unwrap();
        let buttons = std::env::var("NES_BUTTONS").ok().and_then(|b| b.parse().ok()).unwrap_or(0);
        for f in 0..60 {
            nes.set_buttons(0, if f > 10 { buttons } else { 0 });
            nes.run_frame();
        }
        let mut rgba = vec![0u8; 256 * 240 * 4];
        nes.frame().to_rgba(&mut rgba);
        std::fs::write(out, nes_ppu::png::encode(256, 240, &rgba)).unwrap();
    }

    #[test]
    fn rejects_other_mappers() {
        let mut rom = vec![0u8; 16 + 0x4000 + 0x2000];
        rom[..4].copy_from_slice(b"NES\x1A");
        rom[4] = 1;
        rom[5] = 1;
        rom[6] = 0x10;
        assert!(Nes::new(&rom).err().unwrap().contains("mapper 1"));
    }
}
