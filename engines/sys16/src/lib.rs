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

/// Everything a frame is drawn from, back to front.
pub struct Scene<'a> {
    pub palettes: &'a [Palette],
    pub images: &'a [Image],
    pub tiles: &'a Tiles,
    /// A colour for each line, behind everything (raster colour).
    pub backdrop: &'a [u16],
    /// Layers behind the ground (the sky's far scenery), then the ground.
    pub back: &'a [Layer],
    pub ground: Option<Ground>,
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
    for line in 0..HEIGHT {
        let c = *scene.backdrop.get(line).unwrap_or(scene.backdrop.last().unwrap_or(&0));
        px[line * WIDTH..(line + 1) * WIDTH].fill(c);
    }
    for layer in scene.back {
        draw_layer(scene, layer, &mut px);
    }
    if let Some(g) = &scene.ground {
        draw_ground(g, &mut px);
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
        Scene { palettes, images, tiles, backdrop, back: &[], ground: None, sprites, front: &[], max_sprites: None }
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
