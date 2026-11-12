//! Exact Space Dodgeball's GPU module: the `s16` surface, drawn by
//! engines/sys16 (a Super Scaler-style picture processor, 320×224) and shown
//! by nes-screen. The camera rides behind you; everything coming at you is
//! one piece of 4-bit art zoomed to its distance.
//!
//! Inputs: 1. you `x,y,lean,hurt,tick`; 2. the distance travelled; 3. the
//! things `kind,x,y,z,size,colour;…` (kind 2 a pillar, the rest balls); 4. text
//! `col,row|text` joined by `^` (`{…}` yellow).

use exact_gpu::json::text;
use exact_gpu::{Registry, SurfaceError, Value};
use nes_screen::NesGame;
use std::collections::HashMap;
use std::sync::OnceLock;
use sys16::{font_tiles, render, rgb8, Ground, Image, Layer, Palette, Scene, Sprite, Tiles, HEIGHT, WIDTH};

pub const INPUTS: usize = 4;
/// The camera: how far behind you it rides, and its focal length in pixels.
const BEHIND: f32 = 4.5;
const FOCAL: f32 = 260.0;
const PILLAR_HEIGHT: f32 = 6.0;

// ── Palettes: a ramp of 12 shades (1–12), then a few extras ──
fn ramp(dark: (u8, u8, u8), light: (u8, u8, u8), extra: [(u8, u8, u8); 3]) -> Palette {
    let mut p = [0u16; 16];
    for i in 1..=12 {
        let t = (i - 1) as f32 / 11.0;
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
        p[i] = rgb8(m(dark.0, light.0), m(dark.1, light.1), m(dark.2, light.2));
    }
    for (k, c) in extra.iter().enumerate() {
        p[13 + k] = rgb8(c.0, c.1, c.2);
    }
    p
}

const YOU: usize = 0;
const BALLS: usize = 1; // 1–4
const PILLAR: usize = 5;
const PLANET: usize = 6;
const HILLS: usize = 7;
const TEXT: u8 = 8;
const GOLD_TEXT: u8 = 9;
const STARS: u8 = 10;

fn palettes() -> Vec<Palette> {
    vec![
        ramp((20, 30, 90), (235, 240, 255), [(255, 255, 255), (255, 120, 20), (255, 230, 90)]), // you: suit, flame
        ramp((90, 0, 10), (255, 110, 90), [(255, 255, 255), (60, 0, 0), (0, 0, 0)]),             // red ball
        ramp((0, 20, 110), (110, 190, 255), [(255, 255, 255), (0, 10, 50), (0, 0, 0)]),          // blue ball
        ramp((120, 70, 0), (255, 240, 110), [(255, 255, 255), (80, 40, 0), (0, 0, 0)]),          // yellow ball
        ramp((60, 0, 90), (230, 130, 255), [(255, 255, 255), (30, 0, 50), (0, 0, 0)]),           // purple ball
        ramp((40, 40, 70), (210, 210, 230), [(255, 255, 255), (170, 120, 20), (255, 220, 90)]),  // pillar: stone, gold
        ramp((110, 40, 10), (255, 190, 110), [(255, 240, 200), (230, 200, 160), (140, 110, 90)]), // planet, ring
        ramp((30, 10, 60), (170, 70, 170), [(255, 255, 255), (0, 0, 0), (0, 0, 0)]),             // hills
        { let mut p = [0u16; 16]; p[1] = rgb8(255, 255, 255); p[2] = rgb8(20, 20, 70); p }, // text
        { let mut p = [0u16; 16]; p[1] = rgb8(255, 220, 60); p[2] = rgb8(90, 30, 0); p },     // gold text
        { let mut p = [0u16; 16]; p[1] = rgb8(255, 255, 255); p[2] = rgb8(140, 140, 200); p }, // stars
    ]
}

// ── Art ──
fn shade_index(lum: f32) -> u8 {
    1 + (lum.clamp(0.0, 1.0) * 11.0).round() as u8
}

/// A dodgeball, 64×64: lit from the upper left, two curved seams, a shine.
fn ball() -> Image {
    let mut img = Image::new(64, 64);
    for y in 0..64 {
        for x in 0..64 {
            let (dx, dy) = ((x as f32 + 0.5 - 32.0) / 31.5, (y as f32 + 0.5 - 32.0) / 31.5);
            let r2 = dx * dx + dy * dy;
            if r2 > 1.0 {
                continue;
            }
            let nz = (1.0 - r2).sqrt();
            let lum = -0.45 * dx - 0.55 * dy + 0.7 * nz;
            let mut c = shade_index(lum * 0.95 + 0.05);
            let seam1 = (dy - 0.22 * (dx * 3.0).sin() * nz).abs() < 0.045;
            let seam2 = (dx * 0.9 + 0.3 * dy * dy - 0.1).abs() < 0.04;
            if seam1 || seam2 {
                c = 14;
            }
            if (dx + 0.38).powi(2) + (dy + 0.42).powi(2) < 0.012 {
                c = 13;
            }
            img.set(x, y, c);
        }
    }
    img
}

/// A stone pillar, 32×128: a fluted column, a capital and a base, gold bands.
fn pillar() -> Image {
    let mut img = Image::new(32, 128);
    for y in 0..128 {
        let slab = y < 8 || y >= 120;
        let half = if slab { 16.0 } else { 12.0 };
        for x in 0..32 {
            let nx = (x as f32 + 0.5 - 16.0) / half;
            if nx.abs() > 1.0 {
                continue;
            }
            let lum = 0.35 - 0.55 * nx + 0.45 * (1.0 - nx * nx).sqrt();
            let mut c = shade_index(lum);
            // Flutes, stone courses, gold bands.
            if !slab && (x % 4 == 0) {
                c = c.saturating_sub(2).max(1);
            }
            if y % 16 == 15 && !slab {
                c = c.saturating_sub(3).max(1);
            }
            if (8..12).contains(&y) || (116..120).contains(&y) {
                c = if lum > 0.55 { 15 } else { 14 };
            }
            img.set(x, y, c);
        }
    }
    img
}

/// You, from behind, 48×64, flying: helmet, suit, a jet pack, arms out,
/// legs trailing. `lean` 1 banks right (−1 is its mirror); the flame flickers.
fn hero(lean: i32, flame: bool) -> Image {
    hero_pose(lean, flame, false, false)
}

/// The hero in any pose: `flung` throws the arms up and the legs apart (hit),
/// `kneel` folds the legs (getting up), and neither has the jets lit.
fn hero_pose(lean: i32, flame: bool, flung: bool, kneel: bool) -> Image {
    let mut img = Image::new(48, 64);
    let tilt = lean as f32 * 2.0;
    let leg_end = if kneel { 50 } else { 62 };
    let boot = if kneel { 46 } else { 56 };
    let lit = |x: f32, cx: f32, w: f32| shade_index(0.75 - (x - cx) / w * 0.45);
    // Legs, trailing, a little apart.
    for y in 38..leg_end {
        let spread = (y - 38) as f32 * if flung { 0.45 } else if kneel { 0.3 } else { 0.12 };
        for (lx, rx) in [(17.0 - spread - tilt * 0.3, 22.0 - spread - tilt * 0.3), (26.0 + spread - tilt * 0.3, 31.0 + spread - tilt * 0.3)] {
            for x in lx.round() as i32..rx.round() as i32 {
                let c = if y >= boot { 3 } else { lit(x as f32, (lx + rx) / 2.0, 6.0) };
                img.set(x as usize, y as usize, c);
            }
        }
    }
    // Arms out: the banked side lower.
    for (sx, sign) in [(15.0f32, -1.0f32), (33.0, 1.0)] {
        let drop = if flung { -14.0 } else if kneel { 13.0 } else { 10.0 + sign * lean as f32 * 4.0 };
        for t in 0..14 {
            let (ax, ay) = (sx + sign * t as f32 * 0.8 + tilt, 21.0 + t as f32 * drop / 14.0);
            for w in -2..2 {
                img.set((ax + w as f32) as usize, ay as usize, lit(ax, 24.0, 20.0));
                img.set((ax + w as f32) as usize, ay as usize + 1, lit(ax, 24.0, 20.0));
            }
        }
        // Gloves.
        let (gx, gy) = (sx + sign * 12.0 + tilt, 21.0 + drop);
        for dy in 0..3 {
            for dx in -2..2 {
                img.set((gx + dx as f32) as usize, (gy + dy as f32) as usize, 13);
            }
        }
    }
    // Torso.
    for y in 17..39 {
        for x in 15..34 {
            let xx = x as f32 - tilt * (39 - y) as f32 / 22.0;
            if xx < 15.0 || xx > 33.0 {
                continue;
            }
            img.set(x, y, lit(x as f32, 24.0 + tilt, 12.0));
        }
    }
    // The jet pack and its nozzles.
    for y in 19..35 {
        for x in 18..30 {
            img.set((x as f32 + tilt * 0.6) as usize, y, if y == 19 || x == 18 || x == 29 { 1 } else { 3 + ((x + y) % 3 == 0) as u8 });
        }
    }
    let lit_jets = !flung && !kneel;
    for (nx, len) in [(20.0, if !lit_jets { 0 } else if flame { 9 } else { 6 }), (26.0, if !lit_jets { 0 } else if flame { 6 } else { 9 })] {
        for y in 35..37 {
            for x in 0..3 {
                img.set((nx + x as f32 + tilt * 0.6) as usize, y, 2);
            }
        }
        for y in 37..37 + len {
            let w = ((37 + len - y) as f32 / 3.0).ceil() as i32;
            for x in -w / 2..=w / 2 {
                let core = x.abs() < w / 3 || y < 40;
                img.set((nx + 1.0 + x as f32 + tilt * 0.6) as usize, y as usize, if core { 15 } else { 14 });
            }
        }
    }
    // Helmet, with a light catching its crown.
    for y in 2..18 {
        for x in 16..32 {
            let (dx, dy) = ((x as f32 + 0.5 - 24.0 - tilt) / 7.5, (y as f32 + 0.5 - 10.0) / 8.0);
            if dx * dx + dy * dy <= 1.0 {
                let lum = 0.55 - 0.4 * dx - 0.35 * dy;
                img.set(x, y, if dx * dx + dy * dy < 0.08 && dx < 0.0 && dy < 0.0 { 13 } else { shade_index(lum) });
            }
        }
    }
    img
}

/// An image turned a quarter: counter-clockwise (its top to the left), or
/// clockwise. The boards could not turn sprites; games drew such frames.
fn turned(img: &Image, clockwise: bool) -> Image {
    let mut out = Image::new(img.height, img.width);
    for ny in 0..out.height {
        for nx in 0..out.width {
            let (x, y) = if clockwise { (ny, img.height - 1 - nx) } else { (img.width - 1 - ny, nx) };
            out.set(nx, ny, img.get(x, y));
        }
    }
    out
}

fn shadow() -> Image {
    let mut img = Image::new(64, 16);
    for y in 0..16 {
        for x in 0..64 {
            let (dx, dy) = ((x as f32 + 0.5 - 32.0) / 32.0, (y as f32 + 0.5 - 8.0) / 8.0);
            if dx * dx + dy * dy <= 1.0 {
                img.set(x, y, 1);
            }
        }
    }
    img
}

/// A ringed planet, 96×56: banded, lit from the left, its ring passing
/// behind and in front.
fn planet() -> Image {
    let mut img = Image::new(96, 56);
    let ring = |x: f32, y: f32| {
        let (dx, dy) = ((x - 48.0) / 46.0, (y - 30.0 + (x - 48.0) * 0.12) / 9.0);
        let r = dx * dx + dy * dy;
        (0.62..1.0).contains(&r)
    };
    for y in 0..56 {
        for x in 0..96 {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            if ring(fx, fy) && fy < 30.0 - (fx - 48.0) * 0.12 {
                img.set(x, y, 15);
            }
        }
    }
    for y in 0..56 {
        for x in 0..96 {
            let (dx, dy) = ((x as f32 + 0.5 - 48.0) / 22.0, (y as f32 + 0.5 - 28.0) / 22.0);
            let r2 = dx * dx + dy * dy;
            if r2 <= 1.0 {
                let nz = (1.0 - r2).sqrt();
                let band = ((dy * 9.0).sin() * 0.12) as f32;
                img.set(x, y, shade_index(-0.6 * dx - 0.2 * dy + 0.5 * nz + band));
            }
        }
    }
    for y in 0..56 {
        for x in 0..96 {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            if ring(fx, fy) && fy >= 30.0 - (fx - 48.0) * 0.12 {
                img.set(x, y, 14);
            }
        }
    }
    img
}

/// Mountains far off, 512×40: two ranges, the near one darker, in tiles.
fn hills(tiles: &mut Tiles) -> Layer {
    let mut img = Image::new(512, 40);
    for x in 0..512 {
        let t = x as f32 / 512.0 * std::f32::consts::TAU;
        let far = 22.0 + 7.0 * (t * 3.0).sin() + 4.0 * (t * 7.0 + 1.0).sin() + 2.0 * (t * 17.0).sin();
        let near = 30.0 + 5.0 * (t * 5.0 + 2.0).sin() + 3.0 * (t * 11.0).sin();
        for y in 0..40 {
            let yf = y as f32;
            if yf >= 40.0 - near {
                img.set(x, y, 3 + ((40.0 - yf) / near * 4.0) as u8);
            } else if yf >= 40.0 - far - 6.0 {
                img.set(x, y, 7 + ((40.0 - yf) / (far + 6.0) * 4.0) as u8);
            }
        }
    }
    let ids = tiles.cut(&img);
    let mut layer = Layer::new(64, 5);
    for (i, id) in ids.into_iter().enumerate() {
        layer.map[i] = (id, HILLS as u8);
    }
    layer
}

/// A starfield, 512×128, sparse.
fn stars(tiles: &mut Tiles) -> Layer {
    let mut img = Image::new(512, 128);
    let mut r: u32 = 7;
    for _ in 0..170 {
        r = r.wrapping_mul(1103515245).wrapping_add(12345);
        let (x, y) = ((r >> 8) as usize % 512, (r >> 20) as usize % 128);
        img.set(x, y, if r % 5 == 0 { 1 } else { 2 });
    }
    let ids = tiles.cut(&img);
    let mut layer = Layer::new(64, 16);
    for (i, id) in ids.into_iter().enumerate() {
        layer.map[i] = (id, STARS);
    }
    layer
}

struct Art {
    palettes: Vec<Palette>,
    images: Vec<Image>,
    tiles: Tiles,
    font: HashMap<char, u16>,
    gold: HashMap<char, u16>,
    hills: Layer,
    stars: Layer,
}

// Image indices.
const BALL: usize = 0;
const PILLAR_IMG: usize = 1;
const SHADOW: usize = 2;
const PLANET_IMG: usize = 3;
/// The hero: straight (two flame frames), banked right (two).
const HERO: usize = 4;
/// Hit: tumbling (turned), standing still, flat on the floor, kneeling, flung.
const TUMBLE: usize = 8;
const FLAT: usize = 10;
const KNEEL: usize = 11;
const FLUNG: usize = 12;

fn build() -> Art {
    let mut tiles = Tiles::default();
    tiles.add([[0; 8]; 8]);
    let font = font_tiles(&mut tiles, 1, 2);
    let mut gold_tiles = HashMap::new();
    for (ch, _) in nes_ppu::font::FONT {
        gold_tiles.insert(*ch, font[ch]);
    }
    let hills = hills(&mut tiles);
    let stars = stars(&mut tiles);
    let flung = hero_pose(0, false, true, false);
    let images = vec![
        ball(), pillar(), shadow(), planet(),
        hero(0, false), hero(0, true), hero(1, false), hero(1, true),
        turned(&flung, true),
        hero_pose(0, false, false, false),
        turned(&hero_pose(0, false, false, false), false),
        hero_pose(0, false, false, true),
        flung,
    ];
    Art { palettes: palettes(), images, tiles, font: font.clone(), gold: gold_tiles, hills, stars }
}

fn art() -> &'static Art {
    static ART: OnceLock<Art> = OnceLock::new();
    ART.get_or_init(build)
}

fn nums(s: &str) -> Vec<f32> {
    s.split(',').filter_map(|v| v.trim().parse().ok()).collect()
}

#[derive(Default)]
pub struct Game {
    player: Vec<f32>,
    travel: f32,
    things: Vec<Vec<f32>>,
    text: String,
}

/// Where the camera is: beside and above you, a little behind, following.
struct Camera {
    x: f32,
    height: f32,
    horizon: f32,
}

impl Camera {
    fn of(px: f32, py: f32) -> Camera {
        // It climbs with you, nearly as high, and tilts: flying high, the
        // horizon sinks down the screen, so you stay in the picture.
        Camera { x: px * 0.7, height: 1.2 + py * 0.85, horizon: 96.0 + (py - 2.5) * 8.0 }
    }
    /// A world point on screen, and its scale (pixels a world unit).
    fn project(&self, x: f32, y: f32, z: f32) -> Option<(f32, f32, f32)> {
        let d = z + BEHIND;
        if d < 0.6 {
            return None;
        }
        let s = FOCAL / d;
        Some((WIDTH as f32 / 2.0 + (x - self.x) * s, self.horizon + (self.height - y) * s, s))
    }
}

impl Game {
    fn text_layer(&self) -> Layer {
        let art = art();
        let mut layer = Layer::new(40, 28);
        for entry in self.text.split('^').filter(|s| !s.is_empty()) {
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
                let (cx, cy) = (x + col, y);
                if (0..40).contains(&cx) && (0..28).contains(&cy) && ch != ' ' {
                    let set = if gold { &art.gold } else { &art.font };
                    if let Some(&t) = set.get(&ch).or_else(|| set.get(&ch.to_ascii_uppercase())) {
                        layer.map[cy as usize * 40 + cx as usize] = (t, if gold { GOLD_TEXT } else { TEXT });
                    }
                }
                col += 1;
            }
        }
        layer
    }

    pub fn draw_frame(&self, rgba: &mut [u8]) {
        let art = art();
        let [px, py, lean, hurt, tick, pose, down] = self.player[..] else { return };
        let cam = Camera::of(px, py.max(0.4));
        // The instant of a hit: everything flashes white.
        let flash = ((down - 74.0) / 6.0).clamp(0.0, 1.0);
        // The sky: deep space overhead to a pink haze at the horizon.
        let backdrop: Vec<u16> = (0..HEIGHT)
            .map(|y| {
                let t = (y as f32 / cam.horizon.max(1.0)).clamp(0.0, 1.0);
                sys16::mix(sys16::mix(rgb8(6, 0, 30), rgb8(240, 90, 150), t * t), rgb8(255, 255, 255), flash)
            })
            .collect();
        let mut stars = art.stars.clone();
        stars.top = 0;
        stars.scroll = vec![(cam.x * 2.0) as i32];
        // Mountains: nearer lines scroll more as you move (parallax).
        let mut hills = art.hills.clone();
        hills.top = cam.horizon.round() as i32 - 39;
        hills.scroll = (0..HEIGHT).map(|line| (cam.x * (5.0 + (line as f32 - hills.top as f32).max(0.0) * 0.18)) as i32).collect();
        let ground = Ground {
            horizon: cam.horizon,
            height: cam.height,
            focal: FOCAL,
            travel: self.travel - BEHIND,
            shift: cam.x,
            check: (3.0, 3.0),
            colors: [sys16::mix(rgb8(70, 230, 160), rgb8(255, 255, 255), flash), sys16::mix(rgb8(20, 120, 110), rgb8(255, 255, 255), flash)],
            fog: rgb8(240, 90, 150),
            fog_far: 520.0,
        };
        let mut sprites = Vec::new();
        // The planet, far off: it drifts against your moves.
        sprites.push(Sprite { depth: 1e6, ..Sprite::zoom(PLANET_IMG, 236.0 - cam.x * 3.0, cam.horizon - 28.0, 1.3, PLANET, 1e6) });
        for t in &self.things {
            let [kind, x, y, z, size, col] = t[..] else { continue };
            let Some((sx, gy, s)) = cam.project(x, 0.0, z) else { continue };
            // Its shadow on the floor first.
            let w = if kind as i32 == 2 { 1.6 } else { 2.0 * size };
            let mut sh = Sprite::zoom(SHADOW, sx, gy, w * s / 64.0, 0, z + 0.02);
            sh.anchor_y = 0.5;
            sh.shadow = true;
            sprites.push(sh);
            if kind as i32 != 2 {
                let Some((bx, by, _)) = cam.project(x, y, z) else { continue };
                let mut b = Sprite::zoom(BALL, bx, by, 2.0 * size * s / 64.0, BALLS + col as usize % 4, z);
                b.anchor_y = 0.5;
                sprites.push(b);
            } else {
                let mut p = Sprite::zoom(PILLAR_IMG, sx, gy, 1.4 * s / 32.0, PILLAR, z);
                p.zoom_y = PILLAR_HEIGHT * s / 128.0;
                sprites.push(p);
            }
        }
        // You, and your shadow; flickering while hurt.
        if let Some((sx, sy, s)) = cam.project(px, py, 0.0) {
            let (gx, gy, _) = cam.project(px, 0.0, 0.0).unwrap();
            let mut sh = Sprite::zoom(SHADOW, gx, gy, 1.2 * s / 64.0, 0, 0.02);
            sh.anchor_y = 0.5;
            sh.shadow = true;
            sprites.push(sh);
            let p = pose as i32;
            if p > 0 {
                // Down: flung back, tumbling end over end, flat on the floor,
                // then up on one knee.
                let (img, flip) = match p {
                    1 => (FLUNG, false),
                    2 => (TUMBLE, (tick as i32 / 6) % 2 == 1),
                    3 => (FLAT, false),
                    _ => (KNEEL, false),
                };
                let mut h = Sprite::zoom(img, sx, sy, 1.5 * s / 64.0, YOU, 0.0);
                h.flip_x = flip;
                sprites.push(h);
            } else if hurt < 0.5 || (tick as i32) % 4 < 2 {
                let flame = (tick as i32) % 2 == 1;
                let banked = lean.abs() > 0.5;
                let img = HERO + if banked { 2 } else { 0 } + flame as usize;
                let mut h = Sprite::zoom(img, sx, sy, 1.5 * s / 64.0, YOU, 0.0);
                h.flip_x = lean < -0.5;
                sprites.push(h);
            }
        }
        let text = self.text_layer();
        let back = [stars, hills];
        let front = [text];
        let frame = render(&Scene {
            palettes: &art.palettes,
            images: &art.images,
            tiles: &art.tiles,
            backdrop: &backdrop,
            back: &back,
            ground: Some(ground),
            sprites: &sprites,
            front: &front,
            max_sprites: Some(128),
        });
        frame.to_rgba(rgba);
    }
}

impl NesGame for Game {
    fn size(&self) -> (usize, usize) {
        (WIDTH, HEIGHT)
    }

    fn bind(&mut self, inputs: &[Value]) -> Result<(), SurfaceError> {
        let [player, travel, things, words] = inputs else {
            return Err(SurfaceError(format!("s16: expected {INPUTS} inputs, got {}", inputs.len())));
        };
        self.player = nums(&text(player, "player")?);
        self.travel = text(travel, "travel")?.trim().parse().unwrap_or(0.0);
        self.things = text(things, "things")?.split(';').filter(|s| !s.is_empty()).map(nums).collect();
        self.text = text(words, "text")?;
        Ok(())
    }

    fn draw(&mut self, _frame: u64, rgba: &mut [u8]) {
        self.draw_frame(rgba);
    }
}

pub static REGISTRY: Registry = Registry {
    surfaces: &[("s16", INPUTS, nes_screen::surface::<Game>)],
    shaders: nes_screen::shaders::SHADERS,
};

exact_gpu::module!(REGISTRY);

#[cfg(test)]
mod tests {
    use super::*;

    /// A sample frame to assets/ (`cargo test -p exact-space-dodgeball-gpu frame -- --ignored`).
    #[test]
    #[ignore]
    fn frame() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets");
        std::fs::create_dir_all(&root).unwrap();
        let g = Game {
            player: vec![1.0, 2.5, 1.0, 0.0, 3.0, 0.0, 0.0],
            travel: 37.0,
            things: vec![
                vec![1.0, 0.5, 2.6, 6.0, 1.3, 0.0],
                vec![1.0, -3.0, 1.5, 18.0, 1.0, 1.0],
                vec![2.0, 4.0, 0.0, 26.0, 0.6, 0.0],
                vec![1.0, 2.5, 3.5, 40.0, 1.2, 2.0],
                vec![2.0, -5.0, 0.0, 50.0, 0.6, 0.0],
                vec![1.0, -1.0, 2.0, 60.0, 0.9, 3.0],
            ],
            text: "1,1|SCORE 000370^16,1|TOP 001250^30,1|LIFE ***".into(),
        };
        let mut rgba = vec![0u8; WIDTH * HEIGHT * 4];
        g.draw_frame(&mut rgba);
        std::fs::write(root.join("frame.png"), nes_ppu::png::encode(WIDTH as u32, HEIGHT as u32, &rgba)).unwrap();
    }

    #[test]
    fn nearer_things_are_bigger() {
        let cam = Camera::of(0.0, 2.5);
        let (_, _, near) = cam.project(0.0, 1.0, 2.0).unwrap();
        let (_, _, far) = cam.project(0.0, 1.0, 40.0).unwrap();
        assert!(near > far * 5.0);
        assert!(cam.project(0.0, 0.0, -5.9).is_none(), "behind the camera");
    }
}
