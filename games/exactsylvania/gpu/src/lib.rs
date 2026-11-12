//! Exactsylvania's GPU module: the `ppu` surface, an NES-style picture from
//! the shared engines (nes-ppu draws it, nes-screen shows it). The view says
//! what is on screen in small text specs:
//!
//! 1. scroll x, in pixels: the level scrolls sideways under the HUD.
//! 2. sprites: `name:x:y[:flags]` joined by `;`, screen pixels, top-left.
//!    Flags: `h` mirrors it (facing left), a digit 0–3 overrides the palette.
//! 3. text: `col,row|text` joined by `^`, in tiles, fixed on screen (the HUD
//!    and banners). Escapes: `~a` `~b` `~c` a player-bar tile with 0, 1 or 2
//!    halves filled, `~x` `~y` `~z` the same for the enemy bar, `~h` a heart,
//!    `~d` `~e` the dagger icon (two tiles), `~_` a solid black tile.
//! 4. fade: 0 (full colour) to 4 (black).

mod data;
mod level;
mod sheets;

/// What sheets.rs describes: a frame of a sprite sheet, and the grid of
/// background tiles.
pub mod sheets_types {
    pub struct Frame {
        pub name: &'static str,
        pub x: usize,
        pub y: usize,
        pub w: usize,
        pub h: usize,
        /// The sprite palette it is drawn in (0–3).
        pub palette: u8,
    }
    pub struct Tiles {
        /// Each tile's size in pixels, and how many a row of the sheet holds.
        pub size: usize,
        pub per_row: usize,
        /// The map character each tile is, and its background palette.
        pub cells: &'static [(char, u8)],
    }
}

/// The PNG sheets, embedded at build.
const SPRITE_SHEETS: [(&str, &[u8], &[sheets_types::Frame]); 4] = [
    ("hero.png", include_bytes!("../../assets/sprites/hero.png"), sheets::HERO),
    ("enemies.png", include_bytes!("../../assets/sprites/enemies.png"), sheets::ENEMIES),
    ("boss.png", include_bytes!("../../assets/sprites/boss.png"), sheets::BOSS),
    ("items.png", include_bytes!("../../assets/sprites/items.png"), sheets::ITEMS),
];
const TILES_PNG: &[u8] = include_bytes!("../../assets/sprites/tiles.png");
const HUD_PNG: &[u8] = include_bytes!("../../assets/sprites/hud.png");

use data::{BG_PALETTES, SPRITE_PALETTES};
use sheets::{HUD, TILES};
use exact_gpu::json::{number, text};
use exact_gpu::{Registry, SurfaceError, Value};
use level::LEVEL;
use nes_ppu::font::FONT;
use nes_ppu::{glyph_tile, parse_art, render, Cell, Metasprite, Metatile, Palettes, PatternTable, Limits, Scene, Sprite, Tile};
use nes_screen::NesGame;
use std::collections::HashMap;
use std::sync::OnceLock;

pub const INPUTS: usize = 4;

const COLS: usize = 32;
const ROWS: usize = 30;
/// The HUD's height in tiles: the level is drawn below it.
const HUD_ROWS: i32 = 4;
const HUD_PALETTE: u8 = 3;

pub struct Art {
    pub bg: PatternTable,
    pub spr: PatternTable,
    pub metatiles: HashMap<char, Metatile>,
    pub sprites: HashMap<&'static str, Metasprite>,
    /// HUD tiles by glyph or escape.
    pub glyphs: HashMap<char, u16>,
    pub escapes: HashMap<char, u16>,
}

/// The art, every bit of it from the PNG sheets in assets/sprites/ (embedded
/// at build, so a broken sheet fails the build's tests), laid out as
/// sheets.rs says. Each pixel becomes the nearest colour of its frame's
/// palette: draw the sheets in those exact colours (transparent for clear).
pub fn build() -> Art {
    let mut bg = PatternTable::default();
    let mut spr = PatternTable::default();
    bg.add(Tile::default());
    let mut glyphs = HashMap::new();
    for (ch, rows) in FONT {
        glyphs.insert(*ch, bg.add(glyph_tile(rows, 3, 0)));
    }
    let load = |file: &str, bytes: &[u8]| nes_ppu::png::decode(bytes).unwrap_or_else(|e| panic!("assets/sprites/{file}: {e}"));
    // Sprites: every frame of every sprite sheet.
    let mut sprites = HashMap::new();
    for (file, bytes, frames) in SPRITE_SHEETS {
        let sheet = load(file, bytes);
        for f in frames.iter() {
            assert!(f.x + f.w <= sheet.width && f.y + f.h <= sheet.height, "{file}: {} is outside the sheet", f.name);
            let [_, c1, c2, c3] = SPRITE_PALETTES[f.palette as usize];
            sprites.insert(f.name, Metasprite::from_art(&mut spr, &sheet.art(f.x, f.y, f.w, f.h, [c1, c2, c3]), f.palette));
        }
    }
    // Background: 16×16 metatiles, each in its own palette.
    let tiles = load("tiles.png", TILES_PNG);
    let mut metatiles = HashMap::new();
    for (i, (ch, pal)) in TILES.cells.iter().enumerate() {
        let (x, y) = ((i % TILES.per_row) * TILES.size, (i / TILES.per_row) * TILES.size);
        let (_, _, t) = parse_art(&tiles.bg_art(x, y, TILES.size, TILES.size, BG_PALETTES[*pal as usize]));
        metatiles.insert(*ch, Metatile { tiles: [bg.add(t[0]), bg.add(t[1]), bg.add(t[2]), bg.add(t[3])], palette: *pal });
    }
    // The HUD's tiles, by the escape the text uses for each.
    let hud = load("hud.png", HUD_PNG);
    let mut escapes = HashMap::new();
    for (i, name) in HUD.iter().enumerate() {
        let (_, _, t) = parse_art(&hud.bg_art(i * 8, 0, 8, 8, BG_PALETTES[HUD_PALETTE as usize]));
        let ch = match *name {
            "bar20" => 'a', "bar21" => 'b', "bar22" => 'c', "bar10" => 'x', "bar11" => 'y', "bar12" => 'z',
            "heart" => 'h', "dagger0" => 'd', "dagger1" => 'e', other => panic!("hud.png: unknown tile {other}"),
        };
        escapes.insert(ch, bg.add(t[0]));
    }
    escapes.insert('_', 0);
    Art { bg, spr, metatiles, sprites, glyphs, escapes }
}

fn art() -> &'static Art {
    static ART: OnceLock<Art> = OnceLock::new();
    ART.get_or_init(build)
}

fn parse_sprites(spec: &str, out: &mut Vec<Sprite>) {
    let art = art();
    for item in spec.split(';').filter(|s| !s.is_empty()) {
        let mut f = item.split(':');
        let name = f.next().unwrap_or("");
        let x = f.next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0).round() as i32;
        let y = f.next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0).round() as i32;
        let flags = f.next().unwrap_or("");
        let pal = flags.chars().find(|c| c.is_ascii_digit()).map(|c| c as u8 - b'0');
        if let Some(m) = art.sprites.get(name) {
            m.place(x, y, flags.contains('h'), pal, out);
        }
    }
}

fn parse_text(spec: &str) -> Vec<Option<Cell>> {
    let art = art();
    let mut cells = vec![None; COLS * ROWS];
    for entry in spec.split('^').filter(|s| !s.is_empty()) {
        let mut parts = entry.splitn(2, '|');
        let at: Vec<i32> = parts.next().unwrap_or("").split(',').filter_map(|v| v.trim().parse().ok()).collect();
        let [x, y] = at[..] else { continue };
        let mut col = 0;
        let mut chars = parts.next().unwrap_or("").chars();
        while let Some(ch) = chars.next() {
            let tile = if ch == '~' {
                chars.next().and_then(|e| art.escapes.get(&e).copied()).unwrap_or(0)
            } else if ch == ' ' {
                0
            } else {
                *art.glyphs.get(&ch).or_else(|| art.glyphs.get(&ch.to_ascii_uppercase())).unwrap_or(&0)
            };
            let (cx, cy) = (x + col, y);
            if cx >= 0 && cy >= 0 && (cx as usize) < COLS && (cy as usize) < ROWS {
                cells[cy as usize * COLS + cx as usize] = Some(Cell { tile, palette: HUD_PALETTE, flip: 0 });
            }
            col += 1;
        }
    }
    cells
}

#[derive(Default)]
pub struct Game {
    scroll: i32,
    sprites: String,
    text: Vec<Option<Cell>>,
    fade: u8,
}

impl Game {
    pub fn draw_frame(&self, frame: u64, rgba: &mut [u8]) {
        let art = art();
        let palettes = Palettes { bg: BG_PALETTES, spr: SPRITE_PALETTES }.faded(self.fade);
        let background = |tx: i32, ty: i32| -> Cell {
            let row = ty - HUD_ROWS;
            if row < 0 {
                return Cell::default();
            }
            let (mx, my) = (tx.div_euclid(2), row.div_euclid(2));
            let ch = LEVEL.get(my as usize).and_then(|r| r.chars().nth(mx as usize).filter(|_| mx >= 0)).unwrap_or('#');
            art.metatiles.get(&ch).map(|m| m.cell(tx.rem_euclid(2) as usize, row.rem_euclid(2) as usize)).unwrap_or_default()
        };
        let text = &self.text;
        let overlay = |x: i32, y: i32| -> Option<Cell> {
            if y < HUD_ROWS && x >= 0 && (x as usize) < COLS {
                // The HUD band is solid black under its text.
                return Some(text.get(y as usize * COLS + x as usize).copied().flatten().unwrap_or_default());
            }
            if x < 0 || y < 0 || x as usize >= COLS || y as usize >= ROWS {
                return None;
            }
            text[y as usize * COLS + x as usize]
        };
        let mut sprites = Vec::new();
        parse_sprites(&self.sprites, &mut sprites);
        // Nothing drawn over the HUD.
        sprites.retain(|s| s.y >= HUD_ROWS * 8 - 7);
        let out = render(&Scene {
            bg_table: &art.bg,
            spr_table: &art.spr,
            palettes,
            background: &background,
            overlay: &overlay,
            scroll_x: self.scroll,
            scroll_y: 0,
            sprites: &sprites,
            frame,
            // A modern game in NES dress: every sprite, every frame, no flicker.
            limits: Limits::MODERN,
        });
        out.to_rgba(rgba);
    }
}

impl NesGame for Game {
    fn bind(&mut self, inputs: &[Value]) -> Result<(), SurfaceError> {
        let [scroll, sprites, text_spec, fade] = inputs else {
            return Err(SurfaceError(format!("ppu: expected {INPUTS} inputs, got {}", inputs.len())));
        };
        self.scroll = number(scroll, "scroll")?.round() as i32;
        self.sprites = text(sprites, "sprites")?;
        self.text = parse_text(&text(text_spec, "text")?);
        self.fade = number(fade, "fade")?.clamp(0.0, 4.0) as u8;
        Ok(())
    }

    fn draw(&mut self, frame: u64, rgba: &mut [u8]) {
        self.draw_frame(frame, rgba);
    }
}

/// Each surface: its name, its number of inputs, its factory.
pub static REGISTRY: Registry = Registry {
    surfaces: &[("ppu", INPUTS, nes_screen::surface::<Game>)],
    shaders: nes_screen::shaders::SHADERS,
};

exact_gpu::module!(REGISTRY);

#[cfg(test)]
mod tests {
    use super::*;
    use nes_ppu::{HEIGHT, WIDTH};

    #[test]
    fn level_and_art_agree() {
        let art = build();
        for row in LEVEL {
            assert_eq!(row.chars().count(), 80);
            for ch in row.chars() {
                assert!(art.metatiles.contains_key(&ch), "no metatile for {ch:?}");
            }
        }
    }

    /// Writes two sample frames to assets/ (`cargo test -p exactsylvania-gpu frames -- --ignored`).
    #[test]
    #[ignore]
    fn frames() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets");
        std::fs::create_dir_all(&dir).unwrap();
        let mut rgba = vec![0u8; WIDTH * HEIGHT * 4];
        let hud = "1,0|SCORE-000000  TIME 0300  STAGE 01^1,1|PLAYER ~c~c~c~c~c~c~c~b  ~h-05^1,2|ENEMY  ~z~z~z~z~z~z~z~z  ~d~e";
        let mut g = Game { scroll: 0, sprites: "stand:60:176;candle0:68:144;ghoul0:150:176:h;bat0:120:100".into(), text: parse_text(hud), fade: 0 };
        g.draw_frame(0, &mut rgba);
        std::fs::write(dir.join("frame-courtyard.png"), nes_ppu::png::encode(256, 240, &rgba)).unwrap();
        g.scroll = 560;
        g.sprites = "whipfore:80:176;lash:90:180;knight0:200:144:h;candle1:120:112;bigheart:150:192".into();
        g.draw_frame(0, &mut rgba);
        std::fs::write(dir.join("frame-hall.png"), nes_ppu::png::encode(256, 240, &rgba)).unwrap();
    }
}
