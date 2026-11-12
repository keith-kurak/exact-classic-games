// A retro frame on screen: the picture the game's processor drew (256×240 for
// the NES, 320×224 for a System 16 board), scaled up with nearest sampling
// (hard pixels), centred, letterboxed black.
struct Uniforms {
    width: f32,
    height: f32,
    src_width: f32,
    src_height: f32,
}

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var screen: texture_2d<f32>;
@group(0) @binding(2) var screen_sampler: sampler;

struct Fragment {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vs(@builtin(vertex_index) i: u32) -> Fragment {
    var out: Fragment;
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    return out;
}

@fragment
fn fs(f: Fragment) -> @location(0) vec4<f32> {
    let res = vec2<f32>(u.width, u.height);
    let nes = vec2<f32>(u.src_width, u.src_height);
    // The largest whole-number scale that fits, so every NES pixel is the
    // same size; a fractional one when the canvas is smaller than 256×240.
    let fit = min(res.x / nes.x, res.y / nes.y);
    let scale = select(fit, floor(fit), fit >= 1.0);
    let size = nes * scale;
    let origin = floor((res - size) * 0.5);
    let p = (f.position.xy - origin) / scale;
    if (p.x < 0.0 || p.y < 0.0 || p.x >= nes.x || p.y >= nes.y) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    let texel = (floor(p) + 0.5) / nes;
    return textureSampleLevel(screen, screen_sampler, texel, 0.0);
}
