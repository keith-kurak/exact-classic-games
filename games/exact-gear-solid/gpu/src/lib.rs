//! Exact Gear Solid's GPU module: the storage room, ray-marched in WGSL.

pub mod room;
pub use room::RoomSurface;

use exact_gpu::{Registry, Surface};

/// The shaders under `shaders/`, reflected at build by `build.rs`.
pub mod shaders {
    include!(concat!(env!("OUT_DIR"), "/shaders.rs"));
}

fn room() -> Box<dyn Surface> {
    Box::new(RoomSurface::default())
}

/// Each surface: its name, its number of inputs, its factory.
pub static REGISTRY: Registry = Registry {
    surfaces: &[("room", room::INPUTS, room)],
    shaders: shaders::SHADERS,
};

exact_gpu::module!(REGISTRY);
