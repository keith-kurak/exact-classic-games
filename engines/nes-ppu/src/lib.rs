//! An NES-style picture processor.
//!
//! It keeps the limits that give NES games their look, rather than imitating
//! the look with free-form drawing:
//!
//! - **Colours** come from the 64-entry 2C02 palette, by index.
//! - **Tiles** are 8×8 pixels, 2 bits a pixel (colours 0–3), held in
//!   pattern tables (background and sprites). The NES held 256 tiles a
//!   table; these may hold more (indices are 16-bit), for richer art.
//! - **Palettes**: 4 background and 4 sprite palettes of 3 colours each, plus
//!   one universal backdrop colour shared by every background palette.
//! - **Backgrounds** are built from 16×16 metatiles; each metatile has one
//!   palette, as the attribute table allows. Unlike the NES, a background
//!   cell may be flipped, so art can be reused.
//! - **Sprites** are 8×8 OAM entries with a palette, flips and a
//!   behind-background bit. Colour 0 is transparent. At most 64 are drawn,
//!   and at most 8 on one scanline: the rest drop out, and the OAM start
//!   rotates each frame so the dropped ones flicker instead of vanishing.
//! - **Fades** step every colour down by 0x10, as NES games did.
//!
//! The frame is 256×240 colour indices; `to_rgba` turns it into pixels.

pub mod font;
pub mod png;

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 240;
pub const MAX_SPRITES: usize = 64;
pub const SPRITES_PER_LINE: usize = 8;
pub const TABLE_SIZE: usize = 256;

/// The 2C02 palette (an NTSC approximation), indexed 0x00–0x3F.
pub const NES_PALETTE: [[u8; 3]; 64] = [
    [0x62, 0x62, 0x62], [0x00, 0x1F, 0xB2], [0x24, 0x04, 0xC8], [0x52, 0x00, 0xB2],
    [0x73, 0x00, 0x76], [0x80, 0x00, 0x24], [0x73, 0x0B, 0x00], [0x52, 0x28, 0x00],
    [0x24, 0x44, 0x00], [0x00, 0x57, 0x00], [0x00, 0x5C, 0x00], [0x00, 0x53, 0x24],
    [0x00, 0x3C, 0x76], [0x00, 0x00, 0x00], [0x00, 0x00, 0x00], [0x00, 0x00, 0x00],
    [0xAB, 0xAB, 0xAB], [0x0D, 0x57, 0xFF], [0x4B, 0x30, 0xFF], [0x8A, 0x13, 0xFF],
    [0xBC, 0x08, 0xD6], [0xD2, 0x12, 0x69], [0xC7, 0x2E, 0x00], [0x9D, 0x54, 0x00],
    [0x60, 0x7B, 0x00], [0x20, 0x98, 0x00], [0x00, 0xA3, 0x00], [0x00, 0x99, 0x42],
    [0x00, 0x7D, 0xB4], [0x00, 0x00, 0x00], [0x00, 0x00, 0x00], [0x00, 0x00, 0x00],
    [0xFF, 0xFF, 0xFF], [0x53, 0xAE, 0xFF], [0x90, 0x85, 0xFF], [0xD3, 0x65, 0xFF],
    [0xFF, 0x57, 0xFF], [0xFF, 0x5D, 0xCF], [0xFF, 0x77, 0x57], [0xFA, 0x9E, 0x00],
    [0xBD, 0xC7, 0x00], [0x7A, 0xE7, 0x00], [0x43, 0xF6, 0x11], [0x26, 0xEF, 0x7E],
    [0x2C, 0xD5, 0xF6], [0x4E, 0x4E, 0x4E], [0x00, 0x00, 0x00], [0x00, 0x00, 0x00],
    [0xFF, 0xFF, 0xFF], [0xB6, 0xE1, 0xFF], [0xCE, 0xD1, 0xFF], [0xE9, 0xC3, 0xFF],
    [0xFF, 0xBC, 0xFF], [0xFF, 0xBD, 0xF4], [0xFF, 0xC6, 0xC3], [0xFF, 0xD5, 0x9A],
    [0xE9, 0xE6, 0x81], [0xCE, 0xF4, 0x81], [0xB6, 0xFB, 0x9A], [0xA9, 0xFA, 0xC3],
    [0xA9, 0xF0, 0xF4], [0xB8, 0xB8, 0xB8], [0x00, 0x00, 0x00], [0x00, 0x00, 0x00],
];

/// The 2C02's darkest black: fades stop here.
pub const BLACK: u8 = 0x0F;

/// One 8×8 tile, 2 bits a pixel: rows of colour numbers 0–3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Tile(pub [[u8; 8]; 8]);

impl Tile {
    /// The tile in the NES's planar CHR form: 8 bytes of low bits, then 8 of high.
    pub fn to_chr(&self) -> [u8; 16] {
        let mut out = [0u8; 16];
        for (y, row) in self.0.iter().enumerate() {
            for (x, &c) in row.iter().enumerate() {
                let bit = 7 - x;
                out[y] |= (c & 1) << bit;
                out[y + 8] |= ((c >> 1) & 1) << bit;
            }
        }
        out
    }

    pub fn mirrored(&self) -> Tile {
        let mut t = *self;
        for row in t.0.iter_mut() {
            row.reverse();
        }
        t
    }

    pub fn flipped(&self) -> Tile {
        let mut t = *self;
        t.0.reverse();
        t
    }
}

/// A pattern table of distinct tiles. Adding a tile already present returns
/// its index, so art that repeats costs nothing, as on a cartridge. (Not
/// limited to the NES's 256.)
#[derive(Clone, Debug, Default)]
pub struct PatternTable {
    pub tiles: Vec<Tile>,
}

impl PatternTable {
    pub fn add(&mut self, t: Tile) -> u16 {
        if let Some(i) = self.tiles.iter().position(|x| *x == t) {
            return i as u16;
        }
        self.tiles.push(t);
        (self.tiles.len() - 1) as u16
    }

    /// Whether this table would fit an NES pattern table (256 tiles).
    pub fn fits_nes(&self) -> bool {
        self.tiles.len() <= TABLE_SIZE
    }

    pub fn get(&self, i: u16) -> &Tile {
        static EMPTY: Tile = Tile([[0; 8]; 8]);
        self.tiles.get(i as usize).unwrap_or(&EMPTY)
    }

    /// The whole table as CHR-ROM bytes, 16 a tile.
    pub fn to_chr(&self) -> Vec<u8> {
        self.tiles.iter().flat_map(|t| t.to_chr()).collect()
    }
}

/// Pixel art as text: one character a pixel, `.` or `0` for colour 0 and
/// `1`–`3` for the others; spaces and blank lines are ignored. Width and
/// height must be multiples of 8. Returns the size in tiles and the tiles,
/// row by row.
pub fn parse_art(art: &str) -> (usize, usize, Vec<Tile>) {
    let rows: Vec<Vec<u8>> = art
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| {
            l.chars()
                .filter(|c| !c.is_whitespace())
                .map(|c| match c {
                    '.' | '0' => 0,
                    '1' => 1,
                    '2' => 2,
                    '3' => 3,
                    other => panic!("art: unexpected `{other}`"),
                })
                .collect()
        })
        .collect();
    let h = rows.len();
    let w = rows.first().map(|r| r.len()).unwrap_or(0);
    assert!(w % 8 == 0 && h % 8 == 0, "art must be a multiple of 8 pixels, got {w}×{h}");
    assert!(rows.iter().all(|r| r.len() == w), "art rows differ in width");
    let (tw, th) = (w / 8, h / 8);
    let mut tiles = Vec::with_capacity(tw * th);
    for ty in 0..th {
        for tx in 0..tw {
            let mut t = Tile::default();
            for y in 0..8 {
                for x in 0..8 {
                    t.0[y][x] = rows[ty * 8 + y][tx * 8 + x];
                }
            }
            tiles.push(t);
        }
    }
    (tw, th, tiles)
}

/// Four palettes for the background and four for sprites, as 2C02 colour
/// indices. Entry 0 of background palette 0 is the universal backdrop; entry
/// 0 of every sprite palette is transparent.
#[derive(Clone, Copy, Debug)]
pub struct Palettes {
    pub bg: [[u8; 4]; 4],
    pub spr: [[u8; 4]; 4],
}

impl Palettes {
    /// Every colour stepped `steps` levels darker (0x10 a level), black at the
    /// bottom: the NES fade.
    pub fn faded(&self, steps: u8) -> Palettes {
        let dim = |c: u8| -> u8 {
            let level = c >> 4;
            if steps > level || (c & 0x0F) >= 0x0D {
                BLACK
            } else {
                c - steps * 0x10
            }
        };
        let mut p = *self;
        for pal in p.bg.iter_mut().chain(p.spr.iter_mut()) {
            for c in pal.iter_mut() {
                *c = dim(*c);
            }
        }
        p
    }
}

/// One background cell: a tile, the palette of the 16×16 area it is in, and
/// flips (FLIP_H, FLIP_V). Real NES backgrounds cannot flip tiles; this
/// engine allows it, so art can be reused (an enemy shown upside down).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub tile: u16,
    pub palette: u8,
    pub flip: u8,
}

/// A 16×16 background metatile: four tiles (top-left, top-right, bottom-left,
/// bottom-right) and the one palette the attribute table gives them.
#[derive(Clone, Copy, Debug)]
pub struct Metatile {
    pub tiles: [u16; 4],
    pub palette: u8,
}

impl Metatile {
    pub fn cell(&self, tx: usize, ty: usize) -> Cell {
        Cell {
            tile: self.tiles[(ty & 1) * 2 + (tx & 1)],
            palette: self.palette,
            flip: 0,
        }
    }
}

/// Sprite attribute bits, as in OAM byte 2.
pub const FLIP_H: u8 = 0x40;
pub const FLIP_V: u8 = 0x80;
pub const BEHIND: u8 = 0x20;

/// One OAM entry: an 8×8 sprite at a screen position.
#[derive(Clone, Copy, Debug)]
pub struct Sprite {
    pub x: i32,
    pub y: i32,
    pub tile: u16,
    /// Palette 0–3 in the low bits, plus FLIP_H, FLIP_V and BEHIND.
    pub attr: u8,
}

/// A metasprite: a figure made of 8×8 sprites, all in one palette.
#[derive(Clone, Debug)]
pub struct Metasprite {
    /// (dx, dy, tile) for each piece, unflipped.
    pub parts: Vec<(i32, i32, u16)>,
    pub width: i32,
    pub palette: u8,
}

impl Metasprite {
    /// Build a metasprite from art in the sprite table.
    pub fn from_art(table: &mut PatternTable, art: &str, palette: u8) -> Metasprite {
        let (tw, th, tiles) = parse_art(art);
        let mut parts = Vec::new();
        for ty in 0..th {
            for tx in 0..tw {
                let t = tiles[ty * tw + tx];
                // Fully transparent pieces take no OAM slot.
                if t.0.iter().all(|r| r.iter().all(|&c| c == 0)) {
                    continue;
                }
                parts.push((tx as i32 * 8, ty as i32 * 8, table.add(t)));
            }
        }
        Metasprite { parts, width: tw as i32 * 8, palette }
    }

    /// Its OAM entries at (x, y), mirrored when `flip` is set, with a palette
    /// override when one is given.
    pub fn place(&self, x: i32, y: i32, flip: bool, palette: Option<u8>, out: &mut Vec<Sprite>) {
        let pal = palette.unwrap_or(self.palette) & 3;
        for &(dx, dy, tile) in &self.parts {
            let px = if flip { x + self.width - 8 - dx } else { x + dx };
            out.push(Sprite {
                x: px,
                y: y + dy,
                tile,
                attr: pal | if flip { FLIP_H } else { 0 },
            });
        }
    }
}

/// What the picture processor draws one frame from.
pub struct Scene<'a> {
    pub bg_table: &'a PatternTable,
    pub spr_table: &'a PatternTable,
    pub palettes: Palettes,
    /// The background in world tile coordinates, before scrolling.
    pub background: &'a dyn Fn(i32, i32) -> Cell,
    /// Screen-fixed cells over the background (windows, text), by screen tile.
    pub overlay: &'a dyn Fn(i32, i32) -> Option<Cell>,
    pub scroll_x: i32,
    pub scroll_y: i32,
    pub sprites: &'a [Sprite],
    /// The frame count: it rotates OAM so lines with too many sprites flicker.
    pub frame: u64,
}

/// A drawn frame: one 2C02 colour index a pixel.
pub struct Frame {
    pub pixels: Vec<u8>,
}

impl Frame {
    pub fn to_rgba(&self, out: &mut [u8]) {
        for (i, &c) in self.pixels.iter().enumerate() {
            let rgb = NES_PALETTE[(c & 0x3F) as usize];
            out[i * 4..i * 4 + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    }
}

/// Draw one frame, scanline by scanline.
pub fn render(scene: &Scene) -> Frame {
    let mut pixels = vec![0u8; WIDTH * HEIGHT];
    let backdrop = scene.palettes.bg[0][0];
    let oam = &scene.sprites[..scene.sprites.len().min(MAX_SPRITES)];
    // With more than 8 sprites on a line, start OAM evaluation further along
    // each frame, so every sprite gets drawn some of the time.
    let rotate = if oam.is_empty() { 0 } else { (scene.frame as usize * 3) % oam.len() };
    let mut line_sprites: Vec<&Sprite> = Vec::with_capacity(SPRITES_PER_LINE);
    let mut bg_opaque = [false; WIDTH];
    for y in 0..HEIGHT as i32 {
        // Background.
        for x in 0..WIDTH as i32 {
            let (sx, sy) = (x >> 3, y >> 3);
            let (cell, px, py) = match (scene.overlay)(sx, sy) {
                Some(c) => (c, x & 7, y & 7),
                None => {
                    let wx = x + scene.scroll_x;
                    let wy = y + scene.scroll_y;
                    ((scene.background)(wx.div_euclid(8), wy.div_euclid(8)), wx.rem_euclid(8), wy.rem_euclid(8))
                }
            };
            let px = if cell.flip & FLIP_H != 0 { 7 - px } else { px };
            let py = if cell.flip & FLIP_V != 0 { 7 - py } else { py };
            let c = scene.bg_table.get(cell.tile).0[py as usize][px as usize];
            bg_opaque[x as usize] = c != 0;
            pixels[y as usize * WIDTH + x as usize] = if c == 0 {
                backdrop
            } else {
                scene.palettes.bg[(cell.palette & 3) as usize][c as usize]
            };
        }
        // Sprite evaluation: the first 8 in (rotated) OAM order on this line.
        line_sprites.clear();
        for k in 0..oam.len() {
            let s = &oam[(k + rotate) % oam.len()];
            if y >= s.y && y < s.y + 8 {
                if line_sprites.len() == SPRITES_PER_LINE {
                    break;
                }
                line_sprites.push(s);
            }
        }
        // Sprites, front to back: the first opaque pixel wins.
        for x in 0..WIDTH as i32 {
            for s in &line_sprites {
                if x < s.x || x >= s.x + 8 {
                    continue;
                }
                let mut px = (x - s.x) as usize;
                let mut py = (y - s.y) as usize;
                if s.attr & FLIP_H != 0 {
                    px = 7 - px;
                }
                if s.attr & FLIP_V != 0 {
                    py = 7 - py;
                }
                let c = scene.spr_table.get(s.tile).0[py][px];
                if c == 0 {
                    continue;
                }
                if s.attr & BEHIND == 0 || !bg_opaque[x as usize] {
                    pixels[y as usize * WIDTH + x as usize] = scene.palettes.spr[(s.attr & 3) as usize][c as usize];
                }
                break;
            }
        }
    }
    Frame { pixels }
}

/// Text in a window: one 8×8 glyph tile from up to 8 rows of up to 8
/// characters (`#` ink, anything else paper), drawn in colour `ink` on colour
/// `paper`, so text sits in the window's palette. Row 7 is for descenders.
pub fn glyph_tile(rows: &[&str], ink: u8, paper: u8) -> Tile {
    let mut t = Tile([[paper; 8]; 8]);
    for (y, row) in rows.iter().take(8).enumerate() {
        for (x, ch) in row.chars().take(8).enumerate() {
            if ch == '#' {
                t.0[y][x] = ink;
            }
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chr_is_planar() {
        let (_, _, tiles) = parse_art(
            "31000000\n00000000\n00000000\n00000000\n00000000\n00000000\n00000000\n00000000",
        );
        let chr = tiles[0].to_chr();
        assert_eq!(chr[0], 0b1100_0000);
        assert_eq!(chr[8], 0b1000_0000);
    }

    #[test]
    fn tables_dedupe() {
        let mut t = PatternTable::default();
        let a = t.add(Tile([[1; 8]; 8]));
        let b = t.add(Tile([[1; 8]; 8]));
        assert_eq!(a, b);
        assert_eq!(t.tiles.len(), 1);
    }

    #[test]
    fn nine_sprites_on_a_line_drop_one() {
        let mut spr = PatternTable::default();
        let solid = spr.add(Tile([[1; 8]; 8]));
        let bg = PatternTable::default();
        let sprites: Vec<Sprite> = (0..9).map(|i| Sprite { x: i * 10, y: 0, tile: solid, attr: 0 }).collect();
        let pal = Palettes { bg: [[0x0F; 4]; 4], spr: [[0x0F, 0x30, 0x30, 0x30]; 4] };
        let frame = render(&Scene {
            bg_table: &bg,
            spr_table: &spr,
            palettes: pal,
            background: &|_, _| Cell::default(),
            overlay: &|_, _| None,
            scroll_x: 0,
            scroll_y: 0,
            sprites: &sprites,
            frame: 0,
        });
        let drawn = (0..9).filter(|i| frame.pixels[(i * 10) as usize] == 0x30).count();
        assert_eq!(drawn, 8);
    }

    #[test]
    fn fades_reach_black() {
        let p = Palettes { bg: [[0x30, 0x21, 0x11, 0x01]; 4], spr: [[0x0F; 4]; 4] };
        assert_eq!(p.faded(1).bg[0], [0x20, 0x11, 0x01, BLACK]);
        assert_eq!(p.faded(4).bg[0], [BLACK; 4]);
    }
}
