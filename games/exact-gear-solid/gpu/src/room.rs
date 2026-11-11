//! The room surface: 29 numbers in (the view, the camera and where it looks,
//! the alert, the player, and four soldiers, each a position and a facing),
//! one full-screen fragment shader out. It draws again when an input changes.

use crate::shaders::room::{entry, module, Uniforms, GROUP_0, U};
use exact_gpu::json::number;
use exact_gpu::wgpu;
use exact_gpu::{Frame, FrameUniform, Surface, SurfaceError, Value};

#[derive(Default)]
pub struct RoomSurface {
    inputs: [f32; INPUTS],
    gpu: Option<Gpu>,
}

struct Gpu {
    format: wgpu::TextureFormat,
    generation: u32,
    pipeline: wgpu::RenderPipeline,
    uniforms: FrameUniform,
    bind_groups: Vec<wgpu::BindGroup>,
}

pub const INPUTS: usize = 29;

const NAMES: [&str; INPUTS] = ["mode", "cam_x", "cam_y", "cam_z", "alert", "look_x", "look_y", "look_z", "moving", "px", "pz", "pfx", "pfz", "s0x", "s0z", "s0fx", "s0fz", "s1x", "s1z", "s1fx", "s1fz", "s2x", "s2z", "s2fx", "s2fz", "s3x", "s3z", "s3fx", "s3fz"];

impl Surface for RoomSurface {
    fn bind(&mut self, inputs: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
        if inputs.len() != INPUTS {
            return Err(SurfaceError(format!("room: expected {INPUTS} inputs, got {}", inputs.len())));
        }
        for (i, v) in inputs.iter().enumerate() {
            self.inputs[i] = number(v, NAMES[i])? as f32;
        }
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
        if self.gpu.as_ref().map(|g| (g.format, g.generation))
            != Some((format, frame.shader_generation))
        {
            self.gpu = Some(build(device, format, frame.shader_generation));
        }
        let gpu = self.gpu.as_mut().unwrap();
        let (w, h) = frame.pixels();
        let [mode, cam_x, cam_y, cam_z, alert, look_x, look_y, look_z, moving, px, pz, pfx, pfz, s0x, s0z, s0fx, s0fz, s1x, s1z, s1fx, s1fz, s2x, s2z, s2fx, s2fz, s3x, s3z, s3fx, s3fz] = self.inputs;
        let uniforms = Uniforms {
            time: frame.now_ms as f32,
            width: w as f32,
            height: h as f32,
            mode,
            cam_x,
            cam_y,
            cam_z,
            alert,
            look_x,
            look_y,
            look_z,
            moving,
            px,
            pz,
            pfx,
            pfz,
            s0x,
            s0z,
            s0fx,
            s0fz,
            s1x,
            s1z,
            s1fx,
            s1fz,
            s2x,
            s2z,
            s2fx,
            s2fz,
            s3x,
            s3z,
            s3fx,
            s3fz,
        };
        let slot = gpu.uniforms.write(queue, &uniforms.bytes());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("room"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
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
        false
    }
}

fn build(device: &wgpu::Device, format: wgpu::TextureFormat, generation: u32) -> Gpu {
    let shader = device.create_shader_module(module());
    let layout = device.create_bind_group_layout(&GROUP_0);
    let uniforms = FrameUniform::new(device, Uniforms::SIZE, "room uniforms");
    let bind_groups = (0..uniforms.slots())
        .map(|slot| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("room"),
                layout: &layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: U.binding,
                    resource: uniforms.buffer(slot).as_entire_binding(),
                }],
            })
        })
        .collect();
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("room"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("room"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some(entry::VS),
            buffers: &[],
            compilation_options: Default::default(),
        },
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
    Gpu { format, generation, pipeline, uniforms, bind_groups }
}
