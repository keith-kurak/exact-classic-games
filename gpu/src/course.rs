//! The course surface: nine numbers in (mode, camera x/z, aim x/z, ball
//! x/z/height, aim ring distance), one full-screen fragment shader out. It
//! draws again only when an input changes.

use crate::shaders::course::{entry, module, Uniforms, GROUP_0, U};
use exact_gpu::json::number;
use exact_gpu::wgpu;
use exact_gpu::{Frame, FrameUniform, Surface, SurfaceError, Value};

#[derive(Default)]
pub struct CourseSurface {
    inputs: [f32; 9],
    gpu: Option<Gpu>,
}

struct Gpu {
    format: wgpu::TextureFormat,
    generation: u32,
    pipeline: wgpu::RenderPipeline,
    uniforms: FrameUniform,
    bind_groups: Vec<wgpu::BindGroup>,
}

const NAMES: [&str; 9] = ["mode", "camX", "camZ", "aimX", "aimZ", "ballX", "ballZ", "ballY", "ring"];

impl Surface for CourseSurface {
    fn bind(&mut self, inputs: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
        if inputs.len() != 9 {
            return Err(SurfaceError(format!("course: expected 9 inputs, got {}", inputs.len())));
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
        let [mode, cam_x, cam_z, aim_x, aim_z, ball_x, ball_z, ball_y, ring] = self.inputs;
        let uniforms = Uniforms {
            time: frame.now_ms as f32,
            width: w as f32,
            height: h as f32,
            mode,
            cam_x,
            cam_z,
            aim_x,
            aim_z,
            ball_x,
            ball_z,
            ball_y,
            ring,
        };
        let slot = gpu.uniforms.write(queue, &uniforms.bytes());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("course"),
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
    let uniforms = FrameUniform::new(device, Uniforms::SIZE, "course uniforms");
    let bind_groups = (0..uniforms.slots())
        .map(|slot| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("course"),
                layout: &layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: U.binding,
                    resource: uniforms.buffer(slot).as_entire_binding(),
                }],
            })
        })
        .collect();
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("course"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("course"),
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
