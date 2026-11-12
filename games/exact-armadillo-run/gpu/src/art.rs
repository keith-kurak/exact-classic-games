//! The art, made when the game starts and kept as 4-bit sprite frames, the
//! way an arcade board's ROM held them.
//!
//! The armadillo, the beetles and the thorn balls are small solid models
//! (spheres, ellipsoids and capsules) rendered once to pixel art: each spin
//! frame and each tilt (for running up the walls) is its own frame, as an
//! artist would have drawn them. The board's scaler then zooms whichever frame
//! is wanted to its true size every frame: nothing is pre-shrunk.

use sys16::{rgb8, Image, Palette, Tiles, Layer};

// ── Palette lines ──
pub const P_DILLO: usize = 0;
pub const P_BEETLE: usize = 1;
pub const P_THORN: usize = 2;
pub const P_TUBE: usize = 3;
pub const P_GATE: usize = 4;
pub const P_START: usize = 5;
pub const P_MESAS: usize = 6;
pub const P_STARS: usize = 7;
pub const P_TEXT: u8 = 8;
pub const P_GOLD: u8 = 9;
pub const P_RED: u8 = 10;
/// The track's colour schemes, in the order the title offers them: dusk,
/// turquoise, midnight.
pub const P_SCHEMES: [usize; 3] = [P_TUBE, 11, 12];
/// The gem at the end: blue topaz, Texas's state gem.
pub const P_GEM: usize = 13;

fn ramp(p: &mut Palette, from: usize, dark: (u8, u8, u8), light: (u8, u8, u8), n: usize) {
    for i in 0..n {
        let t = i as f32 / (n - 1).max(1) as f32;
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        p[from + i] = rgb8(m(dark.0, light.0), m(dark.1, light.1), m(dark.2, light.2));
    }
}

pub fn palettes() -> Vec<Palette> {
    let mut dillo = [0u16; 16];
    ramp(&mut dillo, 1, (60, 30, 22), (232, 178, 112), 8); // shell: 1–8
    ramp(&mut dillo, 9, (150, 70, 80), (255, 190, 180), 3); // skin: 9–11
    dillo[12] = rgb8(26, 12, 14); // outline
    dillo[13] = rgb8(10, 6, 8); // eye
    dillo[14] = rgb8(255, 250, 235); // glint
    let mut beetle = [0u16; 16];
    ramp(&mut beetle, 1, (70, 34, 4), (255, 236, 110), 10); // gold: 1–10
    beetle[11] = rgb8(255, 255, 230); // shine
    beetle[12] = rgb8(40, 16, 0); // outline
    let mut thorn = [0u16; 16];
    ramp(&mut thorn, 1, (14, 54, 34), (150, 220, 120), 7); // cactus: 1–7
    ramp(&mut thorn, 8, (150, 20, 60), (255, 120, 170), 3); // bloom: 8–10
    thorn[11] = rgb8(255, 246, 210); // spines
    thorn[12] = rgb8(8, 24, 14); // outline
    // The tube: 1–2 the rim's lip, 3 the rails, 4–5 the running track's
    // blocks, 6–9 the walls' bands, 10 seams.
    let c = |v: (u8, u8, u8)| rgb8(v.0, v.1, v.2);
    let tube_in = |track: [(u8, u8, u8); 2], rail: (u8, u8, u8), walls: [(u8, u8, u8); 5]| {
        let mut p = [0u16; 16];
        p[1] = rgb8(255, 214, 90);
        p[2] = rgb8(170, 100, 30);
        p[3] = c(rail);
        p[4] = c(track[0]);
        p[5] = c(track[1]);
        for (k, w) in walls.iter().enumerate() {
            p[6 + k] = c(*w);
        }
        p
    };
    let sandstone = [(214, 120, 70), (178, 84, 52), (140, 56, 44), (104, 36, 40), (70, 24, 34)];
    let tube = |track: [(u8, u8, u8); 2], rail: (u8, u8, u8)| tube_in(track, rail, sandstone);
    let mut mesas = [0u16; 16];
    ramp(&mut mesas, 1, (40, 18, 50), (150, 70, 90), 6);
    let mut stars = [0u16; 16];
    stars[1] = rgb8(255, 255, 255);
    stars[2] = rgb8(150, 160, 230);
    let text = |ink: (u8, u8, u8), shadow: (u8, u8, u8)| {
        let mut p = [0u16; 16];
        p[1] = rgb8(ink.0, ink.1, ink.2);
        p[2] = rgb8(shadow.0, shadow.1, shadow.2);
        p
    };
    vec![
        dillo,
        beetle,
        thorn,
        tube([(240, 196, 120), (196, 136, 70)], (255, 244, 214)),
        tube([(255, 120, 200), (250, 250, 250)], (255, 255, 255)),
        tube([(250, 250, 250), (40, 40, 50)], (255, 214, 90)),
        mesas,
        stars,
        text((255, 255, 255), (30, 20, 70)),
        text((255, 220, 60), (100, 40, 0)),
        text((255, 110, 90), (70, 0, 20)),
        // Turquoise: teal blocks, indigo walls.
        tube_in([(70, 200, 200), (20, 120, 150)], (255, 255, 255), [(80, 70, 170), (60, 50, 140), (44, 36, 110), (30, 24, 84), (18, 14, 54)]),
        // Midnight: a dark slate track, white rails, deep teal walls.
        tube_in([(60, 64, 84), (34, 36, 52)], (255, 255, 255), [(40, 90, 100), (30, 72, 84), (22, 56, 68), (16, 40, 52), (10, 24, 34)]),
        {
            let mut gem = [0u16; 16];
            ramp(&mut gem, 1, (10, 30, 110), (170, 235, 255), 10); // topaz: 1–10
            gem[11] = rgb8(255, 255, 255); // glints
            gem[12] = rgb8(4, 10, 50); // outline
            gem
        },
    ]
}

// ── A small solid-model renderer ──

#[derive(Clone, Copy, Debug)]
pub struct V3(pub f32, pub f32, pub f32);

impl V3 {
    fn add(self, o: V3) -> V3 {
        V3(self.0 + o.0, self.1 + o.1, self.2 + o.2)
    }
    fn sub(self, o: V3) -> V3 {
        V3(self.0 - o.0, self.1 - o.1, self.2 - o.2)
    }
    fn mul(self, k: f32) -> V3 {
        V3(self.0 * k, self.1 * k, self.2 * k)
    }
    fn dot(self, o: V3) -> f32 {
        self.0 * o.0 + self.1 * o.1 + self.2 * o.2
    }
    fn len(self) -> f32 {
        self.dot(self).sqrt()
    }
    fn norm(self) -> V3 {
        self.mul(1.0 / self.len().max(1e-6))
    }
    /// Turned about the vertical axis.
    fn yaw(self, a: f32) -> V3 {
        let (s, c) = a.sin_cos();
        V3(c * self.0 + s * self.2, self.1, -s * self.0 + c * self.2)
    }
    /// Turned about the across axis (tipping the far side down).
    fn pitch(self, a: f32) -> V3 {
        let (s, c) = a.sin_cos();
        V3(self.0, c * self.1 - s * self.2, s * self.1 + c * self.2)
    }
}

#[derive(Clone, Copy, Debug)]
enum Shape {
    Ellipsoid { c: V3, r: V3 },
    Capsule { a: V3, b: V3, r: f32 },
    /// An ellipsoid with everything below `floor` cut away.
    Dome { c: V3, r: V3, floor: f32 },
}

#[derive(Clone, Copy, Debug)]
pub struct Part {
    shape: Shape,
    /// What it is made of: the game's colouring decides from this.
    pub mat: u8,
}

fn ellipsoid_d(p: V3, c: V3, r: V3) -> f32 {
    let q = p.sub(c);
    let k0 = V3(q.0 / r.0, q.1 / r.1, q.2 / r.2).len();
    let k1 = V3(q.0 / (r.0 * r.0), q.1 / (r.1 * r.1), q.2 / (r.2 * r.2)).len();
    if k1 < 1e-6 {
        return -r.0.min(r.1).min(r.2);
    }
    k0 * (k0 - 1.0) / k1
}

impl Part {
    pub fn ellipsoid(c: V3, r: V3, mat: u8) -> Part {
        Part { shape: Shape::Ellipsoid { c, r }, mat }
    }
    pub fn sphere(c: V3, r: f32, mat: u8) -> Part {
        Part::ellipsoid(c, V3(r, r, r), mat)
    }
    pub fn capsule(a: V3, b: V3, r: f32, mat: u8) -> Part {
        Part { shape: Shape::Capsule { a, b, r }, mat }
    }
    pub fn dome(c: V3, r: V3, floor: f32, mat: u8) -> Part {
        Part { shape: Shape::Dome { c, r, floor }, mat }
    }
    fn dist(&self, p: V3) -> f32 {
        match self.shape {
            Shape::Ellipsoid { c, r } => ellipsoid_d(p, c, r),
            Shape::Capsule { a, b, r } => {
                let (pa, ba) = (p.sub(a), b.sub(a));
                let h = (pa.dot(ba) / ba.dot(ba)).clamp(0.0, 1.0);
                pa.sub(ba.mul(h)).len() - r
            }
            Shape::Dome { c, r, floor } => ellipsoid_d(p, c, r).max(floor - p.1),
        }
    }
}

/// How the model is seen: turned `yaw` about its vertical axis, the camera
/// looking down by `pitch`, the picture turned `roll` (clockwise) about the
/// model's origin, at `scale` pixels a model unit.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub scale: f32,
}

/// A frame of art, with where its model's origin (the point it stands on)
/// falls within it, 0–1.
#[derive(Clone, Debug)]
pub struct Frame {
    pub image: Image,
    pub anchor: (f32, f32),
}

fn scene_dist(parts: &[Part], p: V3) -> (f32, usize) {
    let mut best = (f32::MAX, 0);
    for (i, part) in parts.iter().enumerate() {
        let d = part.dist(p);
        if d < best.0 {
            best = (d, i);
        }
    }
    best
}

/// Render a model. `colour(material, point, normal, light)` picks each
/// pixel's palette index (0 leaves it clear); `outline` rings the silhouette.
pub fn render(parts: &[Part], view: View, reach: f32, outline: u8, colour: &dyn Fn(u8, V3, V3, f32) -> u8) -> Frame {
    // The picture is a square about the origin, big enough for the model
    // turned any way; then cropped to what was drawn.
    let half = (reach * view.scale).ceil() as i32 + 2;
    let size = (half * 2) as usize;
    let mut img = Image::new(size, size);
    let light = V3(-0.45, 0.75, 0.5).norm();
    let (rs, rc) = view.roll.sin_cos();
    for py in 0..size {
        for px in 0..size {
            // Screen to view space, the roll undone.
            let (sx, sy) = ((px as f32 + 0.5 - half as f32) / view.scale, -(py as f32 + 0.5 - half as f32) / view.scale);
            let (vx, vy) = (rc * sx - rs * sy, rs * sx + rc * sy);
            if vx * vx + vy * vy > reach * reach {
                continue;
            }
            // View to model space: undo the pitch, then the yaw.
            let to_model = |v: V3| v.pitch(-view.pitch).yaw(-view.yaw);
            let origin = to_model(V3(vx, vy, reach * 2.0));
            let dir = to_model(V3(0.0, 0.0, -1.0));
            let mut t = 0.0;
            let mut hit = None;
            for _ in 0..64 {
                let p = origin.add(dir.mul(t));
                let (d, i) = scene_dist(parts, p);
                if d < 0.002 {
                    hit = Some((p, i));
                    break;
                }
                t += d.max(0.002);
                if t > reach * 4.0 {
                    break;
                }
            }
            let Some((p, i)) = hit else { continue };
            let e = 0.003;
            let g = |o: V3| scene_dist(parts, p.add(o)).0 - scene_dist(parts, p.sub(o)).0;
            let n = V3(g(V3(e, 0.0, 0.0)), g(V3(0.0, e, 0.0)), g(V3(0.0, 0.0, e))).norm();
            // Light in view space, carried to the model.
            let l = to_model(light);
            let c = colour(parts[i].mat, p, n, n.dot(l));
            img.set(px, py, c);
        }
    }
    if outline > 0 {
        let src = img.clone();
        for y in 0..size {
            for x in 0..size {
                if src.get(x, y) != 0 {
                    continue;
                }
                let near = [(0i32, 1i32), (0, -1), (1, 0), (-1, 0)].iter().any(|(dx, dy)| {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    nx >= 0 && ny >= 0 && (nx as usize) < size && (ny as usize) < size && src.get(nx as usize, ny as usize) != 0
                });
                if near {
                    img.set(x, y, outline);
                }
            }
        }
    }
    crop(&img, (half as f32, half as f32))
}

fn crop(img: &Image, origin: (f32, f32)) -> Frame {
    let (mut x0, mut y0, mut x1, mut y1) = (img.width, img.height, 0, 0);
    for y in 0..img.height {
        for x in 0..img.width {
            if img.get(x, y) != 0 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    if x1 <= x0 {
        return Frame { image: Image::new(1, 1), anchor: (0.5, 0.5) };
    }
    let mut out = Image::new(x1 - x0, y1 - y0);
    for y in y0..y1 {
        for x in x0..x1 {
            out.set(x - x0, y - y0, img.get(x, y));
        }
    }
    let anchor = ((origin.0 - x0 as f32) / out.width as f32, (origin.1 - y0 as f32) / out.height as f32);
    Frame { image: out, anchor }
}

/// A light level (−1 to 1) to one of `n` steps starting at `from`.
fn step(light: f32, from: u8, n: u8, ambient: f32) -> u8 {
    let v = (ambient + (1.0 - ambient) * light.max(0.0)).clamp(0.0, 0.999);
    from + (v * n as f32) as u8
}

// ── The armadillo ──
// Model units: it stands on the origin, facing away from you (−z), about
// 0.75 tall and 1.4 long with its tail.

const SHELL: u8 = 1;
const SKIN: u8 = 2;
const EAR: u8 = 3;
const CLAW: u8 = 4;

/// The armadillo running: `phase` 0–1 through a stride.
pub fn armadillo(phase: f32) -> Vec<Part> {
    let tau = std::f32::consts::TAU;
    let bob = (phase * tau * 2.0).sin().abs() * 0.035;
    let mut parts = vec![
        // The banded shell, cut flat underneath.
        Part::dome(V3(0.0, 0.40 + bob, 0.02), V3(0.40, 0.34, 0.56), 0.18 + bob, SHELL),
        // Belly and flanks below it.
        Part::ellipsoid(V3(0.0, 0.27 + bob, 0.0), V3(0.30, 0.14, 0.46), SKIN),
        // Head shield and snout, ahead (mostly hidden), ears up behind it.
        Part::ellipsoid(V3(0.0, 0.36 + bob, -0.58), V3(0.16, 0.15, 0.24), SHELL),
        Part::ellipsoid(V3(0.0, 0.28 + bob, -0.80), V3(0.07, 0.07, 0.14), SKIN),
        Part::ellipsoid(V3(-0.11, 0.56 + bob, -0.60), V3(0.05, 0.11, 0.035), EAR),
        Part::ellipsoid(V3(0.11, 0.56 + bob, -0.60), V3(0.05, 0.11, 0.035), EAR),
        // The tail, banded, curving down behind.
        Part::capsule(V3(0.0, 0.36 + bob, 0.52), V3(0.0, 0.22 + bob, 0.78), 0.065, SHELL),
        Part::capsule(V3(0.0, 0.22 + bob, 0.78), V3(0.0, 0.10, 0.98), 0.04, SHELL),
    ];
    // Legs: diagonal pairs together (front left with back right).
    for (x, z, off) in [(-0.22, -0.30, 0.0), (0.22, 0.34, 0.0), (0.22, -0.30, 0.5), (-0.22, 0.34, 0.5)] {
        let a = (phase + off) * tau;
        let (fz, lift) = (z + 0.16 * a.cos(), (0.10 * a.sin()).max(0.0));
        let hip = V3(x, 0.24 + bob, z);
        let foot = V3(x * 1.1, 0.04 + lift, fz);
        parts.push(Part::capsule(hip, foot, 0.06, SKIN));
        parts.push(Part::capsule(foot, foot.add(V3(0.0, -0.02, -0.06)), 0.035, CLAW));
    }
    parts
}

/// The armadillo rolled into a ball, turned `spin` (radians) forward.
pub fn armadillo_ball(spin: f32) -> (Vec<Part>, f32) {
    (vec![Part::sphere(V3(0.0, 0.40, 0.0), 0.40, SHELL)], spin)
}

pub fn dillo_colour(spin: f32) -> impl Fn(u8, V3, V3, f32) -> u8 {
    move |mat, p, n, light| match mat {
        SHELL => {
            // Bands across the back: grooves every so far along, and a row
            // of scutes (small plates) between them.
            let along = if spin != 0.0 {
                // Rolled: the bands run round the ball, turning with it.
                let q = V3(p.0, p.1 - 0.40, p.2).pitch(spin);
                q.2.atan2(q.1) / std::f32::consts::PI * 2.6
            } else {
                p.2 * 7.5
            };
            let groove = along.rem_euclid(1.0) < 0.16;
            let scute = ((p.0 * 14.0).rem_euclid(1.0) < 0.12) && !groove;
            let mut c = step(light, 1, 8, 0.25);
            if groove {
                c = c.saturating_sub(3).max(1);
            } else if scute {
                c = c.saturating_sub(1).max(1);
            }
            if light > 0.9 && n.1 > 0.5 && !groove {
                c = 14;
            }
            c
        }
        SKIN => step(light, 9, 3, 0.3),
        EAR => step(light, 9, 3, 0.45),
        _ => 13,
    }
}

// ── The golden beetle ──
// A rhinoceros beetle, standing, facing −z; about 1.1 long.

const GOLD: u8 = 1;
const LEG: u8 = 2;

pub fn beetle() -> Vec<Part> {
    let mut parts = vec![
        Part::dome(V3(0.0, 0.30, 0.10), V3(0.32, 0.24, 0.40), 0.12, GOLD),
        Part::ellipsoid(V3(0.0, 0.28, -0.32), V3(0.25, 0.17, 0.16), GOLD),
        Part::ellipsoid(V3(0.0, 0.22, -0.50), V3(0.12, 0.10, 0.10), GOLD),
        Part::capsule(V3(0.0, 0.26, -0.56), V3(0.0, 0.46, -0.70), 0.045, GOLD),
        Part::capsule(V3(0.0, 0.46, -0.70), V3(0.0, 0.54, -0.62), 0.035, GOLD),
    ];
    for (z, swing) in [(-0.30, -0.16), (-0.05, 0.0), (0.20, 0.16)] {
        for side in [-1.0f32, 1.0] {
            let knee = V3(side * 0.40, 0.22, z + swing * 0.5);
            parts.push(Part::capsule(V3(side * 0.18, 0.20, z), knee, 0.04, LEG));
            parts.push(Part::capsule(knee, V3(side * 0.48, 0.0, z + swing), 0.032, LEG));
        }
    }
    parts
}

pub fn beetle_colour(mat: u8, p: V3, n: V3, light: f32) -> u8 {
    match mat {
        GOLD => {
            // Polished gold: a hard highlight, and the seam down the back.
            if p.0.abs() < 0.02 && p.2 > -0.16 && n.1 > 0.2 {
                return 2;
            }
            let spec = light > 0.0 && n.1 > 0.55 && light > 0.86;
            if spec {
                11
            } else {
                step(light, 1, 10, 0.42)
            }
        }
        _ => step(light, 1, 4, 0.1),
    }
}

// ── The thorn ball ──
// A round cactus, rolling, covered in spines, with a pink bloom.

const CACTUS: u8 = 1;
const SPINE: u8 = 2;
const BLOOM: u8 = 3;

pub fn thorn_ball() -> Vec<Part> {
    let mut parts = vec![Part::sphere(V3(0.0, 0.42, 0.0), 0.38, CACTUS)];
    // Spines out along the directions of a golden spiral.
    let n = 26;
    for i in 0..n {
        let y = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
        let r = (1.0 - y * y).sqrt();
        let a = i as f32 * 2.399_963;
        let d = V3(r * a.cos(), y, r * a.sin());
        let c = V3(0.0, 0.42, 0.0);
        parts.push(Part::capsule(c.add(d.mul(0.34)), c.add(d.mul(0.56)), 0.022, SPINE));
    }
    parts.push(Part::sphere(V3(0.0, 0.80, 0.0), 0.09, BLOOM));
    parts
}

pub fn thorn_colour(mat: u8, p: V3, _n: V3, light: f32) -> u8 {
    match mat {
        CACTUS => {
            // Ribs running pole to pole.
            let rib = (p.2.atan2(p.0) * 5.0 / std::f32::consts::PI).rem_euclid(1.0) < 0.2;
            let c = step(light, 1, 7, 0.22);
            if rib { c.saturating_sub(2).max(1) } else { c }
        }
        SPINE => 11,
        _ => step(light, 8, 3, 0.4),
    }
}

// ── The Texas gem ──

/// Texas's outline (longitude, latitude), clockwise from the panhandle's
/// north-west corner.
const TEXAS: [(f32, f32); 38] = [
    (-103.0, 36.5), (-100.0, 36.5), (-100.0, 34.56), (-99.2, 34.3), (-98.1, 34.1), (-97.0, 33.8),
    (-95.8, 33.85), (-94.9, 33.65), (-94.04, 33.55), (-94.04, 31.99), (-93.6, 31.1), (-93.7, 30.3),
    (-93.85, 29.7), (-94.7, 29.4), (-95.3, 28.9), (-96.0, 28.55), (-96.6, 28.15), (-97.2, 27.6),
    (-97.4, 26.8), (-97.15, 25.95), (-97.6, 25.9), (-98.4, 26.2), (-99.1, 26.5), (-99.5, 27.5),
    (-100.3, 28.3), (-100.7, 29.1), (-101.2, 29.6), (-101.7, 29.8), (-102.4, 29.85), (-102.9, 29.2),
    (-103.3, 29.0), (-103.9, 29.3), (-104.5, 29.7), (-104.7, 30.4), (-105.2, 30.8), (-106.0, 31.4),
    (-106.6, 31.9), (-103.06, 32.0),
];

const GEM_W: usize = 96;
const GEM_H: usize = 90;

/// The outline in the gem's pixels.
fn texas_px() -> Vec<(f32, f32)> {
    let (w, e, n, s) = (-106.8f32, -93.4f32, 36.6f32, 25.8f32);
    TEXAS.iter().map(|&(lon, lat)| (2.0 + (lon - w) / (e - w) * (GEM_W as f32 - 4.0), 2.0 + (n - lat) / (n - s) * (GEM_H as f32 - 4.0))).collect()
}

fn inside(poly: &[(f32, f32)], x: f32, y: f32) -> bool {
    let mut odd = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let ((xi, yi), (xj, yj)) = (poly[i], poly[j]);
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            odd = !odd;
        }
        j = i;
    }
    odd
}

/// The nearest edge to a point: its distance, and the edge's outward normal.
fn nearest_edge(poly: &[(f32, f32)], x: f32, y: f32) -> (f32, (f32, f32)) {
    let mut best = (f32::MAX, (0.0, 0.0));
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let ((ax, ay), (bx, by)) = (poly[j], poly[i]);
        let (dx, dy) = (bx - ax, by - ay);
        let h = (((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        let (px, py) = (ax + dx * h - x, ay + dy * h - y);
        let d = (px * px + py * py).sqrt();
        if d < best.0 {
            let len = (dx * dx + dy * dy).sqrt();
            // Clockwise on screen, so the outside is to the left of travel.
            best = (d, (dy / len, -dx / len));
        }
        j = i;
    }
    best
}

/// The gem face-on: a bevel round the edge lit by its slope, a table cut
/// into facets that run from the heart to each corner, and glints.
fn gem_face() -> Image {
    let poly = texas_px();
    let n = poly.len() as f32;
    let (cx, cy) = (poly.iter().map(|p| p.0).sum::<f32>() / n, poly.iter().map(|p| p.1).sum::<f32>() / n + 4.0);
    let light = (-0.6f32, -0.8f32);
    let mut img = Image::new(GEM_W, GEM_H);
    for y in 0..GEM_H {
        for x in 0..GEM_W {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            if !inside(&poly, fx, fy) {
                continue;
            }
            let (d, nrm) = nearest_edge(&poly, fx, fy);
            let c = if d < 7.0 {
                // The bevel: tilted toward its edge.
                let l = nrm.0 * light.0 + nrm.1 * light.1;
                (5.5 + l * 4.5).round().clamp(1.0, 10.0) as u8
            } else {
                // The table: facets by the direction from the heart, every
                // other one a shade brighter, and darker toward the bottom.
                let a = (fy - cy).atan2(fx - cx);
                let facet = ((a / std::f32::consts::TAU * 12.0).rem_euclid(12.0)) as i32;
                let base = 7 - ((fy - cy) / 18.0).clamp(-2.0, 2.0) as i32;
                (base + if facet % 2 == 0 { 1 } else { -1 }).clamp(2, 10) as u8
            };
            img.set(x, y, c);
        }
    }
    // Glints: a few bright crosses on the bevel facing the light.
    for (gx, gy) in [(30, 14), (64, 34), (40, 46), (70, 60)] {
        for k in -2i32..=2 {
            img.set((gx + k) as usize, gy, 11);
            img.set(gx as usize, (gy as i32 + k) as usize, 11);
        }
    }
    img
}

pub const GEM_FRAMES: usize = 8;

/// The gem turning about its upright axis, frame `f` of `GEM_FRAMES` (half
/// a turn; its back is its face mirrored, a little darker). Edge-on, it is
/// a thin slab.
pub fn gem(f: usize) -> Frame {
    let face = gem_face();
    let a = f as f32 / GEM_FRAMES as f32 * std::f32::consts::PI;
    let c = a.cos();
    let mut img = Image::new(GEM_W, GEM_H);
    let mid = GEM_W as f32 / 2.0;
    let half = (mid * c.abs()).max(2.0);
    for y in 0..GEM_H {
        for x in 0..GEM_W {
            let dx = x as f32 + 0.5 - mid;
            if dx.abs() > half {
                continue;
            }
            let u = mid + dx / half * mid * c.signum();
            let v = face.get((u as usize).min(GEM_W - 1), y);
            if v == 0 {
                continue;
            }
            let v = if c.abs() < 0.2 { 4 } else if c < 0.0 && v < 11 { v.saturating_sub(2).max(1) } else { v };
            img.set(x, y, v);
        }
    }
    // Outline.
    let src = img.clone();
    for y in 0..GEM_H {
        for x in 0..GEM_W {
            if src.get(x, y) == 0 && [(0i32, 1i32), (0, -1), (1, 0), (-1, 0)].iter().any(|(dx, dy)| {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                nx >= 0 && ny >= 0 && (nx as usize) < GEM_W && (ny as usize) < GEM_H && src.get(nx as usize, ny as usize) != 0
            }) {
                img.set(x, y, 12);
            }
        }
    }
    crop(&img, (mid, GEM_H as f32 / 2.0))
}

/// A four-pointed sparkle, frame `f` of four: it flashes out, then shrinks
/// and fades to a few pixels.
pub fn sparkle(f: usize) -> Frame {
    let mut img = Image::new(15, 15);
    let reach = [5.0, 7.0, 5.0, 3.0][f.min(3)];
    let (core, glow) = if f < 2 { (11, 10) } else { (10, 7) };
    for y in 0..15 {
        for x in 0..15 {
            let (dx, dy) = ((x as f32 - 7.0).abs(), (y as f32 - 7.0).abs());
            // Thin arms along the axes, a small diamond at the heart.
            let arm = (dx < 0.5 && dy <= reach) || (dy < 0.5 && dx <= reach);
            let heart = dx + dy <= reach * 0.35 + 0.5;
            if heart {
                img.set(x, y, core);
            } else if arm {
                img.set(x, y, glow);
            }
        }
    }
    crop(&img, (7.5, 7.5))
}

/// A soft oval for shadows, tilted with the wall it lies on.
pub fn shadow(roll: f32) -> Frame {
    let mut img = Image::new(48, 48);
    let (s, c) = roll.sin_cos();
    for y in 0..48 {
        for x in 0..48 {
            let (dx, dy) = (x as f32 + 0.5 - 24.0, y as f32 + 0.5 - 24.0);
            let (u, v) = (c * dx + s * dy, -s * dx + c * dy);
            if (u / 23.0).powi(2) + (v / 7.0).powi(2) <= 1.0 {
                img.set(x, y, 1);
            }
        }
    }
    crop(&img, (24.0, 24.0))
}

// ── The tube's skin ──

/// 128 across, once round the pipe (the floor's middle at 64), by 32
/// along. The lower half is the half-pipe: a lip at each rim, the walls in
/// bands of sandstone, cream rails, and a running track of light and dark
/// blocks down the middle. The upper half is an open frame: arched ribs and
/// three beams, with the sky between them (colour 0 is clear).
pub fn skin() -> Image {
    let mut img = Image::new(128, 32);
    for v in 0..32 {
        for u in 0..128 {
            let d = (u as i32 - 64).unsigned_abs() as usize;
            let c = if d >= 32 {
                // The frame overhead.
                let rib = v % 16 < 3;
                let beam = matches!(d, 44 | 45 | 63 | 64);
                if rib || beam {
                    if rib && beam { 2 } else if v % 16 == 0 || d == 44 || d == 63 { 8 } else { 9 }
                } else {
                    0
                }
            } else if d >= 30 {
                if d >= 31 { 2 } else { 1 }
            } else if d >= 10 {
                // Walls: bands along the run, darker toward the rims.
                let band = (v / 8) % 2;
                let depth = ((d - 10) / 7).min(2);
                if v % 8 == 0 { 10 } else { (6 + depth + band) as u8 }
            } else if d >= 8 {
                // Rails: a dashed stripe either side of the track.
                if d == 9 && v % 16 < 12 { 3 } else { 9 }
            } else {
                // The track: blocks four across, alternating along.
                let block = ((u / 4) + (v / 8)) % 2;
                if v % 8 == 7 || u % 4 == 0 { 10 } else { 4 + block as u8 }
            };
            img.set(u, v, c);
        }
    }
    img
}

/// Mesas on the horizon, 512×32, cut into tiles.
pub fn mesas(tiles: &mut Tiles) -> Layer {
    let mut img = Image::new(512, 32);
    for x in 0..512 {
        let t = x as f32 / 512.0 * std::f32::consts::TAU;
        // Flat-topped buttes: a clipped sum of waves.
        let raw = 10.0 + 9.0 * (t * 3.0 + 0.4).sin() + 6.0 * (t * 7.0).sin() + 3.0 * (t * 13.0 + 1.0).sin();
        let h = raw.clamp(4.0, 22.0);
        for y in 0..32 {
            let up = 32.0 - y as f32;
            if up <= h {
                img.set(x, y, 1 + ((up / h) * 5.0).min(5.0) as u8);
            }
        }
    }
    let ids = tiles.cut(&img);
    let mut layer = Layer::new(64, 4);
    for (i, id) in ids.into_iter().enumerate() {
        layer.map[i] = (id, P_MESAS as u8);
    }
    layer
}

/// A starfield, 512×128.
pub fn stars(tiles: &mut Tiles) -> Layer {
    let mut img = Image::new(512, 128);
    let mut r: u32 = 99;
    for _ in 0..200 {
        r = r.wrapping_mul(1103515245).wrapping_add(12345);
        let (x, y) = ((r >> 8) as usize % 512, (r >> 20) as usize % 128);
        img.set(x, y, if r % 4 == 0 { 1 } else { 2 });
    }
    let ids = tiles.cut(&img);
    let mut layer = Layer::new(64, 16);
    for (i, id) in ids.into_iter().enumerate() {
        layer.map[i] = (id, P_STARS as u8);
    }
    layer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_renders_cropped_with_its_anchor() {
        let f = render(&[Part::sphere(V3(0.0, 0.5, 0.0), 0.5, 1)], View { yaw: 0.0, pitch: 0.0, roll: 0.0, scale: 20.0 }, 1.2, 9, &|_, _, _, l| step(l, 1, 4, 0.2));
        // A ball of radius 10 px plus its outline: 22 across, standing on the anchor.
        assert_eq!(f.image.width, 22);
        assert!((f.anchor.1 - 21.0 / 22.0).abs() < 0.06, "{:?}", f.anchor);
        assert!(f.image.pixels.contains(&9), "outlined");
    }

    #[test]
    fn the_gem_is_texas_shaped_and_turns() {
        let face = gem(0).image;
        // The panhandle: filled at the top left, empty at the top right.
        assert!(face.width > 80);
        let px = |x: f32, y: f32| face.get((x * face.width as f32) as usize, (y * face.height as f32) as usize);
        assert_ne!(px(0.32, 0.08), 0, "the panhandle");
        assert_eq!(px(0.85, 0.08), 0, "nothing north-east of the Red River");
        assert_ne!(px(0.5, 0.5), 0);
        assert!(gem(4).image.width < 10, "edge-on");
    }

    #[test]
    fn a_rolled_frame_leans() {
        let parts = armadillo(0.0);
        let col = dillo_colour(0.0);
        let up = render(&parts, View { yaw: 0.0, pitch: 0.35, roll: 0.0, scale: 30.0 }, 1.2, 12, &col);
        let side = render(&parts, View { yaw: 0.0, pitch: 0.35, roll: 1.4, scale: 30.0 }, 1.2, 12, &col);
        assert!(side.image.width > up.image.width, "on its side it is wider than tall");
        assert!(side.anchor.0 < 0.5, "its feet are to the left when it leans right");
    }
}
