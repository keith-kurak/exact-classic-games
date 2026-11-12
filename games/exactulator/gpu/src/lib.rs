//! Exactulator's GPU module: the `ppu` surface runs an NES (engines/nes-emu)
//! and shows its frames through nes-screen, at the console's own rate of
//! 60.0988 frames a second whatever the display's.
//!
//! Inputs: 1. the ROM, an iNES file in base64 ("" for none); 2. the held
//! buttons (nes_emu::button bits); 3. a reset count: each rise presses reset.

use exact_gpu::json::{number, text};
use exact_gpu::{Registry, SurfaceError, Value};
use nes_emu::Nes;
use nes_ppu::font::FONT;
use nes_ppu::Frame;
use nes_screen::NesGame;

pub const INPUTS: usize = 3;
const FRAME_MS: f64 = 1000.0 / 60.0988;

/// Standard base64, padding optional, whitespace ignored.
pub fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for &c in s.as_bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        acc = (acc << 6) | val(c).ok_or("the ROM data is not base64")?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

/// Lines of text on a black NES screen, centred, in the shared font:
/// the screen before a ROM, or why one would not load.
fn message(lines: &[(&str, u8)]) -> Frame {
    let mut f = Frame { pixels: vec![0x0F; 256 * 240] };
    let top = 120 - (lines.len() as i32 * 12) / 2;
    for (i, (line, colour)) in lines.iter().enumerate() {
        let w = line.chars().count() as i32 * 8;
        let x0 = 128 - w / 2;
        let y0 = top + i as i32 * 12;
        for (k, ch) in line.chars().enumerate() {
            let Some((_, rows)) = FONT.iter().find(|(c, _)| *c == ch.to_ascii_uppercase()).or_else(|| FONT.iter().find(|(c, _)| *c == ch)) else { continue };
            for (dy, row) in rows.iter().enumerate() {
                for (dx, px) in row.chars().enumerate() {
                    let (x, y) = (x0 + k as i32 * 8 + dx as i32, y0 + dy as i32);
                    if px == '#' && (0..256).contains(&x) && (0..240).contains(&y) {
                        f.pixels[(y * 256 + x) as usize] = *colour;
                    }
                }
            }
        }
    }
    f
}

/// Words wrapped to `width` characters.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in text.split_whitespace() {
        let last = lines.last_mut().unwrap();
        if !last.is_empty() && last.len() + 1 + word.len() > width {
            lines.push(word.to_string());
        } else {
            if !last.is_empty() {
                last.push(' ');
            }
            last.push_str(word);
        }
    }
    lines
}

#[derive(Default)]
pub struct Emulator {
    rom: String,
    nes: Option<Nes>,
    error: Option<String>,
    buttons: u8,
    resets: f64,
    last_ms: Option<f64>,
    owed: f64,
}

impl Emulator {
    fn load(&mut self, rom: String) {
        self.rom = rom;
        self.nes = None;
        self.error = None;
        self.last_ms = None;
        self.owed = 0.0;
        if self.rom.is_empty() {
            return;
        }
        match base64_decode(&self.rom).and_then(|bytes| Nes::new(&bytes)) {
            Ok(nes) => self.nes = Some(nes),
            Err(e) => self.error = Some(e),
        }
    }
}

impl NesGame for Emulator {
    fn bind(&mut self, inputs: &[Value]) -> Result<(), SurfaceError> {
        let [rom, buttons, resets] = inputs else {
            return Err(SurfaceError(format!("ppu: expected {INPUTS} inputs, got {}", inputs.len())));
        };
        let rom = text(rom, "rom")?;
        if rom != self.rom {
            self.load(rom);
        }
        self.buttons = number(buttons, "buttons")?.clamp(0.0, 255.0) as u8;
        let resets = number(resets, "resets")?;
        if resets > self.resets {
            if let Some(nes) = self.nes.as_mut() {
                nes.reset();
            }
        }
        self.resets = resets;
        Ok(())
    }

    fn draw(&mut self, frame: u64, rgba: &mut [u8]) {
        self.draw_at(frame, frame as f64 * FRAME_MS, true, rgba);
    }

    fn draw_at(&mut self, _frame: u64, now_ms: f64, seekable: bool, rgba: &mut [u8]) {
        let Some(nes) = self.nes.as_mut() else {
            let frame = match &self.error {
                Some(e) => {
                    let mut lines: Vec<(String, u8)> = vec![("THIS ROM CAN NOT RUN".into(), 0x27), (String::new(), 0x0F)];
                    lines.extend(wrap(e, 28).into_iter().map(|l| (l, 0x30)));
                    let refs: Vec<(&str, u8)> = lines.iter().map(|(l, c)| (l.as_str(), *c)).collect();
                    message(&refs)
                }
                None => message(&[("EXACTULATOR", 0x38), ("", 0x0F), ("OPEN A .NES FILE", 0x30), ("MAPPER 0 CARTRIDGES", 0x10)]),
            };
            frame.to_rgba(rgba);
            return;
        };
        nes.set_buttons(0, self.buttons);
        // Run as many NES frames as the clock owes: the console's 60.0988 a
        // second on any display. Under the agent every millisecond counts;
        // otherwise a stall (the app hidden) is not caught up.
        let elapsed = self.last_ms.map_or(FRAME_MS, |last| (now_ms - last).max(0.0));
        self.last_ms = Some(now_ms);
        self.owed += elapsed / FRAME_MS;
        let cap = if seekable { 600.0 } else { 3.0 };
        let n = self.owed.floor().min(cap);
        self.owed = if seekable { self.owed - n } else { (self.owed - n).min(1.0) };
        for _ in 0..n as u32 {
            nes.run_frame();
        }
        nes.frame().to_rgba(rgba);
    }
}

/// Each surface: its name, its number of inputs, its factory.
pub static REGISTRY: Registry = Registry {
    surfaces: &[("ppu", INPUTS, nes_screen::surface::<Emulator>)],
    shaders: nes_screen::shaders::SHADERS,
};

exact_gpu::module!(REGISTRY);

#[cfg(test)]
mod tests {
    use super::*;

    const DEMO: &[u8] = include_bytes!("../../assets/demo.nes");

    fn base64(bytes: &[u8]) -> String {
        const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        bytes.chunks(3).map(|c| {
            let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
            (0..4).map(|i| if i <= c.len() { A[(n >> (18 - 6 * i)) as usize & 63] as char } else { '=' }).collect::<String>()
        }).collect()
    }

    #[test]
    fn base64_round_trips() {
        assert_eq!(base64_decode(&base64(DEMO)).unwrap(), DEMO);
        assert_eq!(base64_decode("TkVTGg==").unwrap(), b"NES\x1A");
    }

    #[test]
    fn the_demo_cart_moves_its_star() {
        let mut nes = Nes::new(DEMO).unwrap();
        for _ in 0..10 {
            nes.run_frame();
        }
        let x0 = nes.board.ram[0x0203];
        nes.set_buttons(0, nes_emu::button::RIGHT);
        for _ in 0..20 {
            nes.run_frame();
        }
        assert_eq!(nes.board.ram[0x0203], x0.wrapping_add(20));
        nes.set_buttons(0, nes_emu::button::A);
        nes.run_frame();
        assert_eq!(nes.board.ram[0x0202], 1, "A turns the star green");
    }

    #[test]
    fn a_bad_rom_says_why() {
        let mut e = Emulator::default();
        e.load(base64(b"not a rom at all"));
        assert!(e.nes.is_none());
        assert!(e.error.as_deref().unwrap().contains("iNES"));
    }
}
