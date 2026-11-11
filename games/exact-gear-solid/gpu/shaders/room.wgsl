// The storage room: 27 m × 20 m, walls 4 m high, three rows of four steel
// containers with aisles between them, the exit door in the north wall's
// right end. Ray-marched from a camera the view places: high above the player
// (mode 0) or low beside them when they press against a wall (mode 1).
// Map units are metres: map x is world x, map y is world z, y is up.
// The containers must match `inBlock` in room.contract.
struct Uniforms {
    time: f32,
    width: f32,
    height: f32,
    mode: f32,
    cam_x: f32,
    cam_y: f32,
    cam_z: f32,
    alert: f32,
    look_x: f32,
    look_y: f32,
    look_z: f32,
    moving: f32,
    px: f32,
    pz: f32,
    pfx: f32,
    pfz: f32,
    s0x: f32,
    s0z: f32,
    s0fx: f32,
    s0fz: f32,
    s1x: f32,
    s1z: f32,
    s1fx: f32,
    s1fz: f32,
    s2x: f32,
    s2z: f32,
    s2fx: f32,
    s2fz: f32,
    s3x: f32,
    s3z: f32,
    s3fx: f32,
    s3fz: f32,
    // 0 nothing, 1 cigarettes (smoking), 2 inside the cardboard box.
    item: f32,
}

@group(0) @binding(0) var<uniform> u: Uniforms;

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

const ROOM_W = 27.0;
const ROOM_D = 20.0;
const WALL_H = 4.0;
const BLOCK_H = 3.0;
const LIGHT = vec3<f32>(0.28, 0.9, 0.34);

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453123);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let s = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), s.x),
               mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), s.x), s.y);
}

fn sdBox(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

fn capsule(p: vec3<f32>, a: vec3<f32>, b: vec3<f32>, r: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h) - r;
}

// The walls: four slabs 1 m thick round the room, WALL_H high. From above
// (mode below 0.5), the south slab, nearest the camera, is cut down to a lip so
// it never hides the player. Past the slabs is empty black.
fn walls(p: vec3<f32>) -> f32 {
    let south = select(WALL_H, 0.35, u.mode < 0.5);
    let w = sdBox(p - vec3<f32>(-0.5, WALL_H * 0.5, ROOM_D * 0.5), vec3<f32>(0.5, WALL_H * 0.5, ROOM_D * 0.5 + 1.0));
    let e = sdBox(p - vec3<f32>(ROOM_W + 0.5, WALL_H * 0.5, ROOM_D * 0.5), vec3<f32>(0.5, WALL_H * 0.5, ROOM_D * 0.5 + 1.0));
    let n = sdBox(p - vec3<f32>(ROOM_W * 0.5, WALL_H * 0.5, -0.5), vec3<f32>(ROOM_W * 0.5 + 1.0, WALL_H * 0.5, 0.5));
    let sd = sdBox(p - vec3<f32>(ROOM_W * 0.5, south * 0.5, ROOM_D + 0.5), vec3<f32>(ROOM_W * 0.5 + 1.0, south * 0.5, 0.5));
    return min(min(w, e), min(n, sd));
}

// The container nearest p: column 0–3 at x = 5 + 6c, row 0–2 at z = 4.75 + 5.5r,
// each 4 m × 3.5 m × 3 m.
fn blockCentre(p: vec3<f32>) -> vec3<f32> {
    let c = clamp(round((p.x - 5.0) / 6.0), 0.0, 3.0);
    let r = clamp(round((p.z - 4.75) / 5.5), 0.0, 2.0);
    return vec3<f32>(5.0 + c * 6.0, BLOCK_H * 0.5, 4.75 + r * 5.5);
}

fn blocks(p: vec3<f32>) -> f32 {
    return sdBox(p - blockCentre(p), vec3<f32>(2.0, BLOCK_H * 0.5, 1.75));
}

// A figure at pos facing f, legs swinging by s (-1…1).
fn figure(p: vec3<f32>, pos: vec2<f32>, f: vec2<f32>, s: f32) -> f32 {
    let q = vec2<f32>(p.x - pos.x, p.z - pos.y);
    let lz = dot(q, f);
    let lx = dot(q, vec2<f32>(f.y, -f.x));
    let r = length(q);
    // Far from the figure, a bound that meets its true distance smoothly (the
    // arms reach 0.4 m out), so soft shadows show no rings.
    if (r > 1.5) { return r - 0.42; }
    if (p.y > 2.2) { return max(r - 0.42, p.y - 1.76); }
    let l = vec3<f32>(lx, p.y, lz);
    var d = capsule(l, vec3<f32>(-0.12, 0.1, s * 0.22), vec3<f32>(-0.11, 0.85, 0.0), 0.1);
    d = min(d, capsule(l, vec3<f32>(0.12, 0.1, -s * 0.22), vec3<f32>(0.11, 0.85, 0.0), 0.1));
    d = min(d, capsule(l, vec3<f32>(0.0, 0.95, 0.0), vec3<f32>(0.0, 1.36, 0.02), 0.21));
    d = min(d, length(l - vec3<f32>(0.0, 1.6, 0.04)) - 0.13);
    d = min(d, capsule(l, vec3<f32>(-0.29, 1.36, 0.0), vec3<f32>(-0.32, 0.95, -s * 0.15), 0.075));
    d = min(d, capsule(l, vec3<f32>(0.29, 1.36, 0.0), vec3<f32>(0.32, 0.95, s * 0.15), 0.075));
    return d;
}

// The player in the box: 0.8 m × 0.66 m × 0.74 m, turned to face f.
fn cardboard(p: vec3<f32>) -> f32 {
    let f = vec2<f32>(u.pfx, u.pfz);
    let q = vec2<f32>(p.x - u.px, p.z - u.pz);
    let l = vec3<f32>(dot(q, vec2<f32>(f.y, -f.x)), p.y - 0.37, dot(q, f));
    return sdBox(l, vec3<f32>(0.4, 0.36, 0.32)) - 0.012;
}

// The cigarette, from the lips forward, and its lit tip.
fn mouth() -> vec3<f32> {
    return vec3<f32>(u.px + u.pfx * 0.13, 1.555, u.pz + u.pfz * 0.13);
}
fn cigTip() -> vec3<f32> {
    return mouth() + vec3<f32>(u.pfx * 0.1, -0.02, u.pfz * 0.1);
}
fn cigarette(p: vec3<f32>) -> f32 {
    return capsule(p, mouth(), cigTip(), 0.016);
}

fn swing(seed: f32, on: f32) -> f32 {
    return sin(u.time * 0.009 + seed) * on;
}

fn soldierPos(i: i32) -> vec4<f32> {
    if (i == 0) { return vec4<f32>(u.s0x, u.s0z, u.s0fx, u.s0fz); }
    if (i == 1) { return vec4<f32>(u.s1x, u.s1z, u.s1fx, u.s1fz); }
    if (i == 2) { return vec4<f32>(u.s2x, u.s2z, u.s2fx, u.s2fz); }
    return vec4<f32>(u.s3x, u.s3z, u.s3fx, u.s3fz);
}

// The nearest thing: (distance, id). Ids: 1 floor, 2 wall, 3 container,
// 4 ceiling, 5 the player, 6–9 a soldier.
fn scene(p: vec3<f32>) -> vec2<f32> {
    // The floor stops at the walls' outer faces.
    var d = max(p.y, sdBox(vec3<f32>(p.x - ROOM_W * 0.5, 0.0, p.z - ROOM_D * 0.5), vec3<f32>(ROOM_W * 0.5 + 1.0, 1.0, ROOM_D * 0.5 + 1.0)));
    var id = 1.0;
    let w = walls(p);
    if (w < d) { d = w; id = 2.0; }
    let b = blocks(p);
    if (b < d) { d = b; id = 3.0; }
    if (u.mode > 0.6 && u.cam_y < 3.8) {
        let c = WALL_H - p.y;
        if (c < d) { d = c; id = 4.0; }
    }
    if (u.item == 2.0) {
        let bx = cardboard(p);
        if (bx < d) { d = bx; id = 10.0; }
    } else {
        let pl = figure(p, vec2<f32>(u.px, u.pz), vec2<f32>(u.pfx, u.pfz), swing(0.0, u.moving));
        if (pl < d) { d = pl; id = 5.0; }
        if (u.item == 1.0) {
            let cg = cigarette(p);
            if (cg < d) { d = cg; id = 11.0; }
        }
    }
    for (var i = 0; i < 4; i = i + 1) {
        let s = soldierPos(i);
        let f = figure(p, s.xy, s.zw, swing(f32(i) * 1.7, 1.0));
        if (f < d) { d = f; id = 6.0 + f32(i); }
    }
    return vec2<f32>(d, id);
}

fn normalAt(p: vec3<f32>) -> vec3<f32> {
    let e = vec2<f32>(0.002, -0.002);
    return normalize(e.xyy * scene(p + e.xyy).x + e.yyx * scene(p + e.yyx).x
        + e.yxy * scene(p + e.yxy).x + e.xxx * scene(p + e.xxx).x);
}

// A soft shadow toward the light: the classic penumbra estimate (k = 8) over
// small, even steps, so it is smooth, with no banding, grain or creases.
fn shadow(p: vec3<f32>) -> f32 {
    var s = 1.0;
    var t = 0.04;
    for (var i = 0; i < 64; i = i + 1) {
        let h = scene(p + LIGHT * t).x;
        s = min(s, 8.0 * h / t);
        t = t + clamp(h, 0.02, 0.25);
        if (s < 0.005 || t > 14.0) { break; }
    }
    s = clamp(s, 0.0, 1.0);
    return s * s * (3.0 - 2.0 * s);
}

fn figureColour(p: vec3<f32>, pos: vec2<f32>, f: vec2<f32>, player: bool) -> vec3<f32> {
    let front = dot(vec2<f32>(p.x - pos.x, p.z - pos.y), f);
    if (player) {
        if (p.y > 1.48) {
            // A dark hood, with teal goggles across the front.
            if (front > 0.06 && p.y > 1.58 && p.y < 1.67) { return vec3<f32>(0.25, 0.85, 0.8); }
            return vec3<f32>(0.14, 0.15, 0.18);
        }
        let seam = step(0.9, fract(p.y * 6.0));
        return mix(vec3<f32>(0.20, 0.24, 0.33), vec3<f32>(0.12, 0.14, 0.2), seam);
    }
    if (p.y > 1.5) {
        if (front > 0.08 && p.y < 1.66) { return vec3<f32>(0.62, 0.48, 0.38); }
        return vec3<f32>(0.20, 0.22, 0.20);
    }
    if (p.y < 0.2) { return vec3<f32>(0.08, 0.08, 0.08); }
    if (p.y > 1.12 && p.y < 1.18) { return vec3<f32>(0.1, 0.1, 0.1); }
    return vec3<f32>(0.34, 0.37, 0.32);
}

fn surface(p: vec3<f32>, n: vec3<f32>, id: f32) -> vec3<f32> {
    if (id == 1.0) {
        let g = fract(p.xz);
        let grout = step(g.x, 0.025) + step(g.y, 0.025);
        var c = vec3<f32>(0.33, 0.43, 0.42) * (0.85 + 0.25 * noise(p.xz * 3.0)) * (0.92 + 0.12 * hash(floor(p.xz)));
        c = mix(c, vec3<f32>(0.18, 0.24, 0.24), min(grout, 1.0));
        // A painted box on the floor before the exit.
        if (p.x > 22.5 && p.x < 25.5 && p.z < 2.0 && (p.x < 22.65 || p.x > 25.35 || p.z > 1.85)) {
            c = vec3<f32>(0.75, 0.62, 0.15);
        }
        return c;
    }
    if (id == 2.0) {
        if (n.y > 0.5) { return vec3<f32>(0.10, 0.12, 0.12); }
        var c = vec3<f32>(0.27, 0.32, 0.33) * (0.8 + 0.3 * noise(vec2<f32>(p.x + p.z, p.y) * 2.0));
        let along = select(p.z, p.x, abs(n.z) > 0.5);
        if (fract(along / 2.0) < 0.03) { c = c * 0.6; }
        if (p.y < 0.35) { c = c * 0.55; }
        // The north wall: three cyan strip lights, and the exit door.
        if (n.z > 0.5) {
            if (p.x > 22.5 && p.x < 25.5 && p.y < 2.7) {
                let edge = p.x < 22.75 || p.x > 25.25 || p.y > 2.45;
                let stripe = step(0.5, fract((p.x + p.y) * 2.0));
                if (edge) { return mix(vec3<f32>(0.9, 0.75, 0.1), vec3<f32>(0.08, 0.08, 0.05), stripe); }
                return vec3<f32>(0.32, 0.36, 0.36) * (0.8 + 0.2 * step(0.5, fract(p.y * 1.4)));
            }
            if (p.x > 23.7 && p.x < 24.3 && p.y > 2.85 && p.y < 3.1) { return vec3<f32>(0.2, 1.6, 0.4); }
            for (var k = 0; k < 3; k = k + 1) {
                let y = 2.55 + f32(k) * 0.35;
                if (p.y > y && p.y < y + 0.12 && fract(p.x / 5.0) < 0.75 && p.x < 22.0) {
                    return vec3<f32>(0.75, 1.5, 1.6);
                }
            }
        }
        return c;
    }
    if (id == 3.0) {
        let local = p - blockCentre(p);
        if (n.y > 0.5) {
            // The top: a grate.
            let g = step(0.5, fract(local.x * 2.5)) * step(0.5, fract(local.z * 2.5));
            return mix(vec3<f32>(0.20, 0.23, 0.22), vec3<f32>(0.13, 0.15, 0.15), g);
        }
        var c = vec3<f32>(0.30, 0.34, 0.31) * (0.85 + 0.2 * noise(vec2<f32>(p.x + p.z, p.y) * 4.0));
        let along = select(local.z, local.x, abs(n.z) > 0.5);
        if (fract(p.y / 0.6) < 0.08) { c = c * 0.65; }
        if (fract(along) < 0.04) { c = c * 0.7; }
        let rivet = length(vec2<f32>(fract(along * 2.0) - 0.5, fract(p.y / 0.6 * 2.0) - 0.5));
        if (rivet < 0.12) { c = c * 1.3; }
        return c;
    }
    if (id == 4.0) {
        var c = vec3<f32>(0.08, 0.09, 0.1);
        if (fract(p.x / 3.0) < 0.12 || fract(p.z / 3.0) < 0.12) { c = vec3<f32>(0.15, 0.16, 0.17); }
        return c;
    }
    if (id == 5.0) {
        return figureColour(p, vec2<f32>(u.px, u.pz), vec2<f32>(u.pfx, u.pfz), true);
    }
    if (id == 10.0) {
        // Cardboard: tape along the top, a stencilled band round the sides.
        let f = vec2<f32>(u.pfx, u.pfz);
        let q = vec2<f32>(p.x - u.px, p.z - u.pz);
        let side = dot(q, vec2<f32>(f.y, -f.x));
        var c = vec3<f32>(0.66, 0.51, 0.31) * (0.9 + 0.15 * noise(p.xz * 9.0 + p.y * 5.0));
        if (n.y > 0.5) {
            if (abs(side) < 0.07) { c = vec3<f32>(0.80, 0.70, 0.50); }
            return c;
        }
        if (abs(p.y - 0.42) < 0.07 && abs(fract((side + dot(q, f)) * 2.5) - 0.5) < 0.3) { c = vec3<f32>(0.28, 0.2, 0.12); }
        if (p.y > 0.7) { c = c * 0.8; }
        return c;
    }
    if (id == 11.0) {
        // The lit end glows (brighter than 1: drawn unlit).
        if (length(p - cigTip()) < 0.03) { return vec3<f32>(2.2, 0.75, 0.2); }
        return vec3<f32>(0.92, 0.9, 0.84);
    }
    let s = soldierPos(i32(id - 6.0));
    return figureColour(p, s.xy, s.zw, false);
}

fn render(ro: vec3<f32>, rd: vec3<f32>) -> vec4<f32> {
    var t = 0.0;
    var id = 0.0;
    for (var i = 0; i < 160; i = i + 1) {
        let s = scene(ro + rd * t);
        if (s.x < 0.001 * (1.0 + t)) { id = s.y; break; }
        t = t + s.x;
        if (t > 70.0) { break; }
    }
    if (id == 0.0) { return vec4<f32>(0.0, 0.0, 0.0, 1e5); }
    let p = ro + rd * t;
    let n = normalAt(p);
    var c = surface(p, n, id);
    // Emissive: the strip lights and the exit lamp are brighter than 1.
    if (max(c.r, max(c.g, c.b)) > 1.0) { return vec4<f32>(c * 0.6, t); }
    let sh = shadow(p + n * 0.01);
    let diff = max(dot(n, LIGHT), 0.0);
    // Ambient occlusion from the nearby geometry.
    let ao = clamp(scene(p + n * 0.25).x / 0.25, 0.3, 1.0);
    var lit = c * (0.45 * ao + 0.75 * diff * sh);
    // Cyan spill from the north wall's lights.
    lit = lit + vec3<f32>(0.10, 0.22, 0.24) * exp(-p.z * 0.35) * (0.5 + 0.5 * max(n.z, 0.0));
    // Darkness with distance, as in a dim building.
    lit = lit * exp(-max(t - 8.0, 0.0) * 0.05);
    return vec4<f32>(lit, t);
}

// Smoke from the cigarette: six puffs that rise, drift, grow and thin out,
// each a soft ball laid over whatever is behind it (not over what is in front).
fn smoke(ro: vec3<f32>, rd: vec3<f32>, hit: f32, col: vec3<f32>) -> vec3<f32> {
    let tip = cigTip();
    var c = col;
    for (var i = 0; i < 6; i = i + 1) {
        let fi = f32(i);
        let ph = fract(u.time * 0.00032 + fi / 6.0);
        let drift = vec3<f32>(sin(fi * 2.3 + u.time * 0.0011) * 0.2, 1.1, cos(fi * 1.7 + u.time * 0.0009) * 0.2);
        let centre = tip + drift * ph;
        let r = 0.07 + 0.32 * ph;
        let tc = dot(centre - ro, rd);
        if (tc < 0.0 || tc > hit) { continue; }
        let d = length(ro + rd * tc - centre);
        let a = 0.6 * (1.0 - ph) * smoothstep(0.0, 0.12, ph) * exp(-2.5 * d * d / (r * r));
        c = mix(c, vec3<f32>(0.80, 0.84, 0.84), a);
    }
    return c;
}

fn bayer(p: vec2<f32>) -> f32 {
    let x = u32(p.x) % 4u;
    let y = u32(p.y) % 4u;
    let m = array<f32, 16>(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
    return m[y * 4u + x] / 16.0;
}

@fragment
fn fs(f: Fragment) -> @location(0) vec4<f32> {
    let res = vec2<f32>(u.width, u.height);
    // Chunky pixels: about 360 lines, as on a late-90s console.
    let size = max(1.0, floor(res.y / 360.0));
    let cell = floor(f.position.xy / size);
    let px = (cell + 0.5) * size;
    let ro = vec3<f32>(u.cam_x, u.cam_y, u.cam_z);
    let fwd = normalize(vec3<f32>(u.look_x, u.look_y, u.look_z) - ro);
    let right = normalize(cross(fwd, vec3<f32>(0.0, 1.0, 0.0)));
    let up = cross(right, fwd);
    let uv = (vec2<f32>(px.x, res.y - px.y) - res * 0.5) / res.y;
    let focal = mix(1.35, 1.05, u.mode);
    let rd = normalize(fwd * focal + right * uv.x + up * uv.y);
    let hit = render(ro, rd);
    var col = hit.xyz;
    if (u.item == 1.0) { col = smoke(ro, rd, hit.w, col); }
    // Spotted: the room flushes red.
    col = mix(col, vec3<f32>(col.r * 0.6 + 0.35, col.g * 0.3, col.b * 0.3), u.alert * 0.6);
    let q = (f.position.xy / res) - 0.5;
    col = col * (1.0 - 0.6 * dot(q, q));
    col = floor(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0)) * 31.0 + bayer(cell)) / 31.0;
    return vec4<f32>(col, 1.0);
}
