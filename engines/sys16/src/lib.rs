//! A picture processor in the style of Sega's 1985–87 arcade boards: System
//! 16's colour and tiles, and the Super Scaler's sprite zoom (Space Harrier,
//! Hang-On, Out Run).
//!
//! - 320×224 pixels of 15-bit colour (5 bits a channel, [`rgb`]).
//! - Art is 4 bits a pixel ([`Image`], [`Tiles`]); colour 0 is clear. Each
//!   thing draws in one 16-colour palette line.
//! - Sprites zoom: [`Sprite::zoom`] stretches or shrinks one piece of art to
//!   any size, sampling it the way the hardware stepped through its ROM, a
//!   whole source pixel at a time, so scaled art keeps hard, chunky pixels
//!   that crawl as it grows. Sprites draw far to near; a shadow sprite halves
//!   the colour of whatever it covers (the dark ovals under things).
//! - A raster backdrop: a colour for every scanline (sky gradients).
//! - Tile layers of 8×8 tiles with a horizontal scroll for every line
//!   (parallax mountains, wavy horizons).
//! - A ground ([`Ground`]): a checkerboard in perspective, drawn a line at a
//!   time from the camera's height and the distance travelled, with fog.
//! - A tube ([`Tube`]): a half-pipe seen from inside, curving and climbing,
//!   its skin found for every pixel from the pipe's true geometry (Out Run's
//!   road layer, one step on).
//!
//! [`render`] composes a [`Scene`] into a [`Frame`].

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 224;

/// A 15-bit colour from 5-bit channels (0–31).
pub const fn rgb(r: u8, g: u8, b: u8) -> u16 {
    ((r as u16 & 31) << 10) | ((g as u16 & 31) << 5) | (b as u16 & 31)
}

/// A 15-bit colour from 8-bit channels, as the board's DAC would quantise.
pub const fn rgb8(r: u8, g: u8, b: u8) -> u16 {
    rgb(r >> 3, g >> 3, b >> 3)
}

/// Half the brightness: the shadow circuit.
pub const fn shade(c: u16) -> u16 {
    (c >> 1) & 0b0_01111_01111_01111
}

/// Mix two colours, `t` from 0 (a) to 1 (b), per channel.
pub fn mix(a: u16, b: u16, t: f32) -> u16 {
    let ch = |c: u16, s: u32| ((c >> s) & 31) as f32;
    let m = |s: u32| (ch(a, s) + (ch(b, s) - ch(a, s)) * t.clamp(0.0, 1.0)).round() as u16;
    (m(10) << 10) | (m(5) << 5) | m(0)
}

/// 16 colours; entry 0 is never drawn (clear).
pub type Palette = [u16; 16];

/// A piece of 4-bit art for sprites.
#[derive(Clone, Debug)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl Image {
    pub fn new(width: usize, height: usize) -> Image {
        Image { width, height, pixels: vec![0; width * height] }
    }
    pub fn get(&self, x: usize, y: usize) -> u8 {
        self.pixels[y * self.width + x]
    }
    pub fn set(&mut self, x: usize, y: usize, c: u8) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = c & 15;
        }
    }
}

/// One sprite: a piece of art, where its anchor lands, and how big.
#[derive(Clone, Copy, Debug)]
pub struct Sprite {
    pub image: usize,
    /// The screen point the anchor lands on.
    pub x: f32,
    pub y: f32,
    /// The anchor within the art, 0–1 (0.5, 1.0 is bottom centre: feet).
    pub anchor_x: f32,
    pub anchor_y: f32,
    /// Screen pixels per art pixel, across and down.
    pub zoom_x: f32,
    pub zoom_y: f32,
    pub palette: usize,
    pub flip_x: bool,
    /// Halve what it covers instead of drawing its colours.
    pub shadow: bool,
    /// Depth: sprites draw from the largest to the smallest.
    pub depth: f32,
}

impl Sprite {
    /// A sprite at (x, y), anchored at its bottom centre, zoomed evenly.
    pub fn zoom(image: usize, x: f32, y: f32, zoom: f32, palette: usize, depth: f32) -> Sprite {
        Sprite { image, x, y, anchor_x: 0.5, anchor_y: 1.0, zoom_x: zoom, zoom_y: zoom, palette, flip_x: false, shadow: false, depth }
    }
}

/// 8×8 4-bit tiles.
#[derive(Clone, Debug, Default)]
pub struct Tiles {
    pub tiles: Vec<[[u8; 8]; 8]>,
}

impl Tiles {
    /// Add a tile; a tile already there is reused.
    pub fn add(&mut self, t: [[u8; 8]; 8]) -> u16 {
        if let Some(i) = self.tiles.iter().position(|x| *x == t) {
            return i as u16;
        }
        self.tiles.push(t);
        (self.tiles.len() - 1) as u16
    }
    /// An image cut into tiles, row by row (its size a multiple of 8).
    pub fn cut(&mut self, img: &Image) -> Vec<u16> {
        let mut ids = Vec::new();
        for ty in 0..img.height / 8 {
            for tx in 0..img.width / 8 {
                let mut t = [[0u8; 8]; 8];
                for y in 0..8 {
                    for x in 0..8 {
                        t[y][x] = img.get(tx * 8 + x, ty * 8 + y);
                    }
                }
                ids.push(self.add(t));
            }
        }
        ids
    }
}

/// A layer of tiles: a map that wraps sideways, a scroll for each line.
#[derive(Clone, Debug)]
pub struct Layer {
    pub cols: usize,
    pub rows: usize,
    /// (tile, palette) for each cell, row by row; tile 0 draws nothing.
    pub map: Vec<(u16, u8)>,
    /// The layer's top on screen.
    pub top: i32,
    /// Horizontal scroll for each screen line (or one value for all).
    pub scroll: Vec<i32>,
}

impl Layer {
    pub fn new(cols: usize, rows: usize) -> Layer {
        Layer { cols, rows, map: vec![(0, 0); cols * rows], top: 0, scroll: vec![0] }
    }
    fn scroll_at(&self, line: usize) -> i32 {
        *self.scroll.get(line).unwrap_or(self.scroll.last().unwrap_or(&0))
    }
}

/// The ground: a checkerboard in perspective. A line `y` below the horizon
/// is `height * focal / (y - horizon)` away; the checks there are found from
/// that distance, the distance travelled and the camera's sideways shift.
#[derive(Clone, Copy, Debug)]
pub struct Ground {
    pub horizon: f32,
    pub height: f32,
    pub focal: f32,
    pub travel: f32,
    pub shift: f32,
    /// The size of a check, in world units, across and along.
    pub check: (f32, f32),
    /// The two colours near, and what they fade to with distance.
    pub colors: [u16; 2],
    pub fog: u16,
    /// Distance at which the fog is total.
    pub fog_far: f32,
}

/// A stripe of the tube drawn in another palette line: a start line, a
/// checkpoint. `at` and `len` are distances along the course.
#[derive(Clone, Copy, Debug)]
pub struct TubeMark {
    pub at: f32,
    pub len: f32,
    pub palette: usize,
}

/// The tube: the inside of a half-pipe, seen from within, running away to a
/// vanishing point, curving and climbing as it goes.
///
/// This is the System 16 family's road layer carried one step on. Out Run's
/// road generator was a layer of its own behind the sprites, drawn line by
/// line from tables the CPU rewrote every frame: where the road's centre
/// was on each line, how wide, which stripe. The tube keeps that split: the
/// game writes a small table each frame (`bend`: how far the tube's axis has
/// swung sideways and up at each step ahead) and the layer does the rest.
/// But where the road chip spread one stripe pattern across each line, the
/// tube finds the true point of the pipe behind every pixel (its angle round
/// the pipe and its distance), so the walls curve as walls do and nothing is
/// a canned frame. A Mega Drive half-pipe was a flip-book of pre-drawn tube
/// frames; this one is computed.
///
/// Coordinates: x across, y up, z ahead of the camera. The pipe's axis
/// starts at the origin; its floor is at `y = -radius`; the camera sits
/// `eye` above the floor, on the axis line.
#[derive(Clone, Debug)]
pub struct Tube<'a> {
    /// The screen line and column the axis runs away to.
    pub horizon: f32,
    pub center: f32,
    pub focal: f32,
    pub radius: f32,
    pub eye: f32,
    /// How far round from the floor the pipe goes before it ends (π/2: a
    /// half-pipe whose rims are level with the axis).
    pub rim: f32,
    /// The distance travelled: the skin scrolls by it.
    pub travel: f32,
    /// The axis's offset (sideways, up) at z = i × `bend_step`; between
    /// entries it is interpolated, past the last it carries on straight.
    pub bend: &'a [(f32, f32)],
    pub bend_step: f32,
    /// The skin: 4-bit art wrapped round the pipe, its width covering
    /// `skin_round` radians centred on the floor (2π: once round), its
    /// height `skin_len` world units along.
    pub skin: usize,
    pub skin_round: f32,
    pub skin_len: f32,
    pub palette: usize,
    pub marks: &'a [TubeMark],
    /// The camera's roll round the pipe's axis (radians): it orbits the axis
    /// and turns with it, so the angle `roll` is straight below it.
    pub roll: f32,
    /// Fog: none until `fog_near`, total at `fog_far`.
    pub fog: u16,
    pub fog_near: f32,
    pub fog_far: f32,
}

impl Tube<'_> {
    /// The axis's offset at depth z, as the rolled camera sees it.
    pub fn bend_at(&self, z: f32) -> (f32, f32) {
        let (x, y) = self.bend_world(z);
        if self.roll == 0.0 {
            return (x, y);
        }
        let (s, c) = self.roll.sin_cos();
        (x * c + y * s, -x * s + y * c)
    }

    /// The axis's offset at depth z, in the world.
    fn bend_world(&self, z: f32) -> (f32, f32) {
        if self.bend.is_empty() {
            return (0.0, 0.0);
        }
        let f = (z / self.bend_step).max(0.0);
        let i = f.floor() as usize;
        if i + 1 < self.bend.len() {
            let t = f - i as f32;
            let (a, b) = (self.bend[i], self.bend[i + 1]);
            (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
        } else {
            // Past the table: on along the last step's heading.
            let n = self.bend.len();
            let last = self.bend[n - 1];
            let prev = if n > 1 { self.bend[n - 2] } else { last };
            let over = f - (n - 1) as f32;
            (last.0 + (last.0 - prev.0) * over, last.1 + (last.1 - prev.1) * over)
        }
    }

    /// Where the camera's ray through screen point (sx, sy) meets the pipe:
    /// its depth and its angle round the pipe from the floor (negative to
    /// the left). The bend is followed by refining the depth a few times.
    pub fn hit(&self, sx: f32, sy: f32) -> Option<(f32, f32)> {
        let (dx, dy) = ((sx - self.center) / self.focal, -(sy - self.horizon) / self.focal);
        let cy = -self.radius + self.eye;
        let a = dx * dx + dy * dy;
        if a < 1e-9 {
            return None;
        }
        let r2 = self.radius * self.radius;
        // The ray against the pipe as it lies at depth z: where it leaves.
        let solve = |bx: f32, by: f32| -> Option<f32> {
            let (ox, oy) = (-bx, cy - by);
            let b = 2.0 * (ox * dx + oy * dy);
            let c = ox * ox + oy * oy - r2;
            let disc = b * b - 4.0 * a * c;
            let t = (-b + disc.max(0.0).sqrt()) / (2.0 * a);
            (disc >= 0.0 && t > 0.0).then_some(t)
        };
        // Straight first, then follow the bend: a few refinements usually
        // settle; where they do not (round a sharp bend), march.
        let mut z = solve(0.0, 0.0)?;
        let mut settled = self.bend.is_empty();
        for _ in 0..4 {
            if settled {
                break;
            }
            let (bx, by) = self.bend_at(z);
            match solve(bx, by) {
                Some(n) => {
                    settled = (n - z).abs() < 0.01 * z.max(1.0);
                    z = n;
                }
                None => break,
            }
        }
        let outside = |z: f32| {
            let (bx, by) = self.bend_at(z);
            let (x, y) = (dx * z - bx, cy + dy * z - by);
            x * x + y * y >= r2
        };
        if !settled || outside(z * 0.85) {
            // March out from the camera to the first point outside the
            // pipe, then halve the step to the wall.
            let (mut lo, mut hi) = (0.3f32, 0.3f32);
            loop {
                if hi > 2000.0 {
                    return None;
                }
                if outside(hi) {
                    break;
                }
                lo = hi;
                hi = hi * 1.1 + 0.2;
            }
            for _ in 0..10 {
                let mid = (lo + hi) * 0.5;
                if outside(mid) { hi = mid } else { lo = mid }
            }
            z = hi;
        }
        let (bx, by) = self.bend_at(z);
        let rel = (dx * z - bx).atan2(-(cy + dy * z - by));
        let ang = wrap(rel + self.roll);
        (ang.abs() <= self.rim).then_some((z, ang))
    }

    /// A point `lift` in from the pipe's surface (toward the axis), at angle
    /// `ang` from the floor and depth z, on screen; and its scale (pixels a
    /// world unit). None behind the camera.
    pub fn project(&self, ang: f32, lift: f32, z: f32) -> Option<(f32, f32, f32)> {
        if z < 0.3 {
            return None;
        }
        let (bx, by) = self.bend_at(z);
        let r = self.radius - lift;
        let rel = ang - self.roll;
        let (x, y) = (bx + r * rel.sin(), by - r * rel.cos());
        let s = self.focal / z;
        let cy = -self.radius + self.eye;
        Some((self.center + x * s, self.horizon - (y - cy) * s, s))
    }

    /// Whether a point at depth z, drawn at (sx, sy), is hidden behind a
    /// nearer stretch of wall (round a bend).
    pub fn hides(&self, sx: f32, sy: f32, z: f32) -> bool {
        matches!(self.hit(sx, sy), Some((wall, _)) if wall < z * 0.9 - 0.5)
    }
}

/// An angle brought into −π..π.
pub fn wrap(a: f32) -> f32 {
    use std::f32::consts::PI;
    (a + PI).rem_euclid(2.0 * PI) - PI
}

/// Everything a frame is drawn from, back to front.
pub struct Scene<'a> {
    pub palettes: &'a [Palette],
    pub images: &'a [Image],
    pub tiles: &'a Tiles,
    /// A colour for each line, behind everything (raster colour).
    pub backdrop: &'a [u16],
    /// Layers behind the ground (the sky's far scenery), then the ground.
    pub back: &'a [Layer],
    /// Turn the backdrop and back layers clockwise by this angle (radians)
    /// about a screen point: (angle, x, y). For a camera that rolls.
    pub turn: Option<(f32, f32, f32)>,
    pub ground: Option<Ground>,
    /// The tube, in front of the ground and the back layers.
    pub tube: Option<Tube<'a>>,
    pub sprites: &'a [Sprite],
    /// Layers in front of everything (text).
    pub front: &'a [Layer],
    /// Sprites drawn at most (the Super Scaler boards took 128).
    pub max_sprites: Option<usize>,
}

/// A drawn frame: 15-bit colour.
pub struct Frame {
    pub pixels: Vec<u16>,
}

impl Frame {
    pub fn to_rgba(&self, out: &mut [u8]) {
        for (i, &c) in self.pixels.iter().enumerate() {
            let ch = |s: u32| {
                let v = ((c >> s) & 31) as u8;
                (v << 3) | (v >> 2)
            };
            out[i * 4..i * 4 + 4].copy_from_slice(&[ch(10), ch(5), ch(0), 255]);
        }
    }
}

/// A layer's colour at screen point (x, line), if any.
fn layer_at(scene: &Scene, layer: &Layer, x: i32, line: i32) -> Option<u16> {
    let ly = line - layer.top;
    if ly < 0 || ly >= (layer.rows * 8) as i32 {
        return None;
    }
    let (ty, fy) = (ly as usize / 8, ly as usize % 8);
    let scroll = layer.scroll_at(line.clamp(0, HEIGHT as i32 - 1) as usize);
    let lx = (x + scroll).rem_euclid((layer.cols * 8) as i32) as usize;
    let (tile, pal) = layer.map[ty * layer.cols + lx / 8];
    if tile == 0 {
        return None;
    }
    let c = scene.tiles.tiles[tile as usize][fy][lx % 8];
    (c != 0).then(|| scene.palettes[pal as usize][c as usize])
}

/// The backdrop and the back layers turned about a point: each screen pixel
/// looks up where it came from (the Y Board's rotating screen did this).
fn draw_turned_back(scene: &Scene, (angle, cx, cy): (f32, f32, f32), px: &mut [u16]) {
    let (s, c) = angle.sin_cos();
    let backdrop = |line: i32| *scene.backdrop.get(line.clamp(0, HEIGHT as i32 - 1) as usize).unwrap_or(scene.backdrop.last().unwrap_or(&0));
    for line in 0..HEIGHT {
        for x in 0..WIDTH {
            let (dx, dy) = (x as f32 + 0.5 - cx, line as f32 + 0.5 - cy);
            let (sx, sy) = (cx + c * dx + s * dy, cy - s * dx + c * dy);
            let (ix, iy) = (sx.floor() as i32, sy.floor() as i32);
            let mut col = backdrop(iy);
            for layer in scene.back {
                if let Some(v) = layer_at(scene, layer, ix, iy) {
                    col = v;
                }
            }
            px[line * WIDTH + x] = col;
        }
    }
}

fn draw_layer(scene: &Scene, layer: &Layer, px: &mut [u16]) {
    for line in 0..HEIGHT {
        let ly = line as i32 - layer.top;
        if ly < 0 || ly >= (layer.rows * 8) as i32 {
            continue;
        }
        let (ty, fy) = (ly as usize / 8, ly as usize % 8);
        let scroll = layer.scroll_at(line);
        for x in 0..WIDTH {
            let lx = (x as i32 + scroll).rem_euclid((layer.cols * 8) as i32) as usize;
            let (tile, pal) = layer.map[ty * layer.cols + lx / 8];
            if tile == 0 {
                continue;
            }
            let c = scene.tiles.tiles[tile as usize][fy][lx % 8];
            if c != 0 {
                px[line * WIDTH + x] = scene.palettes[pal as usize][c as usize];
            }
        }
    }
}

fn draw_ground(g: &Ground, px: &mut [u16]) {
    let start = (g.horizon.floor() as i32 + 1).max(0) as usize;
    for line in start..HEIGHT {
        let dy = line as f32 + 0.5 - g.horizon;
        if dy <= 0.0 {
            continue;
        }
        let dist = g.height * g.focal / dy;
        let along = ((dist + g.travel) / g.check.1).floor() as i64;
        let fog = (dist / g.fog_far).clamp(0.0, 1.0);
        let c = [mix(g.colors[0], g.fog, fog), mix(g.colors[1], g.fog, fog)];
        for x in 0..WIDTH {
            let across = ((x as f32 + 0.5 - WIDTH as f32 / 2.0) * dist / g.focal + g.shift) / g.check.0;
            let k = (across.floor() as i64 + along) & 1;
            px[line * WIDTH + x] = c[k as usize];
        }
    }
}

fn draw_tube(scene: &Scene, t: &Tube, px: &mut [u16]) {
    let skin = &scene.images[t.skin];
    let (w, h) = (skin.width as f32, skin.height as f32);
    for line in 0..HEIGHT {
        for x in 0..WIDTH {
            let Some((z, ang)) = t.hit(x as f32 + 0.5, line as f32 + 0.5) else { continue };
            let along = z + t.travel;
            let u = ((ang / t.skin_round + 0.5) * w).clamp(0.0, w - 1.0) as usize;
            let v = ((along / t.skin_len).rem_euclid(1.0) * h) as usize % skin.height;
            let c = skin.get(u, v);
            if c == 0 {
                continue;
            }
            let pal = t.marks.iter().find(|m| along >= m.at && along < m.at + m.len).map_or(t.palette, |m| m.palette);
            let col = scene.palettes[pal][c as usize];
            let fog = ((z - t.fog_near) / (t.fog_far - t.fog_near)).clamp(0.0, 1.0);
            px[line * WIDTH + x] = if fog > 0.0 { mix(col, t.fog, fog) } else { col };
        }
    }
}

/// Draw a sprite the Super Scaler's way: for each screen pixel, the source
/// pixel is the one the zoom steps to, whole pixels only.
fn draw_sprite(scene: &Scene, s: &Sprite, px: &mut [u16]) {
    let img = &scene.images[s.image];
    let (w, h) = (img.width as f32 * s.zoom_x, img.height as f32 * s.zoom_y);
    if w < 0.5 || h < 0.5 {
        return;
    }
    let left = s.x - s.anchor_x * w;
    let top = s.y - s.anchor_y * h;
    let (x0, x1) = ((left.floor() as i32).max(0), ((left + w).ceil() as i32).min(WIDTH as i32));
    let (y0, y1) = ((top.floor() as i32).max(0), ((top + h).ceil() as i32).min(HEIGHT as i32));
    let pal = &scene.palettes[s.palette];
    for y in y0..y1 {
        let v = ((y as f32 + 0.5 - top) / s.zoom_y) as i32;
        if v < 0 || v >= img.height as i32 {
            continue;
        }
        for x in x0..x1 {
            let mut u = ((x as f32 + 0.5 - left) / s.zoom_x) as i32;
            if u < 0 || u >= img.width as i32 {
                continue;
            }
            if s.flip_x {
                u = img.width as i32 - 1 - u;
            }
            let c = img.get(u as usize, v as usize);
            if c == 0 {
                continue;
            }
            let i = y as usize * WIDTH + x as usize;
            px[i] = if s.shadow { shade(px[i]) } else { pal[c as usize] };
        }
    }
}

/// Draw one frame.
pub fn render(scene: &Scene) -> Frame {
    let mut px = vec![0u16; WIDTH * HEIGHT];
    if let Some(turn) = scene.turn.filter(|t| t.0.abs() > 1e-4) {
        draw_turned_back(scene, turn, &mut px);
    } else {
        for line in 0..HEIGHT {
            let c = *scene.backdrop.get(line).unwrap_or(scene.backdrop.last().unwrap_or(&0));
            px[line * WIDTH..(line + 1) * WIDTH].fill(c);
        }
        for layer in scene.back {
            draw_layer(scene, layer, &mut px);
        }
    }
    if let Some(g) = &scene.ground {
        draw_ground(g, &mut px);
    }
    if let Some(t) = &scene.tube {
        draw_tube(scene, t, &mut px);
    }
    let mut order: Vec<&Sprite> = scene.sprites.iter().take(scene.max_sprites.unwrap_or(usize::MAX)).collect();
    order.sort_by(|a, b| b.depth.partial_cmp(&a.depth).unwrap_or(std::cmp::Ordering::Equal));
    for s in order {
        draw_sprite(scene, s, &mut px);
    }
    for layer in scene.front {
        draw_layer(scene, layer, &mut px);
    }
    Frame { pixels: px }
}

/// Text: the shared bold 8×8 font as 4-bit tiles, ink in `ink`, a drop
/// shadow in `shadow` (arcade style). Returns each character's tile.
pub fn font_tiles(tiles: &mut Tiles, ink: u8, shadow: u8) -> std::collections::HashMap<char, u16> {
    let mut out = std::collections::HashMap::new();
    for (ch, rows) in nes_ppu::font::FONT {
        let mut t = [[0u8; 8]; 8];
        for (y, row) in rows.iter().enumerate().take(7) {
            for (x, c) in row.chars().enumerate().take(7) {
                if c == '#' {
                    t[y][x] = ink;
                    if t[y + 1][x + 1] == 0 {
                        t[y + 1][x + 1] = shadow;
                    }
                }
            }
        }
        // Ink over shadow where they meet.
        for (y, row) in rows.iter().enumerate().take(7) {
            for (x, c) in row.chars().enumerate().take(7) {
                if c == '#' {
                    t[y][x] = ink;
                }
            }
        }
        out.insert(*ch, tiles.add(t));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene_with<'a>(palettes: &'a [Palette], images: &'a [Image], tiles: &'a Tiles, sprites: &'a [Sprite], backdrop: &'a [u16]) -> Scene<'a> {
        Scene { palettes, images, tiles, backdrop, back: &[], turn: None, ground: None, tube: None, sprites, front: &[], max_sprites: None }
    }

    #[test]
    fn colours_round_trip_15_bit() {
        let c = rgb(31, 16, 0);
        let mut out = [0u8; 4];
        Frame { pixels: vec![c] }.to_rgba(&mut out);
        assert_eq!(out, [255, 132, 0, 255]);
        assert_eq!(shade(rgb(30, 20, 10)), rgb(15, 10, 5));
    }

    #[test]
    fn a_sprite_zooms_by_whole_source_pixels() {
        // A 2×1 image (colours 1, 2) at zoom 3 covers 6×3 pixels: 1,1,1,2,2,2.
        let mut img = Image::new(2, 1);
        img.set(0, 0, 1);
        img.set(1, 0, 2);
        let mut pal = [0u16; 16];
        pal[1] = rgb(31, 0, 0);
        pal[2] = rgb(0, 31, 0);
        let s = [Sprite { image: 0, x: 10.0, y: 10.0, anchor_x: 0.0, anchor_y: 0.0, zoom_x: 3.0, zoom_y: 3.0, palette: 0, flip_x: false, shadow: false, depth: 1.0 }];
        let tiles = Tiles::default();
        let f = render(&scene_with(&[pal], &[img], &tiles, &s, &[0]));
        let row: Vec<u16> = (9..17).map(|x| f.pixels[11 * WIDTH + x]).collect();
        assert_eq!(row, vec![0, pal[1], pal[1], pal[1], pal[2], pal[2], pal[2], 0]);
        assert_eq!(f.pixels[13 * WIDTH + 10], 0, "three lines tall");
    }

    #[test]
    fn shadows_halve_and_far_sprites_draw_first() {
        let mut img = Image::new(1, 1);
        img.set(0, 0, 1);
        let mut pal = [0u16; 16];
        pal[1] = rgb(20, 20, 20);
        let near = Sprite { image: 0, x: 0.0, y: 0.0, anchor_x: 0.0, anchor_y: 0.0, zoom_x: 1.0, zoom_y: 1.0, palette: 0, flip_x: false, shadow: true, depth: 1.0 };
        let far = Sprite { shadow: false, depth: 9.0, ..near };
        let tiles = Tiles::default();
        let f = render(&scene_with(&[pal], &[img], &tiles, &[near, far], &[rgb(31, 31, 31)]));
        assert_eq!(f.pixels[0], rgb(10, 10, 10), "the far sprite, then the near shadow over it");
    }

    fn tube<'a>(bend: &'a [(f32, f32)]) -> Tube<'a> {
        Tube { horizon: 90.0, center: 160.0, focal: 200.0, radius: 4.0, eye: 3.4, rim: std::f32::consts::FRAC_PI_2, travel: 0.0, bend, bend_step: 8.0, skin: 0, skin_round: std::f32::consts::PI, skin_len: 4.0, palette: 0, marks: &[], roll: 0.0, fog: 0, fog_near: 40.0, fog_far: 200.0 }
    }

    #[test]
    fn the_tube_meets_each_ray_where_its_points_project() {
        // A point projected onto the screen is found again by the ray
        // through that pixel: the same depth, the same angle.
        for bend in [vec![], vec![(0.0, 0.0), (0.3, 0.0), (1.2, 0.4), (2.7, 0.9), (4.8, 1.6)]] {
            let t = tube(&bend);
            for (ang, z) in [(0.0f32, 7.0f32), (0.8, 12.0), (-1.2, 20.0), (0.4, 30.0)] {
                let (sx, sy, _) = t.project(ang, 0.0, z).unwrap();
                let (hz, ha) = t.hit(sx, sy).unwrap();
                assert!((hz - z).abs() < z * 0.03, "depth {hz} for {z} (bend {})", bend.len());
                assert!((ha - ang).abs() < 0.05, "angle {ha} for {ang}");
            }
        }
    }

    #[test]
    fn the_floor_is_below_the_horizon_and_the_sky_shows_above_the_rims() {
        let t = tube(&[]);
        let (_, y, _) = t.project(0.0, 0.0, 7.0).unwrap();
        assert!(y > 160.0, "the floor near you is low on the screen: {y}");
        assert!(t.hit(160.0, 200.0).is_some());
        assert!(t.hit(160.0, 20.0).is_none(), "straight up is sky: the pipe is open");
        let (lx, _, _) = t.project(-1.0, 0.0, 7.0).unwrap();
        let (rx, _, _) = t.project(1.0, 0.0, 7.0).unwrap();
        assert!(lx < 160.0 && rx > 160.0);
    }

    #[test]
    fn a_bend_swings_the_far_end_and_hides_what_is_round_it() {
        let bend: Vec<(f32, f32)> = (0..12).map(|i| ((i as f32).powi(2) * 0.35, 0.0)).collect();
        let t = tube(&bend);
        let (near, _, _) = t.project(0.0, 0.0, 7.0).unwrap();
        let (far, _, _) = t.project(0.0, 0.0, 70.0).unwrap();
        assert!(far > near + 60.0, "the tube curves right: {near} then {far}");
        // Far round the bend, the left wall's outside is in front of the floor.
        let (sx, sy, _) = t.project(-1.4, 0.0, 80.0).unwrap();
        let _ = t.hides(sx, sy, 80.0);
    }

    #[test]
    fn a_rolled_camera_keeps_what_it_follows_below_it() {
        let mut t = tube(&[]);
        t.rim = std::f32::consts::PI;
        t.roll = 2.5;
        let (x, y, _) = t.project(2.5, 0.0, 7.0).unwrap();
        assert!((x - 160.0).abs() < 0.5 && y > 160.0, "straight below: {x},{y}");
        let (z, a) = t.hit(x, y).unwrap();
        assert!((z - 7.0).abs() < 0.2 && (a - 2.5).abs() < 0.02);
        // Followed up the right wall and over, the floor is up to the left.
        let (fx, fy, _) = t.project(0.0, 0.0, 7.0).unwrap();
        assert!(fx < 160.0 && fy < 84.0, "{fx},{fy}");
    }

    #[test]
    fn the_ground_alternates_and_fogs() {
        let g = Ground { horizon: 100.0, height: 2.0, focal: 200.0, travel: 0.0, shift: 0.0, check: (2.0, 2.0), colors: [rgb(0, 20, 0), rgb(0, 10, 0)], fog: rgb(31, 31, 31), fog_far: 1000.0 };
        let tiles = Tiles::default();
        let mut s = scene_with(&[], &[], &tiles, &[], &[0]);
        s.ground = Some(g);
        let f = render(&s);
        let line = 200;
        let distinct: std::collections::BTreeSet<u16> = (0..WIDTH).map(|x| f.pixels[line * WIDTH + x]).collect();
        assert_eq!(distinct.len(), 2, "a checkerboard line");
        assert_eq!(f.pixels[50 * WIDTH], 0, "nothing above the horizon");
    }
}
