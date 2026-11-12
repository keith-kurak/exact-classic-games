//! Exact Armadillo Run's GPU module: the `s16` surface, drawn by
//! engines/sys16 at 320×224 and shown by nes-screen.
//!
//! The camera rides behind the armadillo down a half-pipe (sys16's tube
//! layer). Everything on the pipe is a sprite zoomed to its true size by the
//! scaler, every frame, from art made once at start-up (`art`).
//!
//! Inputs: 1. the armadillo `angle,lift,hurt,tick,ball,camera roll,scheme`
//! (the camera's roll is 0 when it is fixed; the scheme is the track's
//! colours: 0 dusk, 1 turquoise, 2 midnight); 2. the run
//! `travel,speed,pan`; 3. the course ahead: the pipe's curve and climb at
//! each `BEND_STEP` from the camera, `c0,c1,…|r0,r1,…`; 4. things on it,
//! `kind,angle,z;…` (1 a beetle, 2 a thorn ball, 3 a checkpoint, 4 the start
//! line, 5 a beetle just taken, z ahead of the armadillo; and the gem at
//! the end, `6,outcome,z,ticks`: outcome 0 still coming, 1 taken, 2 not); 5. text, `col,row|text` joined by `^`
//! (`{…}` gold, `[…]` red; `<` and `>` are drawn).

pub mod art;
pub mod music;

use art::{Frame as Art, View, GEM_FRAMES, P_GEM, P_SCHEMES, P_BEETLE, P_DILLO, P_GATE, P_GOLD, P_RED, P_START, P_TEXT, P_THORN};
use exact_gpu::json::text;
use exact_gpu::{Registry, SurfaceError, Value};
use nes_screen::NesGame;
use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::OnceLock;
use sys16::{font_tiles, render, rgb8, Image, Layer, Palette, Scene, Sprite, Tiles, Tube, TubeMark, HEIGHT, WIDTH};

pub const INPUTS: usize = 5;

// ── The camera and the pipe ──
pub const RADIUS: f32 = 4.0;
pub const EYE: f32 = 3.3;
pub const FOCAL: f32 = 200.0;
pub const HORIZON: f32 = 84.0;
/// How far ahead of the camera the armadillo runs.
pub const AHEAD: f32 = 7.0;
/// The course table's spacing, in world units.
pub const BEND_STEP: f32 = 8.0;
pub const SKIN_LEN: f32 = 6.0;

// ── Frames ──
/// Tilts: all the way round in 20° steps, for things on the walls and
/// overhead.
const ROLLS: usize = 18;
const ROLL_STEP: f32 = 20.0 * PI / 180.0;
const RUN_FRAMES: usize = 4;
const BALL_FRAMES: usize = 4;
const SPIN_FRAMES: usize = 8;
const THORN_FRAMES: usize = 4;
const SPARKLE_FRAMES: usize = 4;
/// Pixels a model unit in the art, and world units a model unit.
const DILLO_ART: f32 = 40.0;
const DILLO_SIZE: f32 = 1.4;
const BEETLE_ART: f32 = 26.0;
const BEETLE_SIZE: f32 = 0.8;
/// Beetles float a little above the pipe.
const BEETLE_LIFT: f32 = 0.35;
const THORN_ART: f32 = 30.0;
const THORN_SIZE: f32 = 1.7;

struct Sheet {
    palettes: Vec<Palette>,
    images: Vec<Image>,
    anchors: Vec<(f32, f32)>,
    tiles: Tiles,
    font: HashMap<char, u16>,
    mesas: Layer,
    stars: Layer,
    skin: usize,
    /// First image of each set.
    run: usize,
    ball: usize,
    beetle: usize,
    thorn: usize,
    shadow: usize,
    sparkle: usize,
    gem: usize,
}

impl Sheet {
    fn push(images: &mut Vec<Image>, anchors: &mut Vec<(f32, f32)>, f: Art) -> usize {
        images.push(f.image);
        anchors.push(f.anchor);
        images.len() - 1
    }
}

fn roll_of(i: usize) -> f32 {
    (i as f32 - (ROLLS / 2) as f32) * ROLL_STEP
}

/// The tilt frame for a thing `angle` round the pipe from what the camera
/// sees as straight down: its up points at the pipe's axis, so on the right
/// wall it leans left, and overhead it hangs upside down.
fn roll_index(angle: f32) -> usize {
    ((-sys16::wrap(angle) / ROLL_STEP).round() as i32 + (ROLLS / 2) as i32).rem_euclid(ROLLS as i32) as usize
}

/// Whether there is pipe under a thing to cast a shadow on (not the open
/// frame overhead).
fn on_floor(angle: f32) -> bool {
    sys16::wrap(angle).abs() < PI / 2.0 + 0.1
}

fn build() -> Sheet {
    let (mut images, mut anchors) = (Vec::new(), Vec::new());
    let mut tiles = Tiles::default();
    tiles.add([[0; 8]; 8]);
    let font = font_tiles(&mut tiles, 1, 2);
    let mesas = art::mesas(&mut tiles);
    let stars = art::stars(&mut tiles);
    images.push(art::skin());
    anchors.push((0.0, 0.0));
    let skin = 0;
    // The armadillo, from behind and above: each stride frame at each tilt.
    let run = images.len();
    let plain = art::dillo_colour(0.0);
    for r in 0..ROLLS {
        for f in 0..RUN_FRAMES {
            let parts = art::armadillo(f as f32 / RUN_FRAMES as f32);
            let view = View { yaw: 0.0, pitch: 0.38, roll: roll_of(r), scale: DILLO_ART };
            Sheet::push(&mut images, &mut anchors, art::render(&parts, view, 1.25, 12, &plain));
        }
    }
    let ball = images.len();
    for f in 0..BALL_FRAMES {
        let spin = (f as f32 + 0.5) / BALL_FRAMES as f32 * PI * 2.0 / 2.6;
        let (parts, spin) = art::armadillo_ball(spin);
        let view = View { yaw: 0.0, pitch: 0.38, roll: 0.0, scale: DILLO_ART };
        Sheet::push(&mut images, &mut anchors, art::render(&parts, view, 1.0, 12, &art::dillo_colour(spin)));
    }
    // The beetle, spinning: eight turns at each tilt.
    let beetle = images.len();
    let parts = art::beetle();
    for r in 0..ROLLS {
        for f in 0..SPIN_FRAMES {
            let view = View { yaw: f as f32 / SPIN_FRAMES as f32 * PI * 2.0, pitch: 0.42, roll: roll_of(r), scale: BEETLE_ART };
            Sheet::push(&mut images, &mut anchors, art::render(&parts, view, 1.0, 12, &art::beetle_colour));
        }
    }
    let thorn = images.len();
    let parts = art::thorn_ball();
    for f in 0..THORN_FRAMES {
        let view = View { yaw: f as f32 / THORN_FRAMES as f32 * PI * 2.0 / 5.0, pitch: 0.3, roll: 0.0, scale: THORN_ART };
        Sheet::push(&mut images, &mut anchors, art::render(&parts, view, 1.0, 12, &art::thorn_colour));
    }
    let shadow = images.len();
    for r in 0..ROLLS {
        Sheet::push(&mut images, &mut anchors, art::shadow(roll_of(r)));
    }
    let sparkle = images.len();
    for f in 0..SPARKLE_FRAMES {
        Sheet::push(&mut images, &mut anchors, art::sparkle(f));
    }
    let gem = images.len();
    for f in 0..GEM_FRAMES {
        Sheet::push(&mut images, &mut anchors, art::gem(f));
    }
    Sheet { palettes: art::palettes(), images, anchors, tiles, font, mesas, stars, skin, run, ball, beetle, thorn, shadow, sparkle, gem }
}

fn sheet() -> &'static Sheet {
    static SHEET: OnceLock<Sheet> = OnceLock::new();
    SHEET.get_or_init(build)
}

fn nums(s: &str) -> Vec<f32> {
    s.split(',').filter_map(|v| v.trim().parse().ok()).collect()
}

/// The pipe's axis at each step ahead, from its curve and climb there: the
/// same sums the road CPU did, run every frame. It starts at the camera,
/// level and straight ahead.
pub fn bend_table(curves: &[f32], rises: &[f32]) -> Vec<(f32, f32)> {
    let mut out = vec![(0.0, 0.0)];
    let (mut x, mut y, mut sx, mut sy) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for i in 0..curves.len().max(rises.len()) {
        let c = curves.get(i).copied().unwrap_or(0.0);
        let r = rises.get(i).copied().unwrap_or(0.0);
        x += sx * BEND_STEP + 0.5 * c * BEND_STEP * BEND_STEP;
        y += sy * BEND_STEP + 0.5 * r * BEND_STEP * BEND_STEP;
        sx += c * BEND_STEP;
        sy += r * BEND_STEP;
        out.push((x, y));
    }
    out
}

#[derive(Default)]
pub struct Game {
    player: Vec<f32>,
    run: Vec<f32>,
    curves: Vec<f32>,
    rises: Vec<f32>,
    things: Vec<Vec<f32>>,
    text: String,
    bound_at: u64,
    frame: u64,
}

impl Game {
    fn text_layer(&self) -> Layer {
        let sh = sheet();
        let mut layer = Layer::new(40, 28);
        for entry in self.text.split('^').filter(|s| !s.is_empty()) {
            let mut parts = entry.splitn(2, '|');
            let at: Vec<i32> = parts.next().unwrap_or("").split(',').filter_map(|v| v.trim().parse().ok()).collect();
            let [x, y] = at[..] else { continue };
            let mut col = 0;
            let mut pal = P_TEXT;
            for ch in parts.next().unwrap_or("").chars() {
                match ch {
                    '{' => pal = P_GOLD,
                    '[' => pal = P_RED,
                    '}' | ']' => pal = P_TEXT,
                    _ => {
                        let (cx, cy) = (x + col, y);
                        if (0..40).contains(&cx) && (0..28).contains(&cy) && ch != ' ' {
                            if let Some(&t) = sh.font.get(&ch).or_else(|| sh.font.get(&ch.to_ascii_uppercase())) {
                                layer.map[cy as usize * 40 + cx as usize] = (t, pal);
                            }
                        }
                        col += 1;
                    }
                }
            }
        }
        layer
    }

    fn sprite(&self, image: usize, at: (f32, f32), zoom: f32, palette: usize, depth: f32) -> Sprite {
        let (ax, ay) = sheet().anchors[image];
        Sprite { image, x: at.0, y: at.1, anchor_x: ax, anchor_y: ay, zoom_x: zoom, zoom_y: zoom, palette, flip_x: false, shadow: false, depth }
    }

    pub fn draw_frame(&self, rgba: &mut [u8]) {
        let sh = sheet();
        let [angle, lift, hurt, tick, ball, cam, scheme] = self.player[..] else { return };
        let [travel, speed, pan] = self.run[..] else { return };
        // The contract moves on 30 times a second; the picture 60. Between
        // its ticks, run on at the same speed.
        let ahead = speed * ((self.frame.saturating_sub(self.bound_at)) as f32 / 2.0).min(1.0);
        let travel = travel + ahead;
        let bend = bend_table(&self.curves, &self.rises);
        let marks: Vec<TubeMark> = self
            .things
            .iter()
            .filter(|t| t.len() == 3 && (t[0] as i32 == 3 || t[0] as i32 == 4))
            .map(|t| TubeMark { at: travel - AHEAD + t[2] + AHEAD - ahead, len: 2.0, palette: if t[0] as i32 == 3 { P_GATE } else { P_START } })
            .collect();
        let tube = Tube {
            horizon: HORIZON,
            center: WIDTH as f32 / 2.0,
            focal: FOCAL,
            radius: RADIUS,
            eye: EYE,
            rim: PI,
            travel: travel - AHEAD,
            bend: &bend,
            bend_step: BEND_STEP,
            skin: sh.skin,
            skin_round: 2.0 * PI,
            skin_len: SKIN_LEN,
            palette: P_SCHEMES[(scheme as usize).min(P_SCHEMES.len() - 1)],
            marks: &marks,
            roll: cam,
            fog: rgb8(250, 150, 110),
            fog_near: 30.0,
            fog_far: 150.0,
        };
        // Dusk: indigo overhead to a hot orange at the horizon.
        let backdrop: Vec<u16> = (0..HEIGHT)
            .map(|y| {
                let t = (y as f32 / HORIZON).clamp(0.0, 1.0);
                let top = sys16::mix(rgb8(16, 8, 54), rgb8(150, 50, 120), t * t);
                sys16::mix(top, rgb8(250, 150, 110), ((t - 0.75) * 4.0).clamp(0.0, 1.0))
            })
            .collect();
        let mut stars = sh.stars.clone();
        stars.scroll = vec![(pan * 0.5) as i32];
        let mut mesas = sh.mesas.clone();
        mesas.top = HORIZON as i32 - 30;
        mesas.scroll = vec![pan as i32];
        let frame_tick = tick as u64 * 2 + self.frame.saturating_sub(self.bound_at).min(1);
        let mut sprites = Vec::new();
        for t in &self.things {
            if let [6.0, outcome, z, t] = t[..] {
                self.draw_gem(&tube, &mut sprites, outcome as i32, z + AHEAD - ahead, t, (angle, lift), frame_tick);
                continue;
            }
            let [kind, ang, z] = t[..] else { continue };
            let kind = kind as i32;
            let rel = z - ahead;
            let z = z + AHEAD - ahead;
            if kind == 5 {
                // A beetle just taken: a sparkle where it was, over the
                // armadillo, gone in a few frames.
                let age = (-rel / speed.max(0.1)).max(0.0) as usize;
                if age < SPARKLE_FRAMES * 2 {
                    if let Some((sx, sy, s)) = tube.project(ang, BEETLE_LIFT + 0.4, z.max(AHEAD)) {
                        sprites.push(self.sprite(sh.sparkle + age / 2, (sx, sy), s / 28.0, P_BEETLE, AHEAD - 1.0));
                    }
                }
                continue;
            }
            if kind > 2 {
                continue;
            }
            let up = if kind == 1 { BEETLE_LIFT } else { 0.0 };
            let Some((sx, sy, s)) = tube.project(ang, up, z) else { continue };
            let Some((gx, gy, _)) = tube.project(ang, 0.0, z) else { continue };
            if tube.hides(gx, gy, z) {
                continue;
            }
            let r = roll_index(ang - cam);
            if on_floor(ang) {
                let mut shadow = self.sprite(sh.shadow + r, (gx, gy), s * if kind == 2 { 1.2 } else { 0.5 } / 48.0, 0, z + 0.01);
                shadow.shadow = true;
                sprites.push(shadow);
            }
            if kind == 1 {
                // Beetles spin, each a little out of step with the next (by
                // where it sits on the course, so the step never changes).
                let along = (travel - ahead + rel + 1000.0) as usize;
                let spin = ((frame_tick / 3) as usize + along / 5) % SPIN_FRAMES;
                // Reaching the armadillo, a beetle stays behind it (it is
                // being taken), rather than trading places with it frame by
                // frame; once well past, it is nearer and draws in front.
                let depth = if rel > -0.6 { z.max(AHEAD + 0.05) } else { z };
                sprites.push(self.sprite(sh.beetle + r * SPIN_FRAMES + spin, (sx, sy), s * BEETLE_SIZE / BEETLE_ART, P_BEETLE, depth));
            } else {
                let spin = (frame_tick / 4) as usize % THORN_FRAMES;
                sprites.push(self.sprite(sh.thorn + spin, (sx, sy), s * THORN_SIZE / THORN_ART, P_THORN, z));
            }
        }
        // The armadillo: running, or rolled up in the air; flickering while
        // it gets over a thorn.
        if let (Some((sx, sy, s)), Some((gx, gy, gs))) = (tube.project(angle, lift, AHEAD), tube.project(angle, 0.0, AHEAD)) {
            let r = roll_index(angle - cam);
            if on_floor(angle) {
                let mut shadow = self.sprite(sh.shadow + r, (gx, gy), gs * (1.0 - lift * 0.12).max(0.5) / 48.0, 0, AHEAD + 0.01);
                shadow.shadow = true;
                sprites.push(shadow);
            }
            let zoom = s * DILLO_SIZE / DILLO_ART;
            if hurt < 0.5 || (tick as i32) % 4 < 2 {
                let img = if ball > 0.5 {
                    sh.ball + (frame_tick / 2) as usize % BALL_FRAMES
                } else {
                    sh.run + r * RUN_FRAMES + (frame_tick / 3) as usize % RUN_FRAMES
                };
                sprites.push(self.sprite(img, (sx, sy), zoom, P_DILLO, AHEAD));
            }
            // Hit: the beetles it was carrying spray out and fall away.
            if hurt > 20.0 {
                let t = 40.0 - hurt + ahead / speed.max(0.01) * 0.5;
                for k in 0..8 {
                    let a = k as f32 / 8.0 * PI * 2.0 + 0.3;
                    let (bx, by) = (sx + a.cos() * t * 5.0, sy - 30.0 - a.sin().abs() * t * 4.0 + t * t * 0.35);
                    let spin = ((frame_tick / 2) as usize + k) % SPIN_FRAMES;
                    sprites.push(self.sprite(sh.beetle + (ROLLS / 2) * SPIN_FRAMES + spin, (bx, by), zoom * 0.5, P_BEETLE, AHEAD - 0.5));
                }
            }
        }
        let text = self.text_layer();
        let back = [stars, mesas];
        let front = [text];
        let frame = render(&Scene {
            palettes: &sh.palettes,
            images: &sh.images,
            tiles: &sh.tiles,
            backdrop: &backdrop,
            back: &back,
            turn: Some((cam, WIDTH as f32 / 2.0, HORIZON)),
            ground: None,
            tube: Some(tube),
            sprites: &sprites,
            front: &front,
            max_sprites: Some(128),
        });
        frame.to_rgba(rgba);
    }
}

/// The gem: this many world units across.
const GEM_SIZE: f32 = 5.5;

impl Game {
    /// The giant gem floating in the pipe at the end. Still coming, it turns
    /// slowly on the axis. Taken, it flies into the armadillo, shrinking,
    /// and bursts into sparkles. Not taken, it rises out of the pipe and
    /// away.
    #[allow(clippy::too_many_arguments)]
    fn draw_gem(&self, tube: &Tube, sprites: &mut Vec<Sprite>, outcome: i32, z: f32, t: f32, (angle, lift): (f32, f32), frame_tick: u64) {
        let sh = sheet();
        let spin = |rate: u64| sh.gem + (frame_tick / rate) as usize % GEM_FRAMES;
        // A point `up` world units above the axis (toward the ceiling).
        let above = |up: f32, z: f32| tube.project(PI, RADIUS - up, z);
        match outcome {
            1 => {
                let f = (t / 24.0).min(1.0);
                let (Some((gx, gy, gs)), Some((ax, ay, as_))) = (above(0.0, z), tube.project(angle, lift + 0.6, AHEAD)) else { return };
                if f < 1.0 {
                    let e = f * f;
                    let (x, y) = (gx + (ax - gx) * e, gy + (ay - gy) * e);
                    let size = GEM_SIZE * (1.0 - e * 0.9);
                    let s = gs + (as_ - gs) * e;
                    sprites.push(self.sprite(spin(1), (x, y), s * size / 96.0, P_GEM, AHEAD - 1.0));
                } else if t < 48.0 {
                    // The burst: sparkles flying out from the armadillo.
                    let k = t - 24.0;
                    for i in 0..12 {
                        let a = i as f32 / 12.0 * PI * 2.0;
                        let r = k * (3.0 + (i % 3) as f32);
                        let frame = ((k / 3.0) as usize + i) % 4;
                        sprites.push(self.sprite(sh.sparkle + frame, (ax + a.cos() * r, ay - 12.0 + a.sin() * r), as_ / 20.0, if i % 2 == 0 { P_GEM } else { P_BEETLE }, AHEAD - 1.5));
                    }
                }
            }
            2 => {
                // Up and away, turning faster.
                let (up, z) = (t * t * 0.012, z + t * 0.8);
                if let Some((x, y, s)) = above(up, z) {
                    sprites.push(self.sprite(spin(2), (x, y), s * GEM_SIZE / 96.0, P_GEM, z));
                }
            }
            _ => {
                if let Some((x, y, s)) = above(0.0, z) {
                    sprites.push(self.sprite(spin(4), (x, y), s * GEM_SIZE / 96.0, P_GEM, z));
                }
            }
        }
    }
}

impl NesGame for Game {
    fn size(&self) -> (usize, usize) {
        (WIDTH, HEIGHT)
    }

    fn bind(&mut self, inputs: &[Value]) -> Result<(), SurfaceError> {
        let [player, run, course, things, words] = inputs else {
            return Err(SurfaceError(format!("s16: expected {INPUTS} inputs, got {}", inputs.len())));
        };
        self.player = nums(&text(player, "player")?);
        let run = nums(&text(run, "run")?);
        if run != self.run {
            self.bound_at = self.frame;
        }
        self.run = run;
        let course = text(course, "course")?;
        let mut halves = course.splitn(2, '|');
        self.curves = nums(halves.next().unwrap_or(""));
        self.rises = nums(halves.next().unwrap_or(""));
        self.things = text(things, "things")?.split(';').filter(|s| !s.is_empty()).map(nums).collect();
        self.text = text(words, "text")?;
        Ok(())
    }

    fn draw(&mut self, frame: u64, rgba: &mut [u8]) {
        self.frame = frame;
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

    fn sample(angle: f32, curve: f32, rise: f32, text: &str) -> Game {
        Game {
            player: vec![angle, 0.0, 0.0, 5.0, 0.0, angle * 0.7, 0.0],
            run: vec![120.0, 0.6, 40.0],
            curves: vec![curve; 14],
            rises: vec![rise; 14],
            things: vec![
                vec![1.0, 0.0, 6.0],
                vec![1.0, 0.0, 11.0],
                vec![1.0, 0.0, 16.0],
                vec![2.0, -0.6, 22.0],
                vec![1.0, 0.6, 28.0],
                vec![1.0, 0.6, 34.0],
                vec![2.0, 0.0, 44.0],
                vec![3.0, 0.0, 60.0],
                vec![6.0, 0.0, 40.0, 0.0],
            ],
            text: text.into(),
            bound_at: 0,
            frame: 0,
        }
    }

    /// Sample frames to docs/ (`cargo test -p exact-armadillo-run-gpu frames -- --ignored`).
    #[test]
    #[ignore]
    fn frames() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs");
        std::fs::create_dir_all(&root).unwrap();
        let hud = "1,1|BEETLES {017}^28,1|GOAL {040}";
        for (name, g) in [
            ("straight", sample(0.0, 0.0, 0.0, hud)),
            ("curve", sample(0.9, 0.007, 0.0, hud)),
            ("hill", sample(-0.5, -0.004, 0.005, hud)),
            ("overhead", sample(2.9, 0.004, 0.0, hud)),
            ("fixed", Game { player: vec![1.1, 0.0, 0.0, 5.0, 0.0, 0.0, 0.0], ..sample(1.1, 0.007, 0.0, hud) }),
            ("gem", Game { things: vec![vec![6.0, 0.0, 7.0, 0.0]], ..sample(0.0, 0.0, 0.0, "") }),
            ("turquoise", Game { player: vec![0.0, 0.0, 0.0, 5.0, 0.0, 0.0, 1.0], ..sample(0.0, 0.0, 0.0, hud) }),
            ("midnight", Game { player: vec![0.0, 0.0, 0.0, 5.0, 0.0, 0.0, 2.0], ..sample(0.0, 0.0, 0.0, hud) }),
        ] {
            let mut rgba = vec![0u8; WIDTH * HEIGHT * 4];
            g.draw_frame(&mut rgba);
            std::fs::write(root.join(format!("{name}.png")), nes_ppu::png::encode(WIDTH as u32, HEIGHT as u32, &rgba)).unwrap();
        }
    }

    /// How long a frame takes (`cargo test --release -p exact-armadillo-run-gpu speed -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn speed() {
        let g = sample(0.4, 0.007, 0.003, "");
        let mut rgba = vec![0u8; WIDTH * HEIGHT * 4];
        g.draw_frame(&mut rgba);
        let t = std::time::Instant::now();
        for _ in 0..60 {
            g.draw_frame(&mut rgba);
        }
        println!("a frame: {:.2} ms", t.elapsed().as_secs_f64() * 1000.0 / 60.0);
    }

    #[test]
    fn the_course_table_starts_level_and_bends_the_way_it_curves() {
        let b = bend_table(&[0.01; 4], &[-0.01; 4]);
        assert_eq!(b[0], (0.0, 0.0));
        assert!(b[4].0 > b[2].0 * 3.0, "the curve tightens away: {:?}", b);
        assert!(b[4].1 < 0.0, "a dip drops away");
    }

    #[test]
    fn things_on_the_walls_lean_toward_the_axis() {
        assert_eq!(roll_index(0.0), ROLLS / 2);
        assert!(roll_index(1.0) < ROLLS / 2, "the right wall leans left");
        assert_eq!(roll_index(PI), 0, "overhead: upside down");
        assert_eq!(roll_index(-PI + 0.01), 0);
    }
}

#[cfg(test)]
mod audio {
    use super::music;
    use ym2612::driver::{render as render_song, render_writes, tokens};
    use ym2612::wav;

    const RATE: u32 = 44100;

    fn peak(s: &[f32]) -> f32 {
        s.iter().fold(0.0f32, |a, v| a.max(v.abs()))
    }

    /// Renders the stage music (two passes, for a seamless loop) and the
    /// effects to assets/ (`cargo test -p exact-armadillo-run-gpu render -- --ignored`).
    #[test]
    #[ignore]
    fn render() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets");
        std::fs::create_dir_all(root.join("music")).unwrap();
        std::fs::create_dir_all(root.join("sfx")).unwrap();
        let rows = music::STAGE.rows.iter().map(|r| tokens(r).len()).fold(1, usize::max);
        let r = render_song(&music::STAGE, rows * 2, RATE);
        let gain = 0.8 / peak(&r.samples).max(1e-6);
        std::fs::write(root.join("music/stage.wav"), wav::encode_stereo(&r.samples, RATE, gain)).unwrap();
        for e in music::effects() {
            let s = render_writes(&e.script.writes, e.frames, &[], RATE);
            let gain = 0.85 / peak(&s).max(1e-6);
            std::fs::write(root.join(format!("sfx/{}.wav", e.file)), wav::encode_stereo(&s, RATE, gain)).unwrap();
        }
    }

    #[test]
    fn the_music_and_effects_make_sound() {
        let r = render_song(&music::STAGE, 16, 22050);
        assert!(peak(&r.samples) > 0.1);
        for ch in 0..6 {
            assert!(r.rows.iter().any(|c| c[ch].volume > 0), "channel {ch} is silent");
        }
        for e in music::effects() {
            assert!(peak(&render_writes(&e.script.writes, e.frames, &[], 22050)) > 0.02, "{} is silent", e.file);
        }
    }

    #[test]
    fn every_bar_is_sixteen_rows() {
        for (ch, text) in music::STAGE.rows.iter().enumerate() {
            for (b, bar) in text.split('|').enumerate() {
                assert_eq!(bar.split_whitespace().count(), 16, "channel {} bar {}", ch + 1, b + 1);
            }
        }
    }
}
