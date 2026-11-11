//! Exactbound's GPU module: battle backgrounds as WGSL shaders.
//! `aurora` is ported from Caltrain's (exact2 apps/caltrain/gpu).

pub mod aurora;
pub use aurora::AuroraSurface;

use exact_gpu::{Registry, Surface};

/// The shaders under `shaders/`, reflected at build by `build.rs`.
pub mod shaders {
    include!(concat!(env!("OUT_DIR"), "/shaders.rs"));
}

fn aurora() -> Box<dyn Surface> {
    Box::new(AuroraSurface::new())
}

/// Each surface: its name, its number of inputs, its factory.
pub static REGISTRY: Registry = Registry {
    surfaces: &[("aurora", 1, aurora)],
    shaders: shaders::SHADERS,
};

exact_gpu::module!(REGISTRY);
