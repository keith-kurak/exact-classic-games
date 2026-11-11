//! Exact Odyssey's GPU module: the `ppu` surface. The view says what is on
//! screen in a few small text specs; this composes the scene from the game's
//! art, has the nes-ppu engine draw the 256×240 frame, and shows it scaled.
//!
//! Inputs, in order:
//! 1. scene: "field" or "battle"
//! 2. scroll x, 3. scroll y: the field camera, in pixels
//! 4. sprites: `name:x:y[:flags]` joined by `;`. Flags: `h` mirrors it, a
//!    digit 0–3 overrides its palette. A name of `#123` draws those digits in colour 3, `!123` in colour 2. A
//!    name ending `*` steps in place: `*` becomes its frame, 0 or 1, from the
//!    frame counter.
//! 5. windows: `x,y,w,h[,spacing]|line|line` joined by `^`, in tiles; a
//!    spacing of 2 puts a blank row between lines. In lines, `~` and a digit
//!    0–8 is a gauge tile filled that many pixels (ATB and stagger gauges),
//!    `~9` a solid block (a charged ATB segment),
//!    and text inside `{` `}` is yellow. An entry `@x,y[,palette]|~n~n…` is a
//!    gauge with no window, on the backdrop: the HP bars under enemies.
//! 6. enemies: `name:tx:ty:fx` joined by `;`. fx 0 is normal, 1 a hit flash,
//!    2–5 a defeat dissolving top-down, a tile row a step, 7 juggled: upside
//!    down (its tiles flipped, rows reversed), 16 px up.
//! 7. fade: 0 (full colour) to 4 (black), the NES palette fade.

pub mod art;

use art::{Art, BATTLE, BATTLE_MAP, FIELD, FIELD_MAP, WINDOW_PALETTE};
use exact_gpu::json::{number, text};
use exact_gpu::wgpu;
use exact_gpu::{Frame, FrameUniform, Registry, Surface, SurfaceError, Value};
use nes_ppu::{render, Cell, Scene, Sprite, FLIP_H, HEIGHT, WIDTH};
use std::sync::OnceLock;

/// The shaders under `shaders/`, reflected at build by `build.rs`.
pub mod shaders {
    include!(concat!(env!("OUT_DIR"), "/shaders.rs"));
}

use shaders::screen::{entry, module, Uniforms, GROUP_0, SCREEN, SCREEN_SAMPLER, U};

pub const INPUTS: usize = 7;
const COLS: usize = 32;
const ROWS: usize = 30;

fn art() -> &'static Art {
    static ART: OnceLock<Art> = OnceLock::new();
    ART.get_or_init(art::build)
}

/// The scene as the view last described it.
#[derive(Default)]
struct Spec {
    battle: bool,
    scroll_x: i32,
    scroll_y: i32,
    sprites: String,
    overlay: Vec<Option<Cell>>,
    stage: Vec<Option<Cell>>,
    fade: u8,
}

/// Frames a step lasts when walking in place.
const STEP_FRAMES: u64 = 12;

fn parse_sprites(spec: &str, frame: u64, out: &mut Vec<Sprite>) {
    let art = art();
    let step = if (frame / STEP_FRAMES) % 2 == 0 { "0" } else { "1" };
    for item in spec.split(';').filter(|s| !s.is_empty()) {
        let mut f = item.split(':');
        let raw = f.next().unwrap_or("");
        let owned = raw.replace('*', step);
        let name = owned.as_str();
        let x: i32 = f.next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0).round() as i32;
        let y: i32 = f.next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0).round() as i32;
        let flags = f.next().unwrap_or("");
        let flip = flags.contains('h');
        let pal = flags.chars().find(|c| c.is_ascii_digit()).map(|c| c as u8 - b'0');
        let digits = if name.starts_with('!') { Some(&art.digits2) } else if name.starts_with('#') { Some(&art.digits) } else { None };
        if let Some(set) = digits {
            for (i, ch) in name[1..].chars().enumerate() {
                if let Some(&tile) = set.get(&ch) {
                    out.push(Sprite { x: x + i as i32 * 8, y, tile, attr: pal.unwrap_or(3) });
                }
            }
        } else if let Some(m) = art.sprites.get(name) {
            m.place(x, y, flip, pal, out);
        }
    }
    let _ = FLIP_H;
}

fn parse_windows(spec: &str) -> Vec<Option<Cell>> {
    let art = art();
    let ui = &art.ui;
    let b = &ui.border;
    let mut cells = vec![None; COLS * ROWS];
    let mut put_in = |x: i32, y: i32, tile: u16, palette: u8| {
        if x >= 0 && y >= 0 && (x as usize) < COLS && (y as usize) < ROWS {
            cells[y as usize * COLS + x as usize] = Some(Cell { tile, palette, flip: 0 });
        }
    };
    for win in spec.split('^').filter(|s| !s.is_empty()) {
        if let Some(label) = win.strip_prefix('@') {
            let mut parts = label.splitn(2, '|');
            let at: Vec<i32> = parts.next().unwrap_or("").split(',').filter_map(|v| v.trim().parse().ok()).collect();
            let (x, y, pal) = match at[..] {
                [x, y] => (x, y, WINDOW_PALETTE),
                [x, y, p] => (x, y, p as u8),
                _ => continue,
            };
            let mut chars = parts.next().unwrap_or("").chars();
            let mut col = 0;
            while let Some(ch) = chars.next() {
                let tile = if ch == '~' {
                    ui.clear_bars[chars.next().and_then(|d| d.to_digit(10)).unwrap_or(0).min(8) as usize]
                } else {
                    0
                };
                put_in(x + col, y, tile, pal);
                col += 1;
            }
            continue;
        }
        let mut parts = win.split('|');
        let rect: Vec<i32> = parts.next().unwrap_or("").split(',').filter_map(|v| v.trim().parse().ok()).collect();
        let (x, y, w, h, gap) = match rect[..] {
            [x, y, w, h] => (x, y, w, h, 1),
            [x, y, w, h, g] => (x, y, w, h, g.max(1)),
            _ => continue,
        };
        for j in 0..h {
            for i in 0..w {
                let tile = match (i, j) {
                    (0, 0) => b.tl,
                    (i, 0) if i == w - 1 => b.tr,
                    (0, j) if j == h - 1 => b.bl,
                    (i, j) if i == w - 1 && j == h - 1 => b.br,
                    (_, 0) => b.top,
                    (_, j) if j == h - 1 => b.bottom,
                    (0, _) => b.left,
                    (i, _) if i == w - 1 => b.right,
                    _ => b.fill,
                };
                put_in(x + i, y + j, tile, WINDOW_PALETTE);
            }
        }
        for (row, line) in parts.enumerate() {
            let ty = y + 1 + row as i32 * gap;
            if ty >= y + h - 1 {
                break;
            }
            let mut chars = line.chars().peekable();
            let mut col = 0;
            let mut gold = false;
            while let Some(ch) = chars.next() {
                if ch == '{' || ch == '}' {
                    gold = ch == '{';
                    continue;
                }
                let tx = x + 1 + col;
                if tx >= x + w - 1 {
                    break;
                }
                let tile = if ch == '~' {
                    let n = chars.next().and_then(|d| d.to_digit(10)).unwrap_or(0).min(9);
                    ui.bars[n as usize]
                } else if gold && ui.yellow.contains_key(&ch) {
                    ui.yellow[&ch]
                } else {
                    *ui.glyphs.get(&ch).or_else(|| ui.glyphs.get(&ch.to_ascii_uppercase())).unwrap_or(&b.fill)
                };
                put_in(tx, ty, tile, WINDOW_PALETTE);
                col += 1;
            }
        }
    }
    cells
}

/// The battle screen's background: rock overhead, enemies in the open.
fn parse_stage(spec: &str) -> Vec<Option<Cell>> {
    let art = art();
    let mut cells = vec![None; COLS * ROWS];
    for (my, row) in BATTLE_MAP.iter().enumerate() {
        for (mx, ch) in row.chars().enumerate() {
            if let Some(m) = art.metatiles.get(&ch) {
                for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                    let (tx, ty) = (mx * 2 + dx, my * 2 + dy);
                    cells[ty * COLS + tx] = Some(m.cell(dx, dy));
                }
            }
        }
    }
    for item in spec.split(';').filter(|s| !s.is_empty()) {
        let f: Vec<&str> = item.split(':').collect();
        let [name, tx, ty, fx] = f[..] else { continue };
        let (Ok(tx), Ok(ty), Ok(fx)) = (tx.parse::<usize>(), ty.parse::<usize>(), fx.parse::<usize>()) else { continue };
        let Some(e) = art.enemies.get(name) else { continue };
        let (w, h) = (e.w, e.h);
        let palette = match (fx, name) {
            (1, _) => WINDOW_PALETTE,
            (_, "drone") => 1,
            _ => 3,
        };
        // Juggled: upside down (each tile flipped, the rows in reverse), two
        // rows higher.
        let juggled = fx == 7;
        let ty = if juggled { ty.saturating_sub(2) } else { ty };
        // Defeated: the enemy's rows clear away from the top, one a step.
        let gone = if (2..=5).contains(&fx) { (fx - 1) * h / 4 } else { 0 };
        for y in gone..h {
            for x in 0..w {
                let (cx, cy) = (tx + x, ty + y);
                if cx < COLS && cy < ROWS {
                    let (src, flip) = if juggled { ((h - 1 - y) * w + x, nes_ppu::FLIP_V) } else { (y * w + x, 0) };
                    cells[cy * COLS + cx] = Some(Cell { tile: e.tiles[src], palette, flip });
                }
            }
        }
    }
    cells
}

/// Draw the described scene as one NES frame, in RGBA.
fn draw(spec: &Spec, frame: u64, rgba: &mut [u8]) {
    let art = art();
    let palettes = if spec.battle { BATTLE } else { FIELD }.faded(spec.fade);
    let field = |tx: i32, ty: i32| -> Cell {
        let (mx, my) = (tx.div_euclid(2), ty.div_euclid(2));
        let ch = FIELD_MAP
            .get(my as usize)
            .filter(|_| my >= 0)
            .and_then(|r| r.chars().nth(mx as usize).filter(|_| mx >= 0))
            .unwrap_or('T');
        let m = match art.frames.get(&ch) {
            Some(frames) => &frames[(frame / 10) as usize % frames.len()],
            None => &art.metatiles[&ch],
        };
        m.cell(tx.rem_euclid(2) as usize, ty.rem_euclid(2) as usize)
    };
    let stage = |tx: i32, ty: i32| -> Cell {
        if tx < 0 || ty < 0 || tx as usize >= COLS || ty as usize >= ROWS {
            return Cell::default();
        }
        spec.stage.get(ty as usize * COLS + tx as usize).copied().flatten().unwrap_or_default()
    };
    let overlay = |x: i32, y: i32| -> Option<Cell> {
        if x < 0 || y < 0 || x as usize >= COLS || y as usize >= ROWS {
            return None;
        }
        spec.overlay[y as usize * COLS + x as usize]
    };
    let mut sprites = Vec::new();
    parse_sprites(&spec.sprites, frame, &mut sprites);
    let out = render(&Scene {
        bg_table: if spec.battle { &art.bg_battle } else { &art.bg },
        spr_table: &art.spr,
        palettes,
        background: if spec.battle { &stage } else { &field },
        overlay: &overlay,
        scroll_x: if spec.battle { 0 } else { spec.scroll_x },
        scroll_y: if spec.battle { 0 } else { spec.scroll_y },
        sprites: &sprites,
        frame,
    });
    out.to_rgba(rgba);
}

/// Write the pattern tables as PNG sprite sheets (bg and sprites) to `dir`.
pub fn write_sheets(dir: &std::path::Path) -> std::io::Result<()> {
    let art = art();
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("bg-field.png"), nes_ppu::png::sheet(&art.bg, FIELD.bg[0], false))?;
    std::fs::write(dir.join("bg-battle.png"), nes_ppu::png::sheet(&art.bg_battle, BATTLE.bg[3], false))?;
    for (i, pal) in art::SPRITES.iter().enumerate() {
        std::fs::write(dir.join(format!("sprites-{i}.png")), nes_ppu::png::sheet(&art.spr, *pal, true))?;
    }
    Ok(())
}

pub struct PpuSurface {
    spec: Spec,
    frame: u64,
    rgba: Vec<u8>,
    gpu: Option<Gpu>,
}

impl Default for PpuSurface {
    fn default() -> Self {
        PpuSurface {
            spec: Spec { overlay: vec![None; COLS * ROWS], stage: vec![None; COLS * ROWS], ..Default::default() },
            frame: 0,
            rgba: vec![0; WIDTH * HEIGHT * 4],
            gpu: None,
        }
    }
}

struct Gpu {
    format: wgpu::TextureFormat,
    generation: u32,
    pipeline: wgpu::RenderPipeline,
    uniforms: FrameUniform,
    texture: wgpu::Texture,
    bind_groups: Vec<wgpu::BindGroup>,
}

impl Surface for PpuSurface {
    fn bind(&mut self, inputs: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
        let [scene, sx, sy, sprites, windows, enemies, fade] = inputs else {
            return Err(SurfaceError(format!("ppu: expected {INPUTS} inputs, got {}", inputs.len())));
        };
        let battle = text(scene, "scene")? == "battle";
        let enemy_spec = text(enemies, "enemies")?;
        self.spec = Spec {
            battle,
            scroll_x: number(sx, "scroll x")?.round() as i32,
            scroll_y: number(sy, "scroll y")?.round() as i32,
            sprites: text(sprites, "sprites")?,
            overlay: parse_windows(&text(windows, "windows")?),
            stage: if battle { parse_stage(&enemy_spec) } else { vec![None; COLS * ROWS] },
            fade: number(fade, "fade")?.clamp(0.0, 4.0) as u8,
        };
        Ok(())
    }

    fn render(
        &mut self,
        frame: &Frame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> bool {
        if self.gpu.as_ref().map(|g| (g.format, g.generation)) != Some((format, frame.shader_generation)) {
            self.gpu = Some(build(device, format, frame.shader_generation));
        }
        self.frame += 1;
        draw(&self.spec, self.frame, &mut self.rgba);
        let gpu = self.gpu.as_mut().unwrap();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &gpu.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(WIDTH as u32 * 4), rows_per_image: None },
            wgpu::Extent3d { width: WIDTH as u32, height: HEIGHT as u32, depth_or_array_layers: 1 },
        );
        let (w, h) = frame.pixels();
        let uniforms = Uniforms { width: w as f32, height: h as f32, pad0: 0.0, pad1: 0.0 };
        let slot = gpu.uniforms.write(queue, &uniforms.bytes());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ppu"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&gpu.pipeline);
            pass.set_bind_group(0, &gpu.bind_groups[slot], &[]);
            pass.draw(0..3, 0..1);
        }
        // The picture processor draws every frame (60 a second), so sprite
        // flicker and timing behave as on the console.
        true
    }
}

fn build(device: &wgpu::Device, format: wgpu::TextureFormat, generation: u32) -> Gpu {
    let shader = device.create_shader_module(module());
    let layout = device.create_bind_group_layout(&GROUP_0);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("nes frame"),
        size: wgpu::Extent3d { width: WIDTH as u32, height: HEIGHT as u32, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("nes pixels"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    let uniforms = FrameUniform::new(device, Uniforms::SIZE, "ppu uniforms");
    let bind_groups = (0..uniforms.slots())
        .map(|slot| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ppu"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: U.binding, resource: uniforms.buffer(slot).as_entire_binding() },
                    wgpu::BindGroupEntry { binding: SCREEN.binding, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: SCREEN_SAMPLER.binding, resource: wgpu::BindingResource::Sampler(&sampler) },
                ],
            })
        })
        .collect();
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("ppu"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("ppu"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState { module: &shader, entry_point: Some(entry::VS), buffers: &[], compilation_options: Default::default() },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some(entry::FS),
            targets: &[Some(format.into())],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    Gpu { format, generation, pipeline, uniforms, texture, bind_groups }
}

fn ppu() -> Box<dyn Surface> {
    Box::new(PpuSurface::default())
}

/// Each surface: its name, its number of inputs, its factory.
pub static REGISTRY: Registry = Registry {
    surfaces: &[("ppu", INPUTS, ppu)],
    shaders: shaders::SHADERS,
};

exact_gpu::module!(REGISTRY);

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes the sprite sheets and two sample frames to assets/ (run with
    /// `cargo test -p exact-odyssey-gpu sheets -- --ignored`).
    #[test]
    #[ignore]
    fn sheets() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/sheets");
        write_sheets(&dir).unwrap();
        let mut rgba = vec![0u8; WIDTH * HEIGHT * 4];
        let mut field = Spec { overlay: parse_windows("1,19,30,10,2|Welcome to Reedholm.|The quick brown fox jumps|over the lazy dog? 0123456789|ATB ~0~2~4~6~8"), stage: vec![None; COLS * ROWS], ..Default::default() };
        field.scroll_x = 16;
        field.scroll_y = 16;
        field.sprites = "down*:120:96;villager*:160:64".into();
        draw(&field, 0, &mut rgba);
        std::fs::write(dir.join("frame-field.png"), nes_ppu::png::encode(256, 240, &rgba)).unwrap();
        let mut battle = Spec { battle: true, stage: parse_stage("drone:2:8:0;hound:12:8:7"), overlay: parse_windows("0,20,10,10|Drone|~8~3~0~0 131||{Hound}|~8~8~6~0 {205}^10,20,9,10,2|Fire|Ice|Flee|<Stance^19,20,13,10|Assault|Rook 120~8~8~8|  {Blast}|Ivy  100~4~0~0|  {Blast}|Kai   90~8~8~2|  {Blast}^@2,16|~8~8~8~8~8~3~0~0^@12,16|~8~8~4~0~0~0~0~0^0,0,14,4|{Hound}  212%|~8~8~8~8~8~8~8~4~0~0~0~0"), ..Default::default() };
        battle.sprites = "idle:200:56;idle:208:88:1;strike:184:120:2;hand:66:168".into();
        draw(&battle, 0, &mut rgba);
        std::fs::write(dir.join("frame-battle.png"), nes_ppu::png::encode(256, 240, &rgba)).unwrap();
    }
}
