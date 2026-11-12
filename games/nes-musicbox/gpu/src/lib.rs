//! NES Musicbox's GPU module: its screen, an NES picture from the shared
//! engines (nes-ppu draws it, nes-screen shows it). The music itself is made
//! by nes-apu ahead of time: `cargo test --release -p nes-musicbox-gpu render
//! -- --ignored` renders the songs and sound effects (music.rs) to WAV files
//! and writes songs.contract, the rows the screen's meters follow.
//!
//! Inputs: 1. sprites, `name:x:y` joined by `;`; 2. windows, `x,y,w,h[,gap]|line|line`
//! joined by `^` (tiles; a line starting `%n` is in palette n; `{…}` is yellow
//! in palette 0; `~0`–`~8` a meter tile filled that many pixels).

pub mod music;

use exact_gpu::json::text;
use exact_gpu::{Registry, SurfaceError, Value};
use nes_ppu::font::FONT;
use nes_ppu::{glyph_tile, render, Cell, Limits, Metasprite, Palettes, PatternTable, Scene, Sprite, Tile};
use nes_screen::NesGame;
use std::collections::HashMap;
use std::sync::OnceLock;

pub const INPUTS: usize = 2;
const COLS: usize = 32;
const ROWS: usize = 30;

/// Windows in NES blue; the meters' colours: yellow, red, green and sky for
/// pulse 1, pulse 2, triangle and noise.
const PALETTES: Palettes = Palettes {
    bg: [[0x0F, 0x02, 0x38, 0x30], [0x0F, 0x02, 0x16, 0x30], [0x0F, 0x02, 0x2A, 0x30], [0x0F, 0x02, 0x21, 0x30]],
    spr: [[0x0F, 0x00, 0x10, 0x30], [0x0F, 0x16, 0x27, 0x38], [0x0F, 0x02, 0x21, 0x30], [0x0F, 0x0F, 0x0F, 0x0F]],
};

const HAND: &str = "
................
.1111111........
133333331111....
1333333333331...
133333331111....
13333333.1......
.1111111........
................";
const NOTE: &str = "
....11..
....1.1.
....1..1
....1...
.2221...
2333....
2333....
.22.....";

struct Art {
    bg: PatternTable,
    spr: PatternTable,
    glyphs: HashMap<char, u16>,
    yellow: HashMap<char, u16>,
    meters: [u16; 9],
    border: [u16; 9],
    sprites: HashMap<&'static str, Metasprite>,
}

fn tile(rows: [&str; 8]) -> Tile {
    let mut t = Tile::default();
    for (y, r) in rows.iter().enumerate() {
        for (x, c) in r.chars().enumerate() {
            t.0[y][x] = c.to_digit(10).unwrap_or(0) as u8;
        }
    }
    t
}

fn build() -> Art {
    let mut bg = PatternTable::default();
    let mut spr = PatternTable::default();
    bg.add(Tile::default());
    let mut glyphs = HashMap::new();
    let mut yellow = HashMap::new();
    for (ch, rows) in FONT {
        glyphs.insert(*ch, bg.add(glyph_tile(rows, 3, 1)));
        yellow.insert(*ch, bg.add(glyph_tile(rows, 2, 1)));
    }
    let fill = bg.add(Tile([[1; 8]; 8]));
    glyphs.insert(' ', fill);
    yellow.insert(' ', fill);
    let mut meters = [0u16; 9];
    for n in 0..9 {
        let mut t = Tile([[1; 8]; 8]);
        for y in 1..7 {
            for x in 0..8 {
                t.0[y][x] = if x < n && x != 7 { 2 } else if y == 1 || y == 6 { 0 } else { 1 };
            }
        }
        meters[n] = bg.add(t);
    }
    // Window border: corners, edges, and the fill.
    let corner = tile(["..3333..", ".300003.", "30111103"[..8].try_into().unwrap_or("30111103"), "30111111", "30111111", "30111111", "30111111", "30111111"]);
    let _ = corner;
    let edge_top = tile(["33333333", "00000000", "11111111", "11111111", "11111111", "11111111", "11111111", "11111111"]);
    let edge_left = tile(["30111111", "30111111", "30111111", "30111111", "30111111", "30111111", "30111111", "30111111"]);
    let tl = tile(["..333333", ".3000000", "30111111", "30111111", "30111111", "30111111", "30111111", "30111111"]);
    let flip_h = |t: &Tile| Tile(t.0.map(|r| { let mut r = r; r.reverse(); r }));
    let flip_v = |t: &Tile| { let mut t = *t; t.0.reverse(); t };
    let border = [
        bg.add(tl),
        bg.add(edge_top),
        bg.add(flip_h(&tl)),
        bg.add(edge_left),
        fill,
        bg.add(flip_h(&edge_left)),
        bg.add(flip_v(&tl)),
        bg.add(flip_v(&edge_top)),
        bg.add(flip_h(&flip_v(&tl))),
    ];
    let mut sprites = HashMap::new();
    sprites.insert("hand", Metasprite::from_art(&mut spr, HAND, 0));
    sprites.insert("note", Metasprite::from_art(&mut spr, NOTE, 1));
    Art { bg, spr, glyphs, yellow, meters, border, sprites }
}

fn art() -> &'static Art {
    static ART: OnceLock<Art> = OnceLock::new();
    ART.get_or_init(build)
}

fn parse_windows(spec: &str) -> Vec<Option<Cell>> {
    let art = art();
    let mut cells = vec![None; COLS * ROWS];
    let mut put = |x: i32, y: i32, tile: u16, palette: u8| {
        if x >= 0 && y >= 0 && (x as usize) < COLS && (y as usize) < ROWS {
            cells[y as usize * COLS + x as usize] = Some(Cell { tile, palette, flip: 0 });
        }
    };
    for win in spec.split('^').filter(|s| !s.is_empty()) {
        let mut parts = win.split('|');
        let r: Vec<i32> = parts.next().unwrap_or("").split(',').filter_map(|v| v.trim().parse().ok()).collect();
        let (x, y, w, h, gap) = match r[..] {
            [x, y, w, h] => (x, y, w, h, 1),
            [x, y, w, h, g] => (x, y, w, h, g.max(1)),
            _ => continue,
        };
        let b = &art.border;
        for j in 0..h {
            for i in 0..w {
                let (bx, by) = (if i == 0 { 0 } else if i == w - 1 { 2 } else { 1 }, if j == 0 { 0 } else if j == h - 1 { 2 } else { 1 });
                put(x + i, y + j, b[by * 3 + bx], 0);
            }
        }
        for (row, line) in parts.enumerate() {
            let ty = y + 1 + row as i32 * gap;
            if ty >= y + h - 1 {
                break;
            }
            let (pal, line) = match line.strip_prefix('%') {
                Some(rest) => (rest[..1].parse::<u8>().unwrap_or(0), &rest[1..]),
                None => (0, line),
            };
            let mut chars = line.chars();
            let mut col = 0;
            let mut gold = false;
            while let Some(ch) = chars.next() {
                if ch == '{' || ch == '}' {
                    gold = ch == '{';
                    continue;
                }
                let tx = x + 1 + col;
                if tx >= x + w - 1 {
                    break;
                }
                let tile = if ch == '~' {
                    art.meters[chars.next().and_then(|d| d.to_digit(10)).unwrap_or(0).min(8) as usize]
                } else {
                    let set = if gold { &art.yellow } else { &art.glyphs };
                    *set.get(&ch).or_else(|| set.get(&ch.to_ascii_uppercase())).unwrap_or(&art.border[4])
                };
                put(tx, ty, tile, pal);
                col += 1;
            }
        }
    }
    cells
}

#[derive(Default)]
pub struct Musicbox {
    sprites: String,
    windows: Vec<Option<Cell>>,
}

impl Musicbox {
    fn draw_frame(&self, frame: u64, rgba: &mut [u8]) {
        let art = art();
        let mut sprites: Vec<Sprite> = Vec::new();
        for item in self.sprites.split(';').filter(|s| !s.is_empty()) {
            let f: Vec<&str> = item.split(':').collect();
            if let [name, x, y] = f[..] {
                if let (Some(m), Ok(x), Ok(y)) = (art.sprites.get(name), x.parse::<f64>(), y.parse::<f64>()) {
                    m.place(x.round() as i32, y.round() as i32, false, None, &mut sprites);
                }
            }
        }
        let windows = &self.windows;
        let out = render(&Scene {
            bg_table: &art.bg,
            spr_table: &art.spr,
            palettes: PALETTES,
            background: &|_, _| Cell::default(),
            overlay: &|x, y| {
                if x < 0 || y < 0 || x as usize >= COLS || y as usize >= ROWS {
                    return None;
                }
                windows.get(y as usize * COLS + x as usize).copied().flatten()
            },
            scroll_x: 0,
            scroll_y: 0,
            sprites: &sprites,
            frame,
            limits: Limits::MODERN,
        });
        out.to_rgba(rgba);
    }
}

impl NesGame for Musicbox {
    fn bind(&mut self, inputs: &[Value]) -> Result<(), SurfaceError> {
        let [sprites, windows] = inputs else {
            return Err(SurfaceError(format!("ppu: expected {INPUTS} inputs, got {}", inputs.len())));
        };
        self.sprites = text(sprites, "sprites")?;
        self.windows = parse_windows(&text(windows, "windows")?);
        Ok(())
    }

    fn draw(&mut self, frame: u64, rgba: &mut [u8]) {
        if self.windows.is_empty() {
            self.windows = vec![None; COLS * ROWS];
        }
        self.draw_frame(frame, rgba);
    }
}

pub static REGISTRY: Registry = Registry {
    surfaces: &[("ppu", INPUTS, nes_screen::surface::<Musicbox>)],
    shaders: nes_screen::shaders::SHADERS,
};

exact_gpu::module!(REGISTRY);

/// A song's file name: its name in lower case, words joined by dashes.
pub fn slug(name: &str) -> String {
    name.to_lowercase().split_whitespace().collect::<Vec<_>>().join("-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use nes_apu::driver::render as render_song;
    use nes_apu::{driver::render_writes, wav};
    use nes_ppu::{HEIGHT, WIDTH};

    const RATE: u32 = 44100;

    fn rows_of(text: &str) -> usize {
        nes_apu::driver::tokens(text).len()
    }

    /// Renders the songs (two passes of each, for a seamless loop) and the
    /// sound effects to assets/, and writes songs.contract.
    #[test]
    #[ignore]
    fn render() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        std::fs::create_dir_all(root.join("assets/music")).unwrap();
        std::fs::create_dir_all(root.join("assets/sfx")).unwrap();
        let mut c = String::from(
            "// Written by the nes-musicbox-gpu `render` test from gpu/src/music.rs: do not\n\
             // edit. Each song's rows, per channel: what sounds (a note, a drum, or \"\"\n\
             // for silence) and its volume, 0-15.\n\n",
        );
        let mut names = vec![];
        let mut files = vec![];
        let mut row_ms = vec![];
        let mut counts = vec![];
        let mut cells = vec![];
        for song in music::SONGS.iter() {
            let rows = song.rows.iter().map(|r| rows_of(r)).filter(|&n| n > 0).fold(1, |a, n| a.max(n));
            let r = render_song(song, rows * 2, RATE);
            let file = format!("assets/music/{}.wav", slug(song.name));
            std::fs::write(root.join(&file), wav::encode(&r.samples, RATE, 1.6)).unwrap();
            names.push(format!("\"{}\"", song.name));
            files.push(format!("\"{file}\""));
            row_ms.push(format!("{:.3}", r.row_ms));
            counts.push(rows.to_string());
            let labels: Vec<String> = (0..4)
                .map(|ch| format!("[{}]", r.rows[..rows].iter().map(|cs| format!("\"{}\"", cs[ch].label)).collect::<Vec<_>>().join(", ")))
                .collect();
            let volumes: Vec<String> = (0..4)
                .map(|ch| format!("[{}]", r.rows[..rows].iter().map(|cs| cs[ch].volume.to_string()).collect::<Vec<_>>().join(", ")))
                .collect();
            cells.push((labels, volumes));
        }
        let pick = |xs: &[String]| -> String {
            let mut s = xs.last().unwrap().clone();
            for i in (0..xs.len() - 1).rev() {
                s = format!("(i == {i} ? {} : {s})", xs[i]);
            }
            s
        };
        c += &format!("fn songCount(): number = {}\n", music::SONGS.len());
        c += &format!("fn songName(i: number): string = {}\n", pick(&names));
        c += &format!("fn songFile(i: number): string = {}\n", pick(&files));
        c += &format!("fn rowMs(i: number): number = {}\n", pick(&row_ms));
        c += &format!("fn rowCount(i: number): number = {}\n", pick(&counts));
        for (i, (labels, volumes)) in cells.iter().enumerate() {
            for ch in 0..4 {
                c += &format!("fn labels{i}{ch}(): string[] = {}\n", labels[ch]);
                c += &format!("fn volumes{i}{ch}(): number[] = {}\n", volumes[ch]);
            }
        }
        for ch in 0..4 {
            let l: Vec<String> = (0..music::SONGS.len()).map(|i| format!("labels{i}{ch}()")).collect();
            let v: Vec<String> = (0..music::SONGS.len()).map(|i| format!("volumes{i}{ch}()")).collect();
            c += &format!("fn labels{ch}(i: number): string[] = {}\n", pick(&l));
            c += &format!("fn volumes{ch}(i: number): number[] = {}\n", pick(&v));
        }
        let fx = music::effects();
        let fx_names: Vec<String> = fx.iter().map(|e| format!("\"{}\"", e.name)).collect();
        let fx_files: Vec<String> = fx.iter().map(|e| format!("\"assets/sfx/{}.wav\"", e.file)).collect();
        for e in &fx {
            let samples = render_writes(&e.writes, e.frames, RATE);
            std::fs::write(root.join(format!("assets/sfx/{}.wav", e.file)), wav::encode(&samples, RATE, 2.4)).unwrap();
        }
        c += &format!("fn soundCount(): number = {}\n", fx.len());
        c += &format!("fn soundName(i: number): string = {}\n", pick(&fx_names));
        c += &format!("fn soundFile(i: number): string = {}\n", pick(&fx_files));
        std::fs::write(root.join("songs.contract"), c).unwrap();
    }

    /// A sample frame to assets/frame.png.
    #[test]
    #[ignore]
    fn frame() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut rgba = vec![0u8; WIDTH * HEIGHT * 4];
        let g = Musicbox {
            sprites: "hand:4:48;note:40:12".into(),
            windows: parse_windows("1,1,30,4|{  NES MUSICBOX}|2A03 SOUND, CYCLE BY CYCLE^1,5,18,8,2| Morning Road| Castle Nocturne^1,14,18,14,2|{Morning Road}|%0P1 E5  ~8~8~8~4|%1P2 C4  ~8~4|%2TR C3  ~8~8~8~8~8~8|%3NO K   ~8~8~8~8"),
        };
        g.draw_frame(0, &mut rgba);
        std::fs::write(root.join("assets/frame.png"), nes_ppu::png::encode(256, 240, &rgba)).unwrap();
    }
}
