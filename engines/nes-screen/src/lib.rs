//! A retro picture on an Exact GPU surface: an NES's 256×240 by default, or
//! any size a game's picture processor draws (`NesGame::size`; a System 16
//! board's 320×224). A game implements [`NesGame`]:
//! how its view's inputs bind, and how it draws one 256×240 frame (usually
//! with `nes_ppu::render`). [`NesSurface`] does the rest: it counts frames,
//! uploads the picture and draws it with `shaders/screen.wgsl`, at the largest
//! whole-number scale that fits, letterboxed black.
//!
//! An app using it lists this crate's shaders in its manifest, so the host
//! registers them: `"gpu": { "shaderRoots": ["../../engines/nes-screen/shaders"] }`,
//! and its registry names them: `shaders: nes_screen::shaders::SHADERS`.

use exact_gpu::wgpu;
use exact_gpu::{Frame, FrameUniform, Surface, SurfaceError, Value};
use nes_ppu::{HEIGHT, WIDTH};

/// The shaders under `shaders/`, reflected at build by `build.rs`.
pub mod shaders {
    include!(concat!(env!("OUT_DIR"), "/shaders.rs"));
}

use shaders::screen::{entry, module, Uniforms, GROUP_0, SCREEN, SCREEN_SAMPLER, U};

/// A game's side of the screen.
pub trait NesGame: Default {
    /// The frame's size in pixels: the NES's 256×240 unless the game's
    /// picture processor draws another (a System 16 board's 320×224).
    fn size(&self) -> (usize, usize) {
        (WIDTH, HEIGHT)
    }
    /// Take the view's inputs (positional, typed); refuse with the bad one.
    fn bind(&mut self, inputs: &[Value]) -> Result<(), SurfaceError>;
    /// Draw frame number `frame` (60 a second) into `rgba`, `size()`'s
    /// width × height × 4 bytes.
    fn draw(&mut self, frame: u64, rgba: &mut [u8]);
    /// The same, with the host's clock (`now_ms`) and whether the agent owns
    /// time (`seekable`): for a game that keeps its own pace (an emulator
    /// running 60.0988 frames a second on any display). It draws by default.
    fn draw_at(&mut self, frame: u64, _now_ms: f64, _seekable: bool, rgba: &mut [u8]) {
        self.draw(frame, rgba);
    }
    /// The surface was hidden (`false`) or shown again (`true`), or its
    /// device work interrupted and resumed: a game playing sound pauses it.
    fn shown(&mut self, _shown: bool) {}
}

/// The surface for a game: create one with [`surface`].
pub struct NesSurface<G: NesGame> {
    pub game: G,
    frame: u64,
    rgba: Vec<u8>,
    gpu: Option<Gpu>,
}

impl<G: NesGame> Default for NesSurface<G> {
    fn default() -> Self {
        NesSurface { game: G::default(), frame: 0, rgba: vec![0; WIDTH * HEIGHT * 4], gpu: None }
    }
}

/// A factory for a registry entry: `("ppu", INPUTS, nes_screen::surface::<MyGame>)`.
pub fn surface<G: NesGame + 'static>() -> Box<dyn Surface> {
    Box::new(NesSurface::<G>::default())
}

struct Gpu {
    format: wgpu::TextureFormat,
    generation: u32,
    size: (usize, usize),
    pipeline: wgpu::RenderPipeline,
    uniforms: FrameUniform,
    texture: wgpu::Texture,
    bind_groups: Vec<wgpu::BindGroup>,
}

impl<G: NesGame> Surface for NesSurface<G> {
    fn bind(&mut self, inputs: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
        self.game.bind(inputs)
    }

    fn lifecycle(&mut self, event: exact_gpu::Lifecycle) {
        match event {
            exact_gpu::Lifecycle::Hidden | exact_gpu::Lifecycle::Interrupted => self.game.shown(false),
            exact_gpu::Lifecycle::Visible | exact_gpu::Lifecycle::Resumed => self.game.shown(true),
            _ => {}
        }
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
        let size = self.game.size();
        if self.gpu.as_ref().map(|g| (g.format, g.generation, g.size)) != Some((format, frame.shader_generation, size)) {
            self.gpu = Some(build(device, format, frame.shader_generation, size));
        }
        if self.rgba.len() != size.0 * size.1 * 4 {
            self.rgba = vec![0; size.0 * size.1 * 4];
        }
        self.frame += 1;
        self.game.draw_at(self.frame, frame.now_ms, frame.seekable, &mut self.rgba);
        let gpu = self.gpu.as_mut().unwrap();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &gpu.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(size.0 as u32 * 4), rows_per_image: None },
            wgpu::Extent3d { width: size.0 as u32, height: size.1 as u32, depth_or_array_layers: 1 },
        );
        let (w, h) = frame.pixels();
        let uniforms = Uniforms { width: w as f32, height: h as f32, src_width: size.0 as f32, src_height: size.1 as f32 };
        let slot = gpu.uniforms.write(queue, &uniforms.bytes());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("nes screen"),
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

fn build(device: &wgpu::Device, format: wgpu::TextureFormat, generation: u32, size: (usize, usize)) -> Gpu {
    let shader = device.create_shader_module(module());
    let layout = device.create_bind_group_layout(&GROUP_0);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("retro frame"),
        size: wgpu::Extent3d { width: size.0 as u32, height: size.1 as u32, depth_or_array_layers: 1 },
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
    let uniforms = FrameUniform::new(device, Uniforms::SIZE, "nes screen uniforms");
    let bind_groups = (0..uniforms.slots())
        .map(|slot| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("nes screen"),
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
        label: Some("nes screen"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("nes screen"),
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
    Gpu { format, generation, size, pipeline, uniforms, texture, bind_groups }
}
