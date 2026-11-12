//! Exact Gear Solid NES's GPU module: the `ppu` surface, drawn by nes-ppu and
//! shown by nes-screen.
//!
//! - The 3D view: a ray caster draws the room each frame from a camera behind
//!   you (or, on a wall, beside you looking along it), and cuts the picture
//!   into tiles and per-cell palettes, as a CHR-RAM game would. The guards and
//!   you are sprites scaled to their distance and cut where a nearer wall
//!   hides them; on a wall you stand in profile, back to it.
//! - A minimap in the corner, a pixel a metre: the room in green, the guards
//!   as red dots, you in white.
//! - The radio: Dr. Vale and you in codec frames, the frequency, the words.
//!
//! It breaks NES rules freely: new tiles every frame, scaled sprites, a palette
//! per 8×8 cell, no sprite limits (`Limits::MODERN`).
//!
//! Inputs: 1. view (1 the 3D view, 2 the radio); 2. you `x,y,fx,fy,moving,
//! boxed,step,smoking` (on a wall, f is away from it); 3. the wall camera
//! `x,y,lookX,lookY`, or "" to follow behind you; 4. the guards
//! `x,y,fx,fy,state;…` (state 1 half-seen, 2 spotted); 5. text `col,row|text`
//! joined by `^` (`{…}` red); an entry `@call|who|smiling|freq|words` lays out
//! the radio.

pub mod music;

use exact_gpu::json::{number, text};
use exact_gpu::{Registry, SurfaceError, Value};
use nes_ppu::font::FONT;
use nes_ppu::{glyph_tile, render, Cell, Limits, Metasprite, Palettes, PatternTable, Scene, Sprite, Tile};
use nes_screen::NesGame;
use std::collections::HashMap;
use std::sync::OnceLock;

pub const INPUTS: usize = 5;
const COLS: usize = 32;
const ROWS: usize = 30;
const ROOM_W: f64 = 27.0;
const ROOM_D: f64 = 20.0;

// ── The room (room.contract's geometry) ──
fn blocks() -> impl Iterator<Item = (f64, f64, f64, f64)> {
    (0..3).flat_map(|r| (0..4).map(move |c| (5.0 + 6.0 * c as f64 - 2.0, 4.75 + 5.5 * r as f64 - 1.75, 5.0 + 6.0 * c as f64 + 2.0, 4.75 + 5.5 * r as f64 + 1.75)))
}
fn in_block(x: f64, y: f64, m: f64) -> bool {
    blocks().any(|(x0, y0, x1, y1)| x > x0 - m && x < x1 + m && y > y0 - m && y < y1 + m)
}
fn open_floor(x: f64, y: f64, m: f64) -> bool {
    x > m && x < ROOM_W - m && y > m && y < ROOM_D - m && !in_block(x, y, m)
}

#[derive(Clone, Copy, Debug, Default)]
struct Guard {
    x: f64,
    y: f64,
    fx: f64,
    fy: f64,
    state: u8,
}

// ── Palettes ──
const VIEW: [[u8; 4]; 4] = [
    [0x0F, 0x2D, 0x00, 0x10], // concrete: floor and walls
    [0x0F, 0x08, 0x18, 0x28], // container steel
    [0x0F, 0x00, 0x10, 0x30], // light
    [0x0F, 0x16, 0x1A, 0x30], // text, the minimap (red, green, white), the exit
];
const CALL: [[u8; 4]; 4] = [
    [0x0F, 0x0A, 0x1A, 0x3A], // a portrait, lit
    [0x0F, 0x0F, 0x0A, 0x1A], // a portrait, dim
    [0x0F, 0x1A, 0x2A, 0x3A], // the frames
    [0x0F, 0x16, 0x1A, 0x30], // text
];
const SPRITES: [[u8; 4]; 4] = [
    [0x0F, 0x08, 0x1A, 0x37], // you: dark hair, olive fatigues, skin
    [0x0F, 0x01, 0x11, 0x37], // the guards: navy, blue, skin
    [0x0F, 0x07, 0x17, 0x27], // the box
    [0x0F, 0x16, 0x30, 0x38], // ! ? and smoke
];
const TEXT: u8 = 3;

const BANG: &str = "..1111..\n.122221.\n.122221.\n.122221.\n.122221.\n..1221..\n..1221..\n..1221..\n...11...\n........\n..1221..\n..1221..\n...11...\n........\n........\n........";
const QUERY: &str = "..1111..\n.123321.\n12211221\n.11.1221\n....1221\n...1221.\n..1221..\n..1221..\n...11...\n........\n..1221..\n..1221..\n...11...\n........\n........\n........";
const SMOKE: [&str; 2] = [
    "........\n...22...\n..2222..\n.22.222.\n..2222..\n...22...\n........\n........",
    "........\n..2.22..\n.222222.\n22.2.222\n.222222.\n..22.2..\n........\n........",
];

fn art(s: &str) -> String {
    s.replace('.', "0")
}

type Image = Vec<Vec<u8>>;

fn image(w: usize, h: usize) -> Image {
    vec![vec![0; w]; h]
}

fn ell(cx: f64, cy: f64, rx: f64, ry: f64, x: usize, y: usize) -> bool {
    ((x as f64 + 0.5 - cx) / rx).powi(2) + ((y as f64 + 0.5 - cy) / ry).powi(2) <= 1.0
}

/// Outline: clear pixels touching colours 2 or 3 turn 1.
fn outline(img: &mut Image) {
    let copy = img.clone();
    let (h, w) = (copy.len() as i32, copy[0].len() as i32);
    for y in 0..h {
        for x in 0..w {
            if copy[y as usize][x as usize] == 0
                && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(a, b)| {
                    let (u, v) = (x + a, y + b);
                    (0..w).contains(&u) && (0..h).contains(&v) && matches!(copy[v as usize][u as usize], 2 | 3)
                })
            {
                img[y as usize][x as usize] = 1;
            }
        }
    }
}

/// A soldier, 32×64: front (0), back (1) or side, facing left (2); you wear
/// a bandana, the guards a cap. 1 outline and hair, 2 clothes, 3 skin.
fn soldier(view: u8, bandana: bool) -> Image {
    let mut img = image(32, 64);
    let side = view == 2;
    for y in 0..64usize {
        for x in 0..32usize {
            let (l0, l1, r0, r1) = if side { (12, 19, 13, 20) } else { (9, 15, 17, 23) };
            let mut c = 0;
            if (36..58).contains(&y) && ((l0..l1).contains(&x) || (r0..r1).contains(&x)) {
                c = 2;
            }
            if (58..63).contains(&y) && ((l0 - 1..l1).contains(&x) || (r0..r1 + 1).contains(&x)) {
                c = 1;
            }
            let (t0, t1) = if side { (11, 21) } else { (7, 25) };
            if (18..37).contains(&y) && (t0..t1).contains(&x) {
                c = 2;
            }
            if !side && (19..35).contains(&y) && ((4..8).contains(&x) || (24..28).contains(&x)) {
                c = 2;
            }
            if side && (19..34).contains(&y) && (16..21).contains(&x) {
                c = 1;
            }
            if (34..37).contains(&y) && ((!side && ((4..8).contains(&x) || (24..28).contains(&x))) || (side && (16..21).contains(&x))) {
                c = 3;
            }
            if (33..35).contains(&y) && (t0..t1).contains(&x) {
                c = 1;
            }
            if (15..19).contains(&y) && (13..19).contains(&x) {
                c = 3;
            }
            if ell(16.0, 9.0, 6.0, 7.5, x, y) {
                c = match view {
                    1 => 1,
                    2 => if x < 15 { 3 } else { 1 },
                    _ => if y < 5 { 1 } else { 3 },
                };
            }
            if c != 0 {
                img[y][x] = c;
            }
        }
    }
    if view == 0 {
        for (x, y) in [(13, 9), (18, 9), (14, 13), (15, 13), (16, 13)] {
            img[y][x] = 1;
        }
    }
    if view == 2 {
        img[9][11] = 1;
    }
    if bandana {
        for x in 9..23 {
            img[5][x] = 2;
            img[6][x] = 2;
        }
        if view != 0 {
            for (i, y) in (7..16).enumerate() {
                img[y][if view == 2 { 22 + i / 3 } else { 17 + i / 4 }] = 2;
            }
        }
    } else {
        for y in 1..5 {
            for x in 9..23 {
                if ell(16.0, 5.0, 7.0, 4.0, x, y) {
                    img[y][x] = 2;
                }
            }
        }
        for x in (if side { 4 } else { 8 })..(if side { 16 } else { 24 }) {
            img[5][x] = 1;
        }
    }
    outline(&mut img);
    img
}

/// The cardboard box, 32×32: a front face, a lid, tape, a pair of eyes.
fn cardboard() -> Image {
    let mut img = image(32, 32);
    for y in 4..30 {
        for x in 2..30 {
            img[y][x] = if y < 10 { 3 } else if x == 15 || x == 16 { 3 } else { 2 };
        }
    }
    for x in 2..30 {
        img[4][x] = 1;
        img[10][x] = 1;
        img[29][x] = 1;
    }
    for y in 4..30 {
        img[y][2] = 1;
        img[y][29] = 1;
    }
    for (x, y) in [(10, 18), (11, 18), (12, 19), (19, 18), (20, 18), (21, 19)] {
        img[y][x] = 1;
    }
    img
}

/// A codec portrait, 40×48, in three greens: Dr. Vale (upset, or smiling),
/// or you. 1 dark, 2 mid, 3 light.
fn portrait(who: u8) -> Image {
    let mut img = image(40, 48);
    let vale = who < 2;
    let smile = who == 1;
    for y in 0..48 {
        for x in 0..40 {
            let mut c = 0;
            if y >= 40 && (4..36).contains(&x) {
                c = if vale && (17..23).contains(&x) && y < 46 { 3 } else { 2 };
            }
            if vale {
                if ell(20.0, 21.0, 15.0, 17.0, x, y) || (y > 20 && y < 42 && ((5..11).contains(&x) || (29..35).contains(&x))) {
                    c = 1;
                }
                if ell(20.0, 25.0, 11.0, 13.0, x, y) && y > 15 {
                    c = 3;
                }
                // A fringe, cut in points.
                if (14..19).contains(&y) && ell(20.0, 25.0, 11.5, 13.0, x, y) && y < 15 + (x % 4) {
                    c = 1;
                }
            } else {
                if ell(20.0, 24.0, 10.0, 14.0, x, y) {
                    c = 3;
                }
                if y < 17 && ell(20.0, 20.0, 11.0, 12.0, x, y) {
                    c = 1;
                }
                // The jaw: stubble.
                if y > 30 && ell(20.0, 24.0, 10.0, 14.0, x, y) && (x + y) % 3 == 0 {
                    c = 2;
                }
                if (36..41).contains(&y) && (15..25).contains(&x) {
                    c = 3;
                }
            }
            if c != 0 {
                img[y][x] = c;
            }
        }
    }
    let mut dot = |pts: &[(usize, usize)], c: u8| {
        for &(x, y) in pts {
            img[y][x] = c;
        }
    };
    if vale {
        // A hair clip.
        dot(&[(28, 12), (29, 12), (28, 13), (29, 13)], 2);
        if smile {
            // Eyes closed in a smile, a smile, blush.
            dot(&[(13, 25), (14, 24), (15, 24), (16, 25), (23, 25), (24, 24), (25, 24), (26, 25)], 1);
            dot(&[(17, 32), (18, 33), (19, 33), (20, 33), (21, 33), (22, 32)], 1);
            dot(&[(12, 29), (13, 29), (26, 29), (27, 29)], 2);
        } else {
            // Big eyes under brows drawn down in a frown, a small frown.
            dot(&[(13, 24), (14, 24), (15, 24), (13, 25), (15, 25), (13, 26), (14, 26), (15, 26), (24, 24), (25, 24), (26, 24), (24, 25), (26, 25), (24, 26), (25, 26), (26, 26)], 1);
            dot(&[(14, 25), (25, 25)], 3);
            dot(&[(12, 20), (13, 20), (14, 21), (15, 21), (16, 22), (27, 20), (26, 20), (25, 21), (24, 21), (23, 22)], 1);
            dot(&[(18, 33), (19, 32), (20, 32), (21, 32), (22, 33)], 1);
        }
    } else {
        // The bandana and its tails; narrowed eyes under heavy brows; a scar.
        let mut band = Vec::new();
        for x in 9..32 {
            band.push((x, 14));
            band.push((x, 15));
        }
        dot(&band, 2);
        dot(&[(31, 16), (32, 17), (33, 18), (32, 19), (34, 19), (35, 20)], 2);
        let mut eyes = Vec::new();
        for x in 12..18 {
            eyes.push((x, 21));
        }
        for x in 22..28 {
            eyes.push((x, 21));
        }
        for x in 14..18 {
            eyes.push((x, 23));
        }
        for x in 22..26 {
            eyes.push((x, 23));
        }
        dot(&eyes, 1);
        dot(&[(12, 20), (13, 20), (27, 20), (26, 20), (17, 22), (22, 22)], 1);
        dot(&[(25, 25), (26, 26), (26, 27), (27, 28)], 2);
        dot(&[(17, 33), (18, 33), (19, 33), (20, 33), (21, 33), (22, 33)], 1);
    }
    outline(&mut img);
    img
}

/// An image as tiles, row by row.
fn tiles_of(img: &Image, table: &mut PatternTable) -> Vec<u16> {
    let (h, w) = (img.len() / 8, img[0].len() / 8);
    let mut ids = Vec::new();
    for ty in 0..h {
        for tx in 0..w {
            let mut t = Tile::default();
            for y in 0..8 {
                for x in 0..8 {
                    t.0[y][x] = img[ty * 8 + y][tx * 8 + x];
                }
            }
            ids.push(table.add(t));
        }
    }
    ids
}

struct Art {
    bg: PatternTable,
    spr: PatternTable,
    white: HashMap<char, u16>,
    red: HashMap<char, u16>,
    sprites: HashMap<&'static str, Metasprite>,
    guard_views: [Image; 3],
    you_back: Image,
    you_side: Image,
    box_img: Image,
    /// Codec portraits as tiles (5×6): Vale upset, Vale smiling, you; and the
    /// frame's corner and edge.
    portraits: [Vec<u16>; 3],
    frame: [u16; 3],
}

fn build() -> Art {
    let mut bg = PatternTable::default();
    let mut spr = PatternTable::default();
    bg.add(Tile::default());
    let mut white = HashMap::new();
    let mut red = HashMap::new();
    for (ch, rows) in FONT {
        white.insert(*ch, bg.add(glyph_tile(rows, 3, 0)));
        red.insert(*ch, bg.add(glyph_tile(rows, 1, 0)));
    }
    let portraits = [tiles_of(&portrait(0), &mut bg), tiles_of(&portrait(1), &mut bg), tiles_of(&portrait(2), &mut bg)];
    let mut corner = Tile::default();
    let mut edge = Tile::default();
    let mut side = Tile::default();
    for i in 0..8 {
        corner.0[3][i.max(3)] = 1;
        corner.0[i.max(3)][3] = 1;
        edge.0[3][i] = 1;
        side.0[i][3] = 1;
    }
    let frame = [bg.add(corner), bg.add(edge), bg.add(side)];
    let mut sprites = HashMap::new();
    sprites.insert("bang", Metasprite::from_art(&mut spr, &art(BANG), 3));
    sprites.insert("query", Metasprite::from_art(&mut spr, &art(QUERY), 3));
    sprites.insert("smoke0", Metasprite::from_art(&mut spr, &art(SMOKE[0]), 3));
    sprites.insert("smoke1", Metasprite::from_art(&mut spr, &art(SMOKE[1]), 3));
    Art {
        bg,
        spr,
        white,
        red,
        sprites,
        guard_views: [soldier(0, false), soldier(1, false), soldier(2, false)],
        you_back: soldier(1, true),
        you_side: soldier(2, true),
        box_img: cardboard(),
        portraits,
        frame,
    }
}

fn art_ref() -> &'static Art {
    static ART: OnceLock<Art> = OnceLock::new();
    ART.get_or_init(build)
}

fn nums(s: &str) -> Vec<f64> {
    s.split(',').filter_map(|v| v.trim().parse().ok()).collect()
}

/// Words wrapped to `width` letters.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in text.split(' ') {
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

/// The radio as the text spec describes it.
struct Call {
    who: String,
    smiling: bool,
    freq: String,
    words: String,
}

#[derive(Default)]
pub struct Game {
    view: u8,
    player: Vec<f64>,
    cam: Vec<f64>,
    guards: Vec<Guard>,
    text: String,
}

fn put_text(cells: &mut [Option<Cell>], x: i32, y: i32, line: &str) {
    let art = art_ref();
    let mut col = 0;
    let mut alert = false;
    for ch in line.chars() {
        if ch == '{' || ch == '}' {
            alert = ch == '{';
            continue;
        }
        let set = if alert { &art.red } else { &art.white };
        let tile = if ch == ' ' { 0 } else { *set.get(&ch).or_else(|| set.get(&ch.to_ascii_uppercase())).unwrap_or(&0) };
        let (cx, cy) = (x + col, y);
        if cx >= 0 && cy >= 0 && (cx as usize) < COLS && (cy as usize) < ROWS {
            cells[cy as usize * COLS + cx as usize] = Some(Cell { tile, palette: TEXT, flip: 0 });
        }
        col += 1;
    }
}

/// The text entries, and the call if one is described.
fn text_cells(spec: &str, cells: &mut [Option<Cell>]) -> Option<Call> {
    let mut call = None;
    for entry in spec.split('^').filter(|s| !s.is_empty()) {
        if let Some(rest) = entry.strip_prefix("@call|") {
            let mut f = rest.splitn(4, '|');
            call = Some(Call {
                who: f.next().unwrap_or("").into(),
                smiling: f.next() == Some("1"),
                freq: f.next().unwrap_or("").into(),
                words: f.next().unwrap_or("").into(),
            });
            continue;
        }
        let mut parts = entry.splitn(2, '|');
        let at: Vec<i32> = parts.next().unwrap_or("").split(',').filter_map(|v| v.trim().parse().ok()).collect();
        if let [x, y] = at[..] {
            put_text(cells, x, y, parts.next().unwrap_or(""));
        }
    }
    call
}

/// Black behind the text rows.
fn text_rows(cells: &mut [Option<Cell>]) {
    for y in [0, 1, 2, 3, 27, 28, 29] {
        for x in 0..COLS {
            cells[y * COLS + x] = Some(Cell::default());
        }
    }
}

impl Game {
    /// The camera: the wall camera if given, else behind you over your right
    /// shoulder, pulled in until it stands on open floor with you in sight.
    fn camera(&self) -> Option<(f64, f64, f64, f64)> {
        if let [cx, cy, lx, ly] = self.cam[..] {
            return Some((cx, cy, lx, ly));
        }
        let [px, py, fx, fy, ..] = self.player[..] else { return None };
        let len = (fx * fx + fy * fy).sqrt().max(1e-6);
        let (fx, fy) = (fx / len, fy / len);
        let (rx, ry) = (-fy, fx);
        let mut d = 2.6;
        while d > 0.6 {
            let (cx, cy) = (px - fx * d + rx * 0.4, py - fy * d + ry * 0.4);
            let clear = (1..10).all(|i| {
                let f = i as f64 / 10.0;
                open_floor(px + (cx - px) * f, py + (cy - py) * f, 0.15)
            });
            if open_floor(cx, cy, 0.2) && clear {
                return Some((cx, cy, cx + fx * 6.0, cy + fy * 6.0));
            }
            d -= 0.1;
        }
        Some((px - fx * 0.6, py - fy * 0.6, px + fx * 6.0, py + fy * 6.0))
    }

    fn draw_view(&self, frame: u64, rgba: &mut [u8]) {
        let art = art_ref();
        let Some((cx, cy, lx, ly)) = self.camera() else { return };
        let (mut fx, mut fy) = (lx - cx, ly - cy);
        let len = (fx * fx + fy * fy).sqrt().max(1e-6);
        fx /= len;
        fy /= len;
        let (rx, ry) = (-fy, fx);
        const TOP: i32 = 32;
        const BOTTOM: i32 = 216;
        const HORIZON: f64 = 124.0;
        const EYE: f64 = 1.55;
        let proj = 128.0 / (33f64.to_radians()).tan();
        // Material (0 concrete, 1 steel, 3 exit, 255 nothing) and shade.
        let mut mat = vec![255u8; 256 * 240];
        let mut shade = vec![0u8; 256 * 240];
        let mut zbuf = [f64::INFINITY; 256];
        let dither = |x: i32, y: i32| ((x * 7 + y * 13) & 3) as f64 / 4.0;
        for col in 0..256i32 {
            let k = (col as f64 + 0.5 - 128.0) / proj;
            let (dx, dy) = (fx + rx * k, fy + ry * k);
            let tx = if dx > 0.0 { (ROOM_W - cx) / dx } else if dx < 0.0 { -cx / dx } else { f64::INFINITY };
            let ty = if dy > 0.0 { (ROOM_D - cy) / dy } else if dy < 0.0 { -cy / dy } else { f64::INFINITY };
            let (mut t, mut m, mut u, mut xface, mut height) = if tx < ty {
                (tx, 0u8, cy + dy * tx, true, 4.0)
            } else {
                let x = cx + dx * ty;
                (ty, if dy < 0.0 && x > 22.5 && x < 25.5 { 3 } else { 0 }, x, false, 4.0)
            };
            for (x0, y0, x1, y1) in blocks() {
                let (t0x, t1x) = if dx != 0.0 { let (a, b) = ((x0 - cx) / dx, (x1 - cx) / dx); (a.min(b), a.max(b)) } else if cx > x0 && cx < x1 { (f64::NEG_INFINITY, f64::INFINITY) } else { continue };
                let (t0y, t1y) = if dy != 0.0 { let (a, b) = ((y0 - cy) / dy, (y1 - cy) / dy); (a.min(b), a.max(b)) } else if cy > y0 && cy < y1 { (f64::NEG_INFINITY, f64::INFINITY) } else { continue };
                let enter = t0x.max(t0y);
                if enter > 0.05 && enter <= t1x.min(t1y) && enter < t {
                    t = enter;
                    m = 1;
                    xface = t0x > t0y;
                    u = if xface { cy + dy * enter } else { cx + dx * enter };
                    height = 2.8;
                }
            }
            zbuf[col as usize] = t;
            let top = HORIZON - (height - EYE) * proj / t;
            let bottom = HORIZON + EYE * proj / t;
            for row in TOP..BOTTOM {
                let i = row as usize * 256 + col as usize;
                let y = row as f64 + 0.5;
                if y >= top && y < bottom {
                    let h = (bottom - y) / (bottom - top) * height;
                    let near = (3.2 - t / 4.5 - if xface { 0.6 } else { 0.0 } + dither(col, row)).clamp(1.0, 3.0);
                    let base = near.floor() as u8;
                    let s = match m {
                        1 => {
                            let rib = (u * 4.0).fract() < 0.18;
                            if h > height - 0.12 || h < 0.15 { 1 } else if rib { base.saturating_sub(1).max(1) } else { base }
                        }
                        3 => if h < 2.6 { 2 + ((col + row) & 1) as u8 } else { 2 },
                        _ => {
                            let course = (h * 2.0).floor() as i32;
                            let seam = (h * 2.0).fract() < 0.08 || ((u + if course % 2 == 0 { 0.0 } else { 0.5 }).fract() < 0.04);
                            if seam { 1 } else { base.min(2) }
                        }
                    };
                    mat[i] = m;
                    shade[i] = s;
                } else if y >= bottom && y > HORIZON {
                    let d = EYE * proj / (y - HORIZON);
                    let (wx, wy) = (cx + dx * d, cy + dy * d);
                    let seam = (wx / 2.0).fract().abs() < 0.04 || (wy / 2.0).fract().abs() < 0.04;
                    let s = if seam { 1 } else if d + dither(col, row) * 3.0 > 11.0 { 1 } else { 2 };
                    mat[i] = 0;
                    shade[i] = s;
                }
            }
        }
        // The picture as tiles: each cell takes the palette most of its
        // pixels want, every pixel its shade.
        let mut bg = art.bg.clone();
        let mut cells: Vec<Option<Cell>> = vec![None; COLS * ROWS];
        for ty in (TOP / 8)..(BOTTOM / 8) {
            for tx in 0..COLS as i32 {
                let mut t = Tile::default();
                let mut votes = [0u32; 4];
                for y in 0..8 {
                    for x in 0..8 {
                        let i = (ty * 8 + y) as usize * 256 + (tx * 8 + x) as usize;
                        if mat[i] != 255 {
                            votes[mat[i] as usize] += 1;
                            t.0[y as usize][x as usize] = shade[i];
                        }
                    }
                }
                let palette = (0..4).max_by_key(|&p| votes[p]).unwrap() as u8;
                bg.tiles.push(t);
                cells[ty as usize * COLS + tx as usize] = Some(Cell { tile: (bg.tiles.len() - 1) as u16, palette, flip: 0 });
            }
        }
        text_rows(&mut cells);
        text_cells(&self.text, &mut cells);
        self.minimap(&mut bg, &mut cells, frame);
        // The figures, scaled to their distance, cut where a wall is nearer.
        let mut spr = art.spr.clone();
        let mut drawn: Vec<(f64, Vec<Sprite>)> = Vec::new();
        let on_wall = self.cam.len() == 4;
        let smoking = self.player.get(7).copied().unwrap_or(0.0) > 0.5;
        // (x, y, image, mirrored, palette, alert, height in metres, where on
        // the image the figure's back is, 0–1, for standing on a wall).
        let mut figures: Vec<(f64, f64, &Image, bool, u8, u8, f64, f64)> = Vec::new();
        for g in &self.guards {
            let (vx, vy) = (cx - g.x, cy - g.y);
            let toward = vx * g.fx + vy * g.fy;
            let across = vx * g.fy - vy * g.fx;
            let d = (vx * vx + vy * vy).sqrt().max(1e-6);
            let (view, flip) = if toward > 0.5 * d { (0, false) } else if toward < -0.5 * d { (1, false) } else { (2, across > 0.0) };
            figures.push((g.x, g.y, &art.guard_views[view], flip, 1, g.state, 1.8, 0.5));
        }
        if let [px, py, pfx, pfy, _, boxed, ..] = self.player[..] {
            if boxed > 0.5 {
                figures.push((px, py, &art.box_img, false, 2, 0, 1.0, 0.5));
            } else if on_wall {
                // In profile, facing away from the wall, its back against it:
                // the side view faces left, its back at the right.
                let away_right = pfx * rx + pfy * ry > 0.0;
                figures.push((px, py, &art.you_side, away_right, 0, 0, 1.8, if away_right { 0.3 } else { 0.7 }));
            } else {
                figures.push((px, py, &art.you_back, false, 0, 0, 1.8, 0.5));
            }
        }
        for (x, y, img, flip, pal, state, tall, back) in figures {
            let (vx, vy) = (x - cx, y - cy);
            let z = vx * fx + vy * fy;
            if z < 0.35 {
                continue;
            }
            let side = vx * rx + vy * ry;
            let (iw, ih) = (img[0].len(), img.len());
            let h = tall * proj / z;
            let w = h * iw as f64 / ih as f64;
            // On a wall: shift you so your back, not your middle, is where the
            // wall is (0.3 m behind where you stand).
            let at = 128.0 + side * proj / z;
            let wall_at = if on_wall && pal == 0 {
                let (wx, wy) = (x - (self.player[2]) * 0.3 - cx, y - (self.player[3]) * 0.3 - cy);
                let wz = (wx * fx + wy * fy).max(0.35);
                Some(128.0 + (wx * rx + wy * ry) * proj / wz)
            } else {
                None
            };
            let left_f = match wall_at {
                Some(wa) => wa - back * w,
                None => at - w / 2.0,
            };
            let mid = left_f + w / 2.0;
            let feet = HORIZON + EYE * proj / z;
            let (left, top) = (left_f.floor() as i32, (feet - h).floor() as i32);
            let (wi, hi) = (w.ceil() as i32 + 1, h.ceil() as i32 + 1);
            let (gx, gy) = (left.div_euclid(8) * 8, top.div_euclid(8) * 8);
            let (gw, gh) = (((left + wi - gx) + 7) / 8, ((top + hi - gy) + 7) / 8);
            let mut tiles = vec![Tile::default(); (gw * gh) as usize];
            let mut any = vec![false; (gw * gh) as usize];
            for sxp in left.max(0)..(left + wi).min(256) {
                if zbuf[sxp as usize] < z - 0.05 {
                    continue;
                }
                let mut u = (((sxp - left) as f64 + 0.5) / w * iw as f64) as usize;
                if u >= iw {
                    continue;
                }
                if flip {
                    u = iw - 1 - u;
                }
                for syp in top.max(TOP)..(top + hi).min(BOTTOM) {
                    let v = (((syp - top) as f64 + 0.5) / h * ih as f64) as usize;
                    if v >= ih {
                        continue;
                    }
                    let c = img[v][u];
                    if c == 0 {
                        continue;
                    }
                    let (lx, ly) = (sxp - gx, syp - gy);
                    let ti = ((ly / 8) * gw + lx / 8) as usize;
                    tiles[ti].0[(ly % 8) as usize][(lx % 8) as usize] = c;
                    any[ti] = true;
                }
            }
            let mut out = Vec::new();
            for (i, t) in tiles.into_iter().enumerate() {
                if any[i] {
                    spr.tiles.push(t);
                    let (ix, iy) = (i as i32 % gw, i as i32 / gw);
                    out.push(Sprite { x: gx + ix * 8, y: gy + iy * 8, tile: (spr.tiles.len() - 1) as u16, attr: pal });
                }
            }
            let visible = zbuf[(mid as i32).clamp(0, 255) as usize] >= z - 0.05;
            if state > 0 && visible {
                if let Some(m) = art.sprites.get(if state == 2 { "bang" } else { "query" }) {
                    m.place(mid as i32 - 4, (top - 18).max(TOP), false, None, &mut out);
                }
            }
            // A smoke, drifting up from your head.
            if pal == 0 && smoking {
                let rise = (frame % 60) as i32 / 4;
                let name = if (frame / 15) % 2 == 0 { "smoke0" } else { "smoke1" };
                if let Some(m) = art.sprites.get(name) {
                    m.place(mid as i32 - 2 + rise / 3, (top - 6 - rise).max(TOP), false, None, &mut out);
                }
            }
            drawn.push((z, out));
        }
        drawn.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let sprites: Vec<Sprite> = drawn.into_iter().flat_map(|(_, s)| s).collect();
        let out = render(&Scene {
            bg_table: &bg,
            spr_table: &spr,
            palettes: Palettes { bg: VIEW, spr: SPRITES },
            background: &|_, _| Cell::default(),
            overlay: &|x, y| if x < 0 || y < 0 || x as usize >= COLS || y as usize >= ROWS { None } else { cells[y as usize * COLS + x as usize] },
            scroll_x: 0,
            scroll_y: 0,
            sprites: &sprites,
            frame,
            limits: Limits::MODERN,
        });
        out.to_rgba(rgba);
    }

    /// The minimap, top right, a pixel a metre: walls and containers in
    /// green, the exit in white, the guards red dots, you a white dot that
    /// blinks.
    fn minimap(&self, bg: &mut PatternTable, cells: &mut [Option<Cell>], frame: u64) {
        const MX: usize = 27; // its first tile column
        const W: usize = 4;
        const H: usize = 3;
        let mut img = image(W * 8, H * 8);
        let (ox, oy) = (1usize, 1usize);
        for y in 0..22usize {
            for x in 0..29usize {
                let (wx, wy) = (x as f64 - 0.5, y as f64 - 0.5);
                let wall = x == 0 || x == 28 || y == 0 || y == 21;
                let c = if wall { if y == 0 && (24..27).contains(&x) { 3 } else { 2 } } else if in_block(wx, wy, 0.0) { 2 } else { 0 };
                if c != 0 && ox + x < W * 8 && oy + y < H * 8 {
                    img[oy + y][ox + x] = c;
                }
            }
        }
        let mut dot = |x: f64, y: f64, c: u8| {
            let (px, py) = ((x + 0.5).floor() as usize + 1 + ox, (y + 0.5).floor() as usize + 1 + oy);
            if px < W * 8 && py < H * 8 {
                img[py][px] = c;
            }
        };
        for g in &self.guards {
            dot(g.x, g.y, 1);
        }
        if let [px, py, ..] = self.player[..] {
            if (frame / 20) % 3 != 0 {
                dot(px, py, 3);
            }
        }
        for ty in 0..H {
            for tx in 0..W {
                let mut t = Tile::default();
                for y in 0..8 {
                    for x in 0..8 {
                        t.0[y][x] = img[ty * 8 + y][tx * 8 + x];
                    }
                }
                bg.tiles.push(t);
                cells[ty * COLS + MX + tx] = Some(Cell { tile: (bg.tiles.len() - 1) as u16, palette: TEXT, flip: 0 });
            }
        }
    }

    /// The radio: Dr. Vale and you in codec frames, the frequency between,
    /// and the words below.
    fn draw_call(&self, frame: u64, rgba: &mut [u8]) {
        let art = art_ref();
        let mut cells: Vec<Option<Cell>> = vec![Some(Cell::default()); COLS * ROWS];
        let call = text_cells(&self.text, &mut cells).unwrap_or(Call { who: String::new(), smiling: false, freq: String::new(), words: String::new() });
        let vale_lit = call.who == "VALE" || call.smiling;
        let you_lit = call.who == "YOU";
        let [corner, edge, side] = art.frame;
        for (x0, tiles, lit) in [(2usize, &art.portraits[if call.smiling { 1 } else { 0 }], vale_lit), (23, &art.portraits[2], you_lit)] {
            // A frame of 7×8 tiles around a 5×6 portrait.
            for y in 0..8usize {
                for x in 0..7usize {
                    let (tile, flip) = match (x, y) {
                        (0, 0) => (corner, 0),
                        (6, 0) => (corner, nes_ppu::FLIP_H),
                        (0, 7) => (corner, nes_ppu::FLIP_V),
                        (6, 7) => (corner, nes_ppu::FLIP_H | nes_ppu::FLIP_V),
                        (_, 0) => (edge, 0),
                        (_, 7) => (edge, nes_ppu::FLIP_V),
                        (0, _) => (side, 0),
                        (6, _) => (side, nes_ppu::FLIP_H),
                        _ => (0, 0),
                    };
                    cells[(5 + y) * COLS + x0 + x] = Some(Cell { tile, palette: 2, flip });
                }
            }
            for (i, &tile) in tiles.iter().enumerate().take(30) {
                let (tx, ty) = (i % 5, i / 5);
                cells[(6 + ty) * COLS + x0 + 1 + tx] = Some(Cell { tile, palette: if lit { 0 } else { 1 }, flip: 0 });
            }
        }
        put_text(&mut cells, 14, 6, if call.who == "VALE" { "{TX}" } else { "RX" });
        put_text(&mut cells, 13, 8, &call.freq);
        let bars = ((frame / 6) % 6) as usize + 1;
        put_text(&mut cells, 13, 10, &format!("{{{}}}", "*".repeat(bars)));
        put_text(&mut cells, 2, 16, if call.who == "VALE" { "{DR. VALE}" } else { "{YOU}" });
        for (i, line) in wrap(&call.words.to_uppercase(), 28).iter().take(4).enumerate() {
            put_text(&mut cells, 2, 18 + i as i32 * 2, line);
        }
        let out = render(&Scene {
            bg_table: &art.bg,
            spr_table: &art.spr,
            palettes: Palettes { bg: CALL, spr: SPRITES },
            background: &|_, _| Cell::default(),
            overlay: &|x, y| if x < 0 || y < 0 || x as usize >= COLS || y as usize >= ROWS { None } else { cells[y as usize * COLS + x as usize] },
            scroll_x: 0,
            scroll_y: 0,
            sprites: &[],
            frame,
            limits: Limits::MODERN,
        });
        out.to_rgba(rgba);
    }
}

impl NesGame for Game {
    fn bind(&mut self, inputs: &[Value]) -> Result<(), SurfaceError> {
        let [view, player, cam, guards, words] = inputs else {
            return Err(SurfaceError(format!("ppu: expected {INPUTS} inputs, got {}", inputs.len())));
        };
        self.view = number(view, "view")? as u8;
        self.player = nums(&text(player, "player")?);
        self.cam = nums(&text(cam, "camera")?);
        self.guards = text(guards, "guards")?
            .split(';')
            .filter_map(|g| match nums(g)[..] {
                [x, y, fx, fy, s] => Some(Guard { x, y, fx, fy, state: s as u8 }),
                _ => None,
            })
            .collect();
        self.text = text(words, "text")?;
        Ok(())
    }

    fn draw(&mut self, frame: u64, rgba: &mut [u8]) {
        if self.view == 2 {
            self.draw_call(frame, rgba)
        } else {
            self.draw_view(frame, rgba)
        }
    }
}

pub static REGISTRY: Registry = Registry {
    surfaces: &[("ppu", INPUTS, nes_screen::surface::<Game>)],
    shaders: nes_screen::shaders::SHADERS,
};

exact_gpu::module!(REGISTRY);

#[cfg(test)]
mod tests {
    use super::*;
    use nes_apu::{driver, wav};
    use nes_ppu::{HEIGHT, WIDTH};

    const RATE: u32 = 44100;

    fn guards() -> Vec<Guard> {
        vec![
            Guard { x: 2.0, y: 7.5, fx: 1.0, fy: 0.0, state: 0 },
            Guard { x: 14.0, y: 9.5, fx: 0.0, fy: 1.0, state: 1 },
            Guard { x: 20.0, y: 1.5, fx: 1.0, fy: 0.0, state: 0 },
            Guard { x: 8.0, y: 13.0, fx: 0.0, fy: 1.0, state: 0 },
        ]
    }

    /// Renders the music and sounds to assets/ (`cargo test --release -p
    /// exact-gear-solid-nes-gpu render -- --ignored`).
    #[test]
    #[ignore]
    fn render() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets");
        std::fs::create_dir_all(root.join("music")).unwrap();
        std::fs::create_dir_all(root.join("sfx")).unwrap();
        let rows = music::INFILTRATION.rows.iter().map(|r| driver::tokens(r).len()).max().unwrap();
        let song = driver::render(&music::INFILTRATION, rows, RATE);
        std::fs::write(root.join("music/infiltration.wav"), wav::encode(&song.samples, RATE, 2.0)).unwrap();
        for e in music::effects() {
            let s = driver::render_writes(&e.writes, e.frames, RATE);
            std::fs::write(root.join(format!("sfx/{}.wav", e.file)), wav::encode(&s, RATE, 2.2)).unwrap();
        }
    }

    /// Sample frames to assets/ (`cargo test -p exact-gear-solid-nes-gpu frames -- --ignored`).
    #[test]
    #[ignore]
    fn frames() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets");
        let mut rgba = vec![0u8; WIDTH * HEIGHT * 4];
        let hud = "1,1|STORAGE ROOM B^16,1|TIME 0:12^1,2|ITEM CIGS^13,2|{CAUTION}^1,28|WALK INTO WALLS. Q ITEM";
        let mut g = Game { view: 1, player: vec![14.0, 16.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0], cam: vec![], guards: guards(), text: hud.into() };
        g.draw_view(30, &mut rgba);
        std::fs::write(root.join("frame-behind.png"), nes_ppu::png::encode(256, 240, &rgba)).unwrap();
        // On the east face of a container, looking north along it.
        g.player = vec![7.3, 16.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        g.cam = vec![7.3 + 0.95, 16.0 + 2.1, 7.3 + 0.7, 16.0 - 5.0];
        g.draw_view(0, &mut rgba);
        std::fs::write(root.join("frame-wall.png"), nes_ppu::png::encode(256, 240, &rgba)).unwrap();
        g.view = 2;
        g.text = format!("{hud}^@call|VALE|0|142.37|Carbon monoxide grips your blood 200 times tighter than oxygen.");
        g.draw_call(0, &mut rgba);
        std::fs::write(root.join("frame-call.png"), nes_ppu::png::encode(256, 240, &rgba)).unwrap();
    }

    #[test]
    fn the_chase_camera_stays_on_open_floor() {
        let g = Game { view: 1, player: vec![1.5, 18.5, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0], cam: vec![], guards: vec![], text: String::new() };
        let (cx, cy, ..) = g.camera().unwrap();
        assert!(open_floor(cx, cy, 0.1), "{cx},{cy}");
    }
}
