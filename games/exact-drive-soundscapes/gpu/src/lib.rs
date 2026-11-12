//! Exact Drive Soundscapes' GPU module: its screen, drawn by engines/sys16
//! (320×224, the Mega Drive's own size) and shown by nes-screen. The music
//! is made by engines/ym2612 ahead of time: `cargo test --release -p
//! exact-drive-soundscapes-gpu render -- --ignored` renders the songs and
//! sound effects (music.rs) to WAV and writes songs.contract, the rows the
//! screen follows.
//!
//! Inputs: 1. the six channels' levels `l0,…,l5` (0–15); 2. the watched
//! channel's patch `algorithm,feedback,level`; 3. the clock `ms,row`; 4. text
//! `col,row|text` joined by `^` (`{…}` gold).

pub mod music;

use exact_gpu::json::text;
use exact_gpu::{Registry, SurfaceError, Value};
use nes_screen::NesGame;
use std::collections::HashMap;
use std::sync::OnceLock;
use sys16::{font_tiles, render, rgb8, Ground, Image, Layer, Palette, Scene, Sprite, Tiles, HEIGHT, WIDTH};

pub const INPUTS: usize = 4;
const HORIZON: f32 = 148.0;

const TEXT: u8 = 0;
const GOLD_TEXT: u8 = 1;
const SUN: usize = 2;
const METER: usize = 3;
/// The diagram: dim, lit and bright.
const DIAGRAM: usize = 4;

fn palettes() -> Vec<Palette> {
    let mut text = [0u16; 16];
    text[1] = rgb8(255, 255, 255);
    text[2] = rgb8(40, 10, 70);
    let mut gold = [0u16; 16];
    gold[1] = rgb8(255, 210, 60);
    gold[2] = rgb8(90, 20, 40);
    // The sun: bands from yellow at the top to hot pink.
    let mut sun = [0u16; 16];
    for i in 1..=12 {
        let t = (i - 1) as f32 / 11.0;
        sun[i] = sys16::mix(rgb8(255, 236, 90), rgb8(255, 40, 140), t);
    }
    // The meters: green, then yellow, then red at the top.
    let mut meter = [0u16; 16];
    for i in 1..=12 {
        let t = (i - 1) as f32 / 11.0;
        meter[i] = if t < 0.5 { sys16::mix(rgb8(40, 230, 120), rgb8(250, 230, 60), t * 2.0) } else { sys16::mix(rgb8(250, 230, 60), rgb8(255, 50, 60), (t - 0.5) * 2.0) };
    }
    meter[13] = rgb8(255, 255, 255);
    // Diagram: 1 lines, 2 box, 3 border, 4 digits, 5 carrier box.
    let diagram = |line: (u8, u8, u8), boxc: (u8, u8, u8), carrier: (u8, u8, u8)| {
        let mut p = [0u16; 16];
        p[1] = rgb8(line.0, line.1, line.2);
        p[2] = rgb8(boxc.0, boxc.1, boxc.2);
        p[3] = rgb8(255, 255, 255);
        p[4] = rgb8(20, 10, 40);
        p[5] = rgb8(carrier.0, carrier.1, carrier.2);
        p
    };
    vec![
        text,
        gold,
        sun,
        meter,
        diagram((120, 90, 170), (110, 120, 170), (150, 110, 190)),
        diagram((200, 160, 255), (140, 170, 230), (255, 130, 200)),
        diagram((255, 230, 255), (190, 220, 255), (255, 190, 90)),
    ]
}

/// Where each algorithm puts operators 1–4 (column, row) and which feed which.
const LAYOUT: [([(i32, i32); 4], &[(usize, usize)]); 8] = [
    ([(0, 1), (1, 1), (2, 1), (3, 1)], &[(0, 1), (1, 2), (2, 3)]),
    ([(0, 0), (0, 2), (1, 1), (2, 1)], &[(0, 2), (1, 2), (2, 3)]),
    ([(1, 0), (0, 2), (1, 2), (2, 1)], &[(1, 2), (0, 3), (2, 3)]),
    ([(0, 0), (1, 0), (1, 2), (2, 1)], &[(0, 1), (1, 3), (2, 3)]),
    ([(0, 0), (1, 0), (0, 2), (1, 2)], &[(0, 1), (2, 3)]),
    ([(0, 1), (1, 0), (1, 1), (1, 2)], &[(0, 1), (0, 2), (0, 3)]),
    ([(0, 0), (1, 0), (1, 1), (1, 2)], &[(0, 1)]),
    ([(0, 0), (1, 1), (2, 2), (3, 3)], &[]),
];
const BOX_W: i32 = 18;
const BOX_H: i32 = 12;
const COL: i32 = 30;
const ROW: i32 = 18;
const DIAGRAM_W: usize = 140;
const DIAGRAM_H: usize = 80;
const BUS_X: i32 = 128;

fn line(img: &mut Image, (x0, y0): (i32, i32), (x1, y1): (i32, i32), c: u8) {
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
    let (mut x, mut y, mut err) = (x0, y0, dx + dy);
    loop {
        if x >= 0 && y >= 0 && (x as usize) < img.width && (y as usize) < img.height {
            img.set(x as usize, y as usize, c);
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

fn glyph(img: &mut Image, ch: char, x: i32, y: i32, c: u8) {
    if let Some((_, rows)) = nes_ppu::font::FONT.iter().find(|(k, _)| *k == ch) {
        for (j, row) in rows.iter().enumerate() {
            for (i, p) in row.chars().enumerate() {
                if p == '#' {
                    img.set((x + i as i32) as usize, (y + j as i32) as usize, c);
                }
            }
        }
    }
}

/// An algorithm's diagram: four boxes, the lines between them, operator 1's
/// feedback loop, and the carriers' lines to the output bus.
fn diagram(alg: usize) -> Image {
    let mut img = Image::new(DIAGRAM_W, DIAGRAM_H);
    let (places, edges) = LAYOUT[alg];
    let carrier = ym2612::driver::carriers(alg as u8);
    let at = |i: usize| (6 + places[i].0 * COL, 6 + places[i].1 * ROW);
    for &(a, b) in edges {
        let (ax, ay) = at(a);
        let (bx, by) = at(b);
        line(&mut img, (ax + BOX_W, ay + BOX_H / 2), (bx - 1, by + BOX_H / 2), 1);
    }
    let carrier_ys: Vec<i32> = (0..4).filter(|&i| carrier[i]).map(|i| at(i).1 + BOX_H / 2).collect();
    for i in (0..4).filter(|&i| carrier[i]) {
        let (x, y) = at(i);
        line(&mut img, (x + BOX_W, y + BOX_H / 2), (BUS_X, y + BOX_H / 2), 1);
    }
    let (top, bottom) = (*carrier_ys.iter().min().unwrap(), *carrier_ys.iter().max().unwrap());
    line(&mut img, (BUS_X, top), (BUS_X, bottom), 1);
    let mid = (top + bottom) / 2;
    line(&mut img, (BUS_X, mid), (DIAGRAM_W as i32 - 2, mid), 1);
    line(&mut img, (DIAGRAM_W as i32 - 5, mid - 3), (DIAGRAM_W as i32 - 2, mid), 1);
    line(&mut img, (DIAGRAM_W as i32 - 5, mid + 3), (DIAGRAM_W as i32 - 2, mid), 1);
    // Feedback: a loop from operator 1's output back over its top.
    let (x, y) = at(0);
    let path = [(x + BOX_W, y + 3), (x + BOX_W + 3, y + 3), (x + BOX_W + 3, y - 4), (x - 3, y - 4), (x - 3, y + 6), (x - 1, y + 6)];
    for w in path.windows(2) {
        line(&mut img, w[0], w[1], 1);
    }
    for i in 0..4 {
        let (x, y) = at(i);
        for j in 0..BOX_H {
            for k in 0..BOX_W {
                let edge = j == 0 || j == BOX_H - 1 || k == 0 || k == BOX_W - 1;
                img.set((x + k) as usize, (y + j) as usize, if edge { 3 } else if carrier[i] { 5 } else { 2 });
            }
        }
        glyph(&mut img, char::from(b'1' + i as u8), x + 6, y + 3, 4);
    }
    img
}

/// The sun: a disc, banded, with gaps growing toward its base.
fn sun() -> Image {
    let (w, h) = (96usize, 48usize);
    let mut img = Image::new(w, h);
    let r = 48.0f32;
    for y in 0..h {
        let dy = r - y as f32 - 0.5;
        let half = (r * r - dy * dy).max(0.0).sqrt();
        // Gaps: from a third of the way down, thicker toward the bottom.
        let band = y as f32 / h as f32;
        let gap = band > 0.4 && ((y as f32 * 0.9) as usize % (3 + (6.0 * (1.0 - band)) as usize) == 0);
        if gap {
            continue;
        }
        let colour = 1 + ((band * 11.0) as u8).min(11);
        for x in 0..w {
            if (x as f32 + 0.5 - r).abs() < half {
                img.set(x, y, colour);
            }
        }
    }
    img
}

/// A meter bar: its colour climbs from green to red, banded in segments.
fn meter() -> Image {
    let (w, h) = (20usize, 60usize);
    let mut img = Image::new(w, h);
    for y in 0..h {
        if y % 4 == 3 {
            continue;
        }
        let c = 12 - ((y * 12 / h) as u8).min(11);
        for x in 0..w {
            img.set(x, y, c);
        }
    }
    img
}

struct Art {
    palettes: Vec<Palette>,
    images: Vec<Image>,
    tiles: Tiles,
    font: HashMap<char, u16>,
}

const SUN_IMG: usize = 8;
const METER_IMG: usize = 9;

fn build() -> Art {
    let mut tiles = Tiles::default();
    tiles.add([[0; 8]; 8]);
    let font = font_tiles(&mut tiles, 1, 2);
    let mut images: Vec<Image> = (0..8).map(diagram).collect();
    images.push(sun());
    images.push(meter());
    Art { palettes: palettes(), images, tiles, font }
}

fn art() -> &'static Art {
    static ART: OnceLock<Art> = OnceLock::new();
    ART.get_or_init(build)
}

fn nums(s: &str) -> Vec<f32> {
    s.split(',').filter_map(|v| v.trim().parse().ok()).collect()
}

#[derive(Default)]
pub struct Screen {
    levels: Vec<f32>,
    patch: Vec<f32>,
    clock: Vec<f32>,
    text: String,
}

impl Screen {
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
                    if let Some(&t) = art.font.get(&ch).or_else(|| art.font.get(&ch.to_ascii_uppercase())) {
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
        let ms = self.clock.first().copied().unwrap_or(0.0);
        let row = self.clock.get(1).copied().unwrap_or(0.0) as i32;
        // A dusk sky: deep violet overhead to orange at the horizon.
        let backdrop: Vec<u16> = (0..HEIGHT)
            .map(|y| {
                let t = (y as f32 / HORIZON).clamp(0.0, 1.0);
                if t < 0.6 { sys16::mix(rgb8(16, 4, 44), rgb8(120, 20, 110), t / 0.6) } else { sys16::mix(rgb8(120, 20, 110), rgb8(255, 120, 70), (t - 0.6) / 0.4) }
            })
            .collect();
        let ground = Ground {
            horizon: HORIZON,
            height: 1.0,
            focal: 220.0,
            travel: ms / 1000.0 * 9.0,
            shift: (ms / 2400.0).sin() * 0.6,
            check: (1.0, 1.0),
            colors: [rgb8(70, 10, 90), rgb8(30, 4, 50)],
            fog: rgb8(255, 90, 120),
            fog_far: 40.0,
        };
        let mut sprites = Vec::new();
        // The sun on the horizon; it swells a little on the beat.
        let beat = if row % 4 == 0 { 1.04 } else { 1.0 };
        sprites.push(Sprite { depth: 1e6, ..Sprite::zoom(SUN_IMG, 160.0, HORIZON, beat, SUN, 1e6) });
        // The patch's algorithm, lit as its channel sounds.
        if let [alg, _fb, level] = self.patch[..] {
            let pal = DIAGRAM + if level >= 10.0 { 2 } else if level >= 1.0 { 1 } else { 0 };
            sprites.push(Sprite { anchor_x: 0.0, anchor_y: 0.0, ..Sprite::zoom((alg as usize).min(7), 172.0, 56.0, 1.0, pal, 0.0) });
        }
        // Six meters, standing on the floor.
        for (i, &l) in self.levels.iter().take(6).enumerate() {
            if l <= 0.0 {
                continue;
            }
            let mut m = Sprite::zoom(METER_IMG, 40.0 + i as f32 * 48.0, 204.0, 1.0, METER, 1.0);
            m.zoom_y = (l / 15.0).clamp(0.0, 1.0);
            sprites.push(m);
        }
        let text = self.text_layer();
        let front = [text];
        let frame = render(&Scene {
            palettes: &art.palettes,
            images: &art.images,
            tiles: &art.tiles,
            backdrop: &backdrop,
            back: &[],
            ground: Some(ground),
            sprites: &sprites,
            front: &front,
            max_sprites: Some(80),
        });
        frame.to_rgba(rgba);
    }
}

impl NesGame for Screen {
    fn size(&self) -> (usize, usize) {
        (WIDTH, HEIGHT)
    }

    fn bind(&mut self, inputs: &[Value]) -> Result<(), SurfaceError> {
        let [levels, patch, clock, words] = inputs else {
            return Err(SurfaceError(format!("s16: expected {INPUTS} inputs, got {}", inputs.len())));
        };
        self.levels = nums(&text(levels, "levels")?);
        self.patch = nums(&text(patch, "patch")?);
        self.clock = nums(&text(clock, "clock")?);
        self.text = text(words, "text")?;
        Ok(())
    }

    fn draw(&mut self, _frame: u64, rgba: &mut [u8]) {
        self.draw_frame(rgba);
    }
}

pub static REGISTRY: Registry = Registry {
    surfaces: &[("s16", INPUTS, nes_screen::surface::<Screen>)],
    shaders: nes_screen::shaders::SHADERS,
};

exact_gpu::module!(REGISTRY);

/// A name's file name: lower case, words joined by dashes.
pub fn slug(name: &str) -> String {
    name.to_lowercase().split_whitespace().collect::<Vec<_>>().join("-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ym2612::driver::{render as render_song, render_writes, tokens};
    use ym2612::wav;

    const RATE: u32 = 44100;

    fn peak(s: &[f32]) -> f32 {
        s.iter().fold(0.0f32, |a, v| a.max(v.abs()))
    }

    /// Renders the songs (two passes each, for a seamless loop) and the
    /// sound effects to assets/, each scaled to the same peak, and writes
    /// songs.contract.
    #[test]
    #[ignore]
    fn render() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        std::fs::create_dir_all(root.join("assets/music")).unwrap();
        std::fs::create_dir_all(root.join("assets/sfx")).unwrap();
        let mut c = String::from(
            "// Written by the exact-drive-soundscapes-gpu `render` test from gpu/src/music.rs:\n\
             // do not edit. Each song's rows, per channel: what sounds (a note, a drum's\n\
             // letter, or \"\"), its level (0-15) and its patch (an index into the patches).\n\n",
        );
        let pick = |xs: &[String]| -> String {
            let mut s = xs.last().unwrap().clone();
            for i in (0..xs.len() - 1).rev() {
                s = format!("(i == {i} ? {} : {s})", xs[i]);
            }
            s
        };
        let list = |xs: Vec<String>| format!("[{}]", xs.join(", "));
        let (mut names, mut files, mut row_ms, mut counts) = (vec![], vec![], vec![], vec![]);
        let mut fns = String::new();
        for (i, song) in music::SONGS.iter().enumerate() {
            let rows = song.rows.iter().map(|r| tokens(r).len()).fold(1, usize::max);
            let r = render_song(song, rows * 2, RATE);
            let gain = 0.89 / peak(&r.samples).max(1e-6);
            let file = format!("assets/music/{}.wav", slug(song.name));
            std::fs::write(root.join(&file), wav::encode_stereo(&r.samples, RATE, gain)).unwrap();
            names.push(format!("\"{}\"", song.name));
            files.push(format!("\"{file}\""));
            row_ms.push(format!("{:.3}", r.row_ms));
            counts.push(rows.to_string());
            for ch in 0..6 {
                let rs = &r.rows[..rows];
                fns += &format!("fn labels{i}{ch}(): string[] = {}\n", list(rs.iter().map(|cs| format!("\"{}\"", cs[ch].label)).collect()));
                fns += &format!("fn volumes{i}{ch}(): number[] = {}\n", list(rs.iter().map(|cs| cs[ch].volume.to_string()).collect()));
                let patches = rs.iter().map(|cs| music::patch_index(&song.instruments[cs[ch].instrument.min(song.instruments.len() - 1)].patch).to_string()).collect();
                fns += &format!("fn patches{i}{ch}(): number[] = {}\n", list(patches));
            }
        }
        c += &format!("fn songCount(): number = {}\n", music::SONGS.len());
        c += &format!("fn songName(i: number): string = {}\n", pick(&names));
        c += &format!("fn songFile(i: number): string = {}\n", pick(&files));
        c += &format!("fn rowMs(i: number): number = {}\n", pick(&row_ms));
        c += &format!("fn rowCount(i: number): number = {}\n", pick(&counts));
        c += &fns;
        for ch in 0..6 {
            for what in ["labels", "volumes", "patches"] {
                let xs: Vec<String> = (0..music::SONGS.len()).map(|i| format!("{what}{i}{ch}()")).collect();
                let ty = if what == "labels" { "string[]" } else { "number[]" };
                c += &format!("fn {what}{ch}(i: number): {ty} = {}\n", pick(&xs));
            }
        }
        let pnames: Vec<String> = music::PATCHES.iter().map(|p| format!("\"{}\"", p.name)).collect();
        let palgs: Vec<String> = music::PATCHES.iter().map(|p| p.algorithm.to_string()).collect();
        let pfbs: Vec<String> = music::PATCHES.iter().map(|p| p.feedback.to_string()).collect();
        c += &format!("fn patchName(i: number): string = {}\n", pick(&pnames));
        c += &format!("fn patchAlgorithm(i: number): number = {}\n", pick(&palgs));
        c += &format!("fn patchFeedback(i: number): number = {}\n", pick(&pfbs));
        let fx = music::effects();
        for e in &fx {
            let samples = render_writes(&e.script.writes, e.frames, &e.dac, RATE);
            let gain = 0.89 / peak(&samples).max(1e-6);
            std::fs::write(root.join(format!("assets/sfx/{}.wav", e.file)), wav::encode_stereo(&samples, RATE, gain)).unwrap();
        }
        c += &format!("fn soundCount(): number = {}\n", fx.len());
        c += &format!("fn soundName(i: number): string = {}\n", pick(&fx.iter().map(|e| format!("\"{}\"", e.name)).collect::<Vec<_>>()));
        c += &format!("fn soundFile(i: number): string = {}\n", pick(&fx.iter().map(|e| format!("\"assets/sfx/{}.wav\"", e.file)).collect::<Vec<_>>()));
        std::fs::write(root.join("songs.contract"), c).unwrap();
    }

    #[test]
    fn songs_render_and_have_sound() {
        for song in music::SONGS.iter() {
            let r = render_song(song, 16, 22050);
            let p = peak(&r.samples);
            assert!(p > 0.1, "{} peak {p}", song.name);
            for ch in 0..6 {
                assert!(r.rows.iter().any(|c| c[ch].volume > 0), "{} channel {ch} is silent", song.name);
            }
        }
        for e in music::effects() {
            let s = render_writes(&e.script.writes, e.frames, &e.dac, 22050);
            assert!(peak(&s) > 0.02, "{} is silent", e.name);
        }
    }

    #[test]
    fn songs_bars_are_whole() {
        for song in music::SONGS.iter() {
            for (ch, text) in song.rows.iter().enumerate() {
                for (b, bar) in text.split('|').enumerate() {
                    assert_eq!(bar.split_whitespace().count(), 16, "{} channel {} bar {}", song.name, ch + 1, b + 1);
                }
            }
        }
    }

    /// A sample frame to assets/frame.png.
    #[test]
    #[ignore]
    fn frame() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets");
        std::fs::create_dir_all(&root).unwrap();
        let s = Screen {
            levels: vec![14.0, 9.0, 6.0, 15.0, 5.0, 12.0],
            patch: vec![4.0, 6.0, 12.0],
            clock: vec![3200.0, 8.0],
            text: "8,1|{EXACT DRIVE SOUNDSCAPES}^2,4|{SONGS}^3,5|NEON HIGHWAY".into(),
        };
        let mut rgba = vec![0u8; WIDTH * HEIGHT * 4];
        s.draw_frame(&mut rgba);
        std::fs::write(root.join("frame.png"), nes_ppu::png::encode(WIDTH as u32, HEIGHT as u32, &rgba)).unwrap();
    }
}
