//! Exact Gear Solid NES's GPU module: the `ppu` surface, drawn by nes-ppu
//! and shown by nes-screen, in two views.
//!
//! - From above: the room at 8 pixels a metre, its floor plates, steel
//!   containers, walls and exit cut into tiles once; each guard's view cone
//!   lights the floor cells it can see (a palette swap per 8×8 cell).
//! - Over the shoulder: a ray-cast view along the wall you are pressed to,
//!   drawn each frame and cut into tiles and palettes as a CHR-RAM game would;
//!   the guards and your shoulder are sprites scaled to their distance and
//!   hidden behind the walls in front of them.
//!
//! It breaks NES rules freely: tiles made every frame, scaled sprites, a
//! palette per 8×8 cell, no sprite limits (`Limits::MODERN`).
//!
//! Inputs: 1. view (0 above, 1 shoulder); 2. the player `x,y,fx,fy,moving,
//! boxed,step`; 3. the shoulder camera `x,y,lookX,lookY`; 4. the guards
//! `x,y,fx,fy,state;…` (state 1 half-seen, 2 spotted); 5. text `col,row|text`
//! joined by `^` (`{…}` red).

pub mod music;

use exact_gpu::json::{number, text};
use exact_gpu::{Registry, SurfaceError, Value};
use nes_ppu::font::FONT;
use nes_ppu::{glyph_tile, parse_art, render, Cell, Limits, Metasprite, Palettes, PatternTable, Scene, Sprite, Tile};
use nes_screen::NesGame;
use std::collections::HashMap;
use std::sync::OnceLock;

pub const INPUTS: usize = 5;
const COLS: usize = 32;
const ROWS: usize = 30;
/// The room's top-left on screen, and pixels a metre.
const X0: f64 = 24.0;
const Y0: f64 = 48.0;
const PPM: f64 = 8.0;
const ROOM_W: f64 = 27.0;
const ROOM_D: f64 = 20.0;

// ── The room (room.contract's geometry) ──
fn blocks() -> impl Iterator<Item = (f64, f64, f64, f64)> {
    (0..3).flat_map(|r| (0..4).map(move |c| (5.0 + 6.0 * c as f64 - 2.0, 4.75 + 5.5 * r as f64 - 1.75, 5.0 + 6.0 * c as f64 + 2.0, 4.75 + 5.5 * r as f64 + 1.75)))
}
fn in_block(x: f64, y: f64) -> bool {
    blocks().any(|(x0, y0, x1, y1)| x > x0 && x < x1 && y > y0 && y < y1)
}
fn clear_line(ax: f64, ay: f64, bx: f64, by: f64) -> bool {
    (1..10).all(|i| {
        let f = i as f64 / 10.0;
        !in_block(ax + (bx - ax) * f, ay + (by - ay) * f)
    })
}
/// room.contract's `sees`, for the view cones.
fn sees(g: &Guard, x: f64, y: f64) -> bool {
    let (dx, dy) = (x - g.x, y - g.y);
    let d2 = dx * dx + dy * dy;
    (d2 < 36.0 && dx * g.fx + dy * g.fy > 0.87 * d2.sqrt() && clear_line(g.x, g.y, x, y)) || d2 < 0.64
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
const ABOVE: [[u8; 4]; 4] = [
    [0x0F, 0x2D, 0x00, 0x18], // the room: seams, floor plates, container steel
    [0x0F, 0x2D, 0x00, 0x10], // the walls
    [0x0F, 0x0F, 0x06, 0x18], // the floor a guard can see
    [0x0F, 0x16, 0x1A, 0x30], // text (red for alerts), and the exit
];
const SHOULDER: [[u8; 4]; 4] = [
    [0x0F, 0x2D, 0x00, 0x10], // concrete: floor and walls
    [0x0F, 0x08, 0x18, 0x28], // container steel
    [0x0F, 0x00, 0x10, 0x30], // light
    [0x0F, 0x16, 0x1A, 0x30], // text (red for alerts), and the exit
];
const SPRITES: [[u8; 4]; 4] = [
    [0x0F, 0x08, 0x1A, 0x37], // you: dark hair, olive fatigues, skin
    [0x0F, 0x01, 0x11, 0x37], // the guards: navy, blue, skin
    [0x0F, 0x07, 0x17, 0x27], // the box
    [0x0F, 0x16, 0x30, 0x38], // ! and ?
];
const TEXT: u8 = 3;

// ── Sprites from above, 16×16: 1 outline and hair, 2 clothes, 3 skin ──
const DOWN: [&str; 2] = [
    ".....111111.....\n....11111111....\n....11333311....\n....13133131....\n....13333331....\n.....133331.....\n....12222221....\n...1222222221...\n..132222222231..\n..131222222131..\n...1122222211...\n....12222221....\n....12211221....\n....1221.121....\n....111..111....\n................",
    ".....111111.....\n....11111111....\n....11333311....\n....13133131....\n....13333331....\n.....133331.....\n....12222221....\n...1222222221...\n..132222222231..\n..131222222131..\n...1122222211...\n....12222221....\n....12211221....\n....121..1221...\n....111..1111...\n................",
];
const UP: [&str; 2] = [
    ".....111111.....\n....11111111....\n....11111111....\n....11111111....\n....11111111....\n.....111111.....\n....12222221....\n...1222222221...\n..132222222231..\n..131222222131..\n...1122222211...\n....12222221....\n....12211221....\n....1221.121....\n....111..111....\n................",
    ".....111111.....\n....11111111....\n....11111111....\n....11111111....\n....11111111....\n.....111111.....\n....12222221....\n...1222222221...\n..132222222231..\n..131222222131..\n...1122222211...\n....12222221....\n....12211221....\n....121..1221...\n....111..1111...\n................",
];
const LEFT: [&str; 2] = [
    ".....11111......\n....1111111.....\n...13311111.....\n...13131111.....\n...13333111.....\n....13331.......\n.....12221......\n....1222221.....\n....1322221.....\n....1322221.....\n....1122211.....\n.....12221......\n.....12221......\n.....1211.......\n....1111........\n................",
    ".....11111......\n....1111111.....\n...13311111.....\n...13131111.....\n...13333111.....\n....13331.......\n.....12221......\n....1222221.....\n....1322221.....\n....1322221.....\n....1122211.....\n.....122221.....\n....12211221....\n...1211..121....\n...111...111....\n................",
];
const BOX: &str = "................\n................\n..111111111111..\n..122222222221..\n..123333333321..\n..122222222221..\n..122233332221..\n..122223322221..\n..122222222221..\n..122222222221..\n..122222222221..\n..122222222221..\n..111111111111..\n................\n................\n................";
const BANG: &str = "..1111..\n.122221.\n.122221.\n.122221.\n.122221.\n..1221..\n..1221..\n..1221..\n...11...\n........\n..1221..\n..1221..\n...11...\n........\n........\n........";
const QUERY: &str = "..1111..\n.123321.\n12211221\n.11.1221\n....1221\n...1221.\n..1221..\n..1221..\n...11...\n........\n..1221..\n..1221..\n...11...\n........\n........\n........";

fn art(s: &str) -> String {
    s.replace('.', "0")
}

/// A soldier as the shoulder view sees one, 32×64: from the front, the back
/// or the side; you wear a bandana, the guards a cap. 1 outline and hair,
/// 2 clothes, 3 skin.
fn soldier(view: u8, bandana: bool) -> Vec<[u8; 32]> {
    let mut img = vec![[0u8; 32]; 64];
    let side = view == 2;
    let mut put = |x: i32, y: i32, c: u8| {
        if (0..32).contains(&x) && (0..64).contains(&y) {
            img[y as usize][x as usize] = c;
        }
    };
    let ell = |cx: f64, cy: f64, rx: f64, ry: f64, x: i32, y: i32| ((x as f64 + 0.5 - cx) / rx).powi(2) + ((y as f64 + 0.5 - cy) / ry).powi(2) <= 1.0;
    for y in 0..64 {
        for x in 0..32 {
            // Legs, boots.
            let (l0, l1, r0, r1) = if side { (12, 19, 13, 20) } else { (9, 15, 17, 23) };
            if (36..58).contains(&y) && ((l0..l1).contains(&x) || (r0..r1).contains(&x)) {
                put(x, y, 2);
            }
            if (58..63).contains(&y) && ((l0 - 1..l1).contains(&x) || (r0..r1 + 1).contains(&x)) {
                put(x, y, 1);
            }
            // Torso and arms.
            let (t0, t1) = if side { (11, 21) } else { (7, 25) };
            if (18..37).contains(&y) && (t0..t1).contains(&x) {
                put(x, y, 2);
            }
            if !side && (19..35).contains(&y) && ((4..8).contains(&x) || (24..28).contains(&x)) {
                put(x, y, 2);
            }
            if side && (19..34).contains(&y) && (13..18).contains(&x) {
                put(x, y, 1);
            }
            if (34..37).contains(&y) && (!side && ((4..8).contains(&x) || (24..28).contains(&x)) || side && (13..18).contains(&x)) {
                put(x, y, 3);
            }
            if (33..35).contains(&y) && (t0..t1).contains(&x) {
                put(x, y, 1);
            }
            // Neck and head.
            if (15..19).contains(&y) && (13..19).contains(&x) {
                put(x, y, 3);
            }
            if ell(16.0, 9.0, 6.0, 7.5, x, y) {
                let c = match view {
                    1 => 1,                                // the back of the head
                    2 => if x < 15 { 3 } else { 1 },      // the side: face, then hair
                    _ => if y < 5 { 1 } else { 3 },       // the front
                };
                put(x, y, c);
            }
        }
    }
    if view == 0 {
        put(13, 9, 1);
        put(18, 9, 1);
        put(14, 13, 1);
        put(15, 13, 1);
        put(16, 13, 1);
    }
    if bandana {
        for x in 9..23 {
            put(x, 5, 2);
            put(x, 6, 2);
        }
        if view != 0 {
            for (i, y) in (7..16).enumerate() {
                put(if view == 2 { 22 + i as i32 / 3 } else { 17 + i as i32 / 4 }, y, 2);
            }
        }
    } else {
        for y in 1..5 {
            for x in 9..23 {
                if ell(16.0, 5.0, 7.0, 4.0, x, y) {
                    put(x, y, 2);
                }
            }
        }
        for x in (if side { 4 } else { 8 })..(if side { 16 } else { 24 }) {
            put(x, 5, 1);
        }
    }
    // Outline: clear pixels touching the figure.
    let copy = img.clone();
    for y in 0..64i32 {
        for x in 0..32i32 {
            if copy[y as usize][x as usize] == 0
                && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(a, b)| {
                    let (u, v) = (x + a, y + b);
                    (0..32).contains(&u) && (0..64).contains(&v) && matches!(copy[v as usize][u as usize], 2 | 3)
                })
            {
                img[y as usize][x as usize] = 1;
            }
        }
    }
    img
}

struct Art {
    bg: PatternTable,
    spr: PatternTable,
    white: HashMap<char, u16>,
    green: HashMap<char, u16>,
    /// The room from above: each cell, and whether it is open floor.
    above: Vec<(Cell, bool)>,
    sprites: HashMap<&'static str, Metasprite>,
    /// Soldiers for the shoulder view: front, back and side, as guards (cap)
    /// and as you (bandana).
    guard_views: [Vec<[u8; 32]>; 3],
    you_back: Vec<[u8; 32]>,
}

/// A pixel of the room from above: its palette and colour.
fn above_pixel(px: i32, py: i32) -> (u8, u8) {
    let (wx, wy) = ((px as f64 + 0.5 - X0) / PPM, (py as f64 + 0.5 - Y0) / PPM);
    let inside = (0.0..ROOM_W).contains(&wx) && (0.0..ROOM_D).contains(&wy);
    if inside {
        for (x0, y0, x1, y1) in blocks() {
            if wx > x0 && wx < x1 && wy > y0 && wy < y1 {
                // A container from above: an outline, ribs across its lid.
                let (ex, ey) = ((wx - x0).min(x1 - wx), (wy - y0).min(y1 - wy));
                let c = if ex < 0.13 || ey < 0.13 { 1 } else if ((wx - x0) * PPM) as i32 % 4 == 0 { 2 } else { 3 };
                return (0, c);
            }
        }
        // Floor plates, 2 m, with seams.
        let sx = ((wx * PPM) as i32).rem_euclid(16);
        let sy = ((wy * PPM) as i32).rem_euclid(16);
        let c = if sx == 0 || sy == 0 || (sx == 8 && sy == 8) { 1 } else { 2 };
        return (0, c);
    }
    // The exit: a gap in the north wall.
    if wy < 0.0 && wy > -1.0 && wx > 22.5 && wx < 25.5 {
        return (3, if ((px + py) & 1) == 0 { 2 } else { 3 });
    }
    // The walls, a metre thick: concrete blocks.
    if wx > -1.0 && wx < ROOM_W + 1.0 && wy > -1.0 && wy < ROOM_D + 1.0 {
        let bx = (px + if (py / 4) % 2 == 0 { 0 } else { 4 }).rem_euclid(8);
        return (1, if py % 4 == 3 || bx == 7 { 1 } else if (px * 3 + py) % 5 == 0 { 3 } else { 2 });
    }
    (1, 0)
}

fn build() -> Art {
    let mut bg = PatternTable::default();
    let mut spr = PatternTable::default();
    bg.add(Tile::default());
    let mut white = HashMap::new();
    let mut green = HashMap::new();
    for (ch, rows) in FONT {
        white.insert(*ch, bg.add(glyph_tile(rows, 3, 0)));
        green.insert(*ch, bg.add(glyph_tile(rows, 1, 0)));
    }
    let mut above = Vec::with_capacity(COLS * ROWS);
    for ty in 0..ROWS as i32 {
        for tx in 0..COLS as i32 {
            let mut t = Tile::default();
            let mut votes = [0u32; 4];
            let mut floor = true;
            for y in 0..8 {
                for x in 0..8 {
                    let (p, c) = above_pixel(tx * 8 + x, ty * 8 + y);
                    t.0[y as usize][x as usize] = c;
                    if c != 0 {
                        votes[p as usize] += 1;
                    }
                    let (wx, wy) = (((tx * 8 + x) as f64 + 0.5 - X0) / PPM, ((ty * 8 + y) as f64 + 0.5 - Y0) / PPM);
                    if !(0.0..ROOM_W).contains(&wx) || !(0.0..ROOM_D).contains(&wy) || in_block(wx, wy) {
                        floor = false;
                    }
                }
            }
            let palette = (0..4).max_by_key(|&p| votes[p]).unwrap() as u8;
            above.push((Cell { tile: bg.add(t), palette, flip: 0 }, floor));
        }
    }
    let mut sprites = HashMap::new();
    for (name, s) in [("down0", DOWN[0]), ("down1", DOWN[1]), ("up0", UP[0]), ("up1", UP[1]), ("left0", LEFT[0]), ("left1", LEFT[1])] {
        sprites.insert(name, Metasprite::from_art(&mut spr, &art(s), 0));
    }
    sprites.insert("box", Metasprite::from_art(&mut spr, &art(BOX), 2));
    sprites.insert("bang", Metasprite::from_art(&mut spr, &art(BANG), 3));
    sprites.insert("query", Metasprite::from_art(&mut spr, &art(QUERY), 3));
    let _ = parse_art;
    Art {
        bg,
        spr,
        white,
        green,
        above,
        sprites,
        guard_views: [soldier(0, false), soldier(1, false), soldier(2, false)],
        you_back: soldier(1, true),
    }
}

fn art_ref() -> &'static Art {
    static ART: OnceLock<Art> = OnceLock::new();
    ART.get_or_init(build)
}

fn nums(s: &str) -> Vec<f64> {
    s.split(',').filter_map(|v| v.trim().parse().ok()).collect()
}

#[derive(Default)]
pub struct Game {
    view: u8,
    player: Vec<f64>,
    cam: Vec<f64>,
    guards: Vec<Guard>,
    text: String,
}

/// Text cells: white, or red inside `{…}` (alerts), in the text palette.
fn text_cells(spec: &str, cells: &mut [Option<Cell>]) {
    let art = art_ref();
    for entry in spec.split('^').filter(|s| !s.is_empty()) {
        let mut parts = entry.splitn(2, '|');
        let at: Vec<i32> = parts.next().unwrap_or("").split(',').filter_map(|v| v.trim().parse().ok()).collect();
        let [x, y] = at[..] else { continue };
        let mut col = 0;
        let mut gold = false;
        for ch in parts.next().unwrap_or("").chars() {
            if ch == '{' || ch == '}' {
                gold = ch == '{';
                continue;
            }
            let set = if gold { &art.green } else { &art.white };
            let tile = if ch == ' ' { 0 } else { *set.get(&ch).or_else(|| set.get(&ch.to_ascii_uppercase())).unwrap_or(&0) };
            let (cx, cy) = (x + col, y);
            if cx >= 0 && cy >= 0 && (cx as usize) < COLS && (cy as usize) < ROWS {
                cells[cy as usize * COLS + cx as usize] = Some(Cell { tile, palette: TEXT, flip: 0 });
            }
            col += 1;
        }
    }
}

/// The face a 16×16 overhead sprite shows for a facing.
fn frame_for(fx: f64, fy: f64, step: u8) -> (&'static str, bool) {
    let s = (step & 1) as usize;
    let names = [["down0", "down1"], ["up0", "up1"], ["left0", "left1"]];
    if fx.abs() > fy.abs() {
        (names[2][s], fx > 0.0)
    } else if fy < 0.0 {
        (names[1][s], false)
    } else {
        (names[0][s], false)
    }
}

impl Game {
    fn draw_above(&self, frame: u64, rgba: &mut [u8]) {
        let art = art_ref();
        // The floor each guard sees, lit.
        let mut cells: Vec<Option<Cell>> = vec![None; COLS * ROWS];
        let mut background = vec![Cell::default(); COLS * ROWS];
        for (i, (cell, floor)) in art.above.iter().enumerate() {
            let mut c = *cell;
            if *floor {
                let (tx, ty) = ((i % COLS) as f64, (i / COLS) as f64);
                let (wx, wy) = ((tx * 8.0 + 4.0 - X0) / PPM, (ty * 8.0 + 4.0 - Y0) / PPM);
                if self.guards.iter().any(|g| sees(g, wx, wy)) {
                    c.palette = 2;
                }
            }
            background[i] = c;
        }
        // The text rows: black behind the words.
        for y in [0, 1, 2, 3, 27, 28, 29] {
            for x in 0..COLS {
                cells[y * COLS + x] = Some(Cell::default());
            }
        }
        text_cells(&self.text, &mut cells);
        let mut sprites = Vec::new();
        let place = |name: &str, x: f64, y: f64, flip: bool, pal: Option<u8>, out: &mut Vec<Sprite>| {
            if let Some(m) = art.sprites.get(name) {
                m.place(x.round() as i32, y.round() as i32, flip, pal, out);
            }
        };
        let sx = |x: f64| X0 + x * PPM;
        let sy = |y: f64| Y0 + y * PPM;
        for g in &self.guards {
            if g.state > 0 {
                place(if g.state == 2 { "bang" } else { "query" }, sx(g.x) - 4.0, sy(g.y) - 30.0, false, None, &mut sprites);
            }
        }
        if let [x, y, fx, fy, moving, boxed, step] = self.player[..] {
            if boxed > 0.5 {
                place("box", sx(x) - 8.0, sy(y) - 10.0, false, None, &mut sprites);
            } else {
                let (name, flip) = frame_for(fx, fy, if moving > 0.5 { step as u8 } else { 0 });
                place(name, sx(x) - 8.0, sy(y) - 13.0, flip, Some(0), &mut sprites);
            }
        }
        for g in &self.guards {
            let (name, flip) = frame_for(g.fx, g.fy, ((frame / 12) & 1) as u8);
            place(name, sx(g.x) - 8.0, sy(g.y) - 13.0, flip, Some(1), &mut sprites);
        }
        let out = render(&Scene {
            bg_table: &art.bg,
            spr_table: &art.spr,
            palettes: Palettes { bg: ABOVE, spr: SPRITES },
            background: &|tx, ty| {
                if tx < 0 || ty < 0 || tx as usize >= COLS || ty as usize >= ROWS {
                    Cell::default()
                } else {
                    background[ty as usize * COLS + tx as usize]
                }
            },
            overlay: &|x, y| {
                if x < 0 || y < 0 || x as usize >= COLS || y as usize >= ROWS {
                    None
                } else {
                    cells[y as usize * COLS + x as usize]
                }
            },
            scroll_x: 0,
            scroll_y: 0,
            sprites: &sprites,
            frame,
            limits: Limits::MODERN,
        });
        out.to_rgba(rgba);
    }

    fn draw_shoulder(&self, frame: u64, rgba: &mut [u8]) {
        let art = art_ref();
        let [cx, cy, lx, ly] = self.cam[..] else { return self.draw_above(frame, rgba) };
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
        // Material (0 concrete, 1 steel, 3 exit, 255 nothing) and shade, a pixel.
        let mut mat = vec![255u8; 256 * 240];
        let mut shade = vec![0u8; 256 * 240];
        let mut zbuf = [f64::INFINITY; 256];
        let dither = |x: i32, y: i32| ((x * 7 + y * 13) & 3) as f64 / 4.0;
        for col in 0..256i32 {
            let k = (col as f64 + 0.5 - 128.0) / proj;
            let (dx, dy) = (fx + rx * k, fy + ry * k);
            // The room's walls, from inside.
            let tx = if dx > 0.0 { (ROOM_W - cx) / dx } else if dx < 0.0 { -cx / dx } else { f64::INFINITY };
            let ty = if dy > 0.0 { (ROOM_D - cy) / dy } else if dy < 0.0 { -cy / dy } else { f64::INFINITY };
            let (mut t, mut m, mut u, mut xface, mut height) = if tx < ty {
                (tx, 0u8, cy + dy * tx, true, 4.0)
            } else {
                let x = cx + dx * ty;
                (ty, if dy < 0.0 && x > 22.5 && x < 25.5 { 3 } else { 0 }, x, false, 4.0)
            };
            // The containers: the nearest face hit.
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
                    // A wall: its texture by u and height, darker with distance.
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
                    // The floor: plates, fading with distance.
                    let d = EYE * proj / (y - HORIZON);
                    let (wx, wy) = (cx + dx * d, cy + dy * d);
                    let seam = (wx / 2.0).fract().abs() < 0.04 || (wy / 2.0).fract().abs() < 0.04;
                    let s = if seam { 1 } else if d + dither(col, row) * 3.0 > 11.0 { 1 } else { 2 };
                    mat[i] = 0;
                    shade[i] = s;
                }
            }
        }
        // The view as tiles: each cell takes the palette most of its pixels
        // want (the attribute clash, at 8×8), and every pixel its shade.
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
        for y in [0, 1, 2, 3, 27, 28, 29] {
            for x in 0..COLS {
                cells[y * COLS + x] = Some(Cell::default());
            }
        }
        text_cells(&self.text, &mut cells);
        // The figures: guards, and you in front, each scaled to its distance
        // and cut where a nearer wall hides it, then into sprite tiles.
        let mut spr = art.spr.clone();
        let mut sprites = Vec::new();
        let mut figures: Vec<(f64, f64, &Vec<[u8; 32]>, bool, u8, u8)> = Vec::new();
        for g in &self.guards {
            let (vx, vy) = (cx - g.x, cy - g.y);
            let toward = vx * g.fx + vy * g.fy;
            let across = vx * g.fy - vy * g.fx;
            let d = (vx * vx + vy * vy).sqrt().max(1e-6);
            let (view, flip) = if toward > 0.5 * d { (0, false) } else if toward < -0.5 * d { (1, false) } else { (2, across > 0.0) };
            figures.push((g.x, g.y, &art.guard_views[view], flip, 1, g.state));
        }
        if let [x, y, ..] = self.player[..] {
            figures.push((x, y, &art.you_back, false, 0, 0));
        }
        let mut drawn: Vec<(f64, Vec<Sprite>)> = Vec::new();
        for (x, y, img, flip, pal, state) in figures {
            let (vx, vy) = (x - cx, y - cy);
            let z = vx * fx + vy * fy;
            if z < 0.4 {
                continue;
            }
            let side = vx * rx + vy * ry;
            let h = 1.8 * proj / z;
            let w = h / 2.0;
            let mid = 128.0 + side * proj / z;
            let feet = HORIZON + EYE * proj / z;
            let (left, top) = ((mid - w / 2.0).floor() as i32, (feet - h).floor() as i32);
            let (wi, hi) = (w.ceil() as i32 + 1, h.ceil() as i32 + 1);
            // Into an image on an 8-pixel grid, then tiles.
            let (gx, gy) = (left.div_euclid(8) * 8, top.div_euclid(8) * 8);
            let (gw, gh) = (((left + wi - gx) + 7) / 8, ((top + hi - gy) + 7) / 8);
            let mut tiles = vec![Tile::default(); (gw * gh) as usize];
            let mut any = vec![false; (gw * gh) as usize];
            for sxp in left.max(0)..(left + wi).min(256) {
                if zbuf[sxp as usize] < z - 0.05 {
                    continue;
                }
                let mut u = (((sxp - left) as f64 + 0.5) / w * 32.0) as usize;
                if u >= 32 {
                    continue;
                }
                if flip {
                    u = 31 - u;
                }
                for syp in top.max(TOP)..(top + hi).min(BOTTOM) {
                    let v = (((syp - top) as f64 + 0.5) / h * 64.0) as usize;
                    if v >= 64 {
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
            if state > 0 && zbuf[(mid as i32).clamp(0, 255) as usize] >= z - 0.05 {
                if let Some(m) = art.sprites.get(if state == 2 { "bang" } else { "query" }) {
                    m.place(mid as i32 - 4, (top - 18).max(TOP), false, None, &mut out);
                }
            }
            drawn.push((z, out));
        }
        // Nearer figures in front: earlier in OAM.
        drawn.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        for (_, s) in drawn {
            sprites.extend(s);
        }
        let out = render(&Scene {
            bg_table: &bg,
            spr_table: &spr,
            palettes: Palettes { bg: SHOULDER, spr: SPRITES },
            background: &|_, _| Cell::default(),
            overlay: &|x, y| {
                if x < 0 || y < 0 || x as usize >= COLS || y as usize >= ROWS {
                    None
                } else {
                    cells[y as usize * COLS + x as usize]
                }
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
        if self.view == 1 {
            self.draw_shoulder(frame, rgba)
        } else {
            self.draw_above(frame, rgba)
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

    /// The guards at their starting posts (room.contract's routes).
    fn guards() -> String {
        "2,7.5,1,0,0;14,1.5,0,1,0;20,1.5,1,0,1;8,13,0,1,0".into()
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

    /// Two sample frames to assets/ (`cargo test -p exact-gear-solid-nes-gpu frames -- --ignored`).
    #[test]
    #[ignore]
    fn frames() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets");
        let mut rgba = vec![0u8; WIDTH * HEIGHT * 4];
        let mut g = Game { view: 0, player: vec![1.5, 18.5, 0.0, -1.0, 0.0, 0.0, 0.0], cam: vec![], guards: vec![], text: "1,1|STORAGE ROOM B^20,1|TIME 0:00^1,2|ITEM NONE^18,2|{CAUTION}^1,28|PUSH INTO WALLS. Q BOX".into() };
        g.bind_guards(&guards());
        g.draw_above(0, &mut rgba);
        std::fs::write(root.join("frame-above.png"), nes_ppu::png::encode(256, 240, &rgba)).unwrap();
        // Pressed to the west face of the bottom-left container's... the
        // aisle at x = 8, looking north along a container's side.
        g.view = 1;
        g.player = vec![7.3, 16.0, 1.0, 0.0, 0.0, 0.0, 0.0];
        g.cam = vec![7.3 + 0.95, 16.0 + 2.1, 7.3 + 0.7, 16.0 - 5.0];
        g.draw_shoulder(0, &mut rgba);
        std::fs::write(root.join("frame-shoulder.png"), nes_ppu::png::encode(256, 240, &rgba)).unwrap();
    }

    #[test]
    fn cones_match_the_rules() {
        let g = Guard { x: 2.0, y: 7.5, fx: 1.0, fy: 0.0, state: 0 };
        assert!(sees(&g, 6.0, 7.5));
        assert!(!sees(&g, 9.0, 7.5), "past 6 m");
        assert!(!sees(&g, 0.5, 7.5), "behind");
        let art = art_ref();
        assert!(art.above.iter().filter(|(_, f)| *f).count() > 300, "the open floor");
    }

    impl Game {
        fn bind_guards(&mut self, s: &str) {
            self.guards = s.split(';').filter_map(|g| match nums(g)[..] {
                [x, y, fx, fy, st] => Some(Guard { x, y, fx, fy, state: st as u8 }),
                _ => None,
            }).collect();
        }
    }
}
