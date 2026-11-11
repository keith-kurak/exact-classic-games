//! Links Exact86's GPU module: the course, ray-marched in WGSL.

pub mod course;
pub use course::CourseSurface;

use exact_gpu::{Registry, Surface};

/// The shaders under `shaders/`, reflected at build by `build.rs`.
pub mod shaders {
    include!(concat!(env!("OUT_DIR"), "/shaders.rs"));
}

fn course() -> Box<dyn Surface> {
    Box::new(CourseSurface::default())
}

/// Each surface: its name, its number of inputs, its factory.
pub static REGISTRY: Registry = Registry {
    surfaces: &[("course", 9, course)],
    shaders: shaders::SHADERS,
};

exact_gpu::module!(REGISTRY);
