// The course: hole 1, a dogleg-right par 4, ray-marched from a fixed camera
// behind the ball (mode 0, mode 2 on the green) or drawn from above (mode 1).
// World units are yards: x east, z north, y up. The tee is at the origin.
// The lies here must match `lieAt` in course.contract.
struct Uniforms {
    time: f32,
    width: f32,
    height: f32,
    mode: f32,
    cam_x: f32,
    cam_z: f32,
    aim_x: f32,
    aim_z: f32,
    ball_x: f32,
    ball_z: f32,
    ball_y: f32,
    ring: f32,
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

const TEE = vec2<f32>(0.0, 0.0);
const CORNER = vec2<f32>(170.0, 130.0);
const GREEN = vec2<f32>(298.0, 132.0);
const GREEN_R = vec2<f32>(15.0, 11.0);
const PIN = vec2<f32>(301.0, 133.0);
const DA = vec2<f32>(0.794, 0.607);
const NA = vec2<f32>(-0.607, 0.794);
const SUN = vec3<f32>(0.42, 0.80, -0.43);
const TREE_CELL = 8.0;
const HOUSE_CELL = 24.0;
const BAND = 11.0;

const OOB = 0;
const TREES = 1;
const ROUGH = 2;
const FAIRWAY = 3;
const SAND = 4;
const PUTTING = 5;
const TEEBOX = 6;

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

fn fbm(p0: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var p = p0;
    for (var i = 0; i < 4; i = i + 1) {
        v = v + a * noise(p);
        p = p * 2.03 + vec2<f32>(1.7, 9.2);
        a = a * 0.5;
    }
    return v;
}

// The squared distance from p to the segment a–b.
fn seg2(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    let d = pa - ba * h;
    return dot(d, d);
}

// Below 1 inside the ellipse at c with radii r.
fn ell(p: vec2<f32>, c: vec2<f32>, r: vec2<f32>) -> f32 {
    let q = (p - c) / r;
    return dot(q, q);
}

fn bunker(p: vec2<f32>) -> f32 {
    var e = ell(p, vec2<f32>(186.0, 152.0), vec2<f32>(8.0, 5.0));
    e = min(e, ell(p, vec2<f32>(226.0, 151.0), vec2<f32>(7.0, 4.5)));
    e = min(e, ell(p, vec2<f32>(296.0, 147.0), vec2<f32>(13.0, 4.0)));
    e = min(e, ell(p, vec2<f32>(289.0, 117.0), vec2<f32>(6.0, 5.0)));
    e = min(e, ell(p, vec2<f32>(303.0, 116.0), vec2<f32>(6.0, 5.0)));
    return e;
}

fn lie(p: vec2<f32>) -> i32 {
    if (ell(p, GREEN, GREEN_R) < 1.0) { return PUTTING; }
    if (bunker(p) < 1.0) { return SAND; }
    if (p.x < -15.0 || p.y < -12.0 || p.x > 340.0 || p.y > 185.0) { return OOB; }
    let a2 = seg2(p, TEE, CORNER);
    let b2 = seg2(p, CORNER, GREEN);
    let along = dot(p, DA);
    let side = dot(p, NA);
    if (a2 < b2) {
        if (side > 38.0) { return OOB; }
        if (side < -32.0) { return TREES; }
    } else {
        let s = p.y - 131.0;
        if (s > 40.0 || s < -26.0 || p.x > 328.0) { return TREES; }
    }
    if (along > -6.0 && along < 10.0 && abs(side) < 7.0) { return TEEBOX; }
    let fw = 20.0 + 3.0 * sin(p.x * 0.07) + 2.0 * sin(p.y * 0.11);
    if (min(a2, b2) < fw * fw && along > 60.0) { return FAIRWAY; }
    return ROUGH;
}

fn height(p: vec2<f32>) -> f32 {
    var h = p.x * 0.012 + p.y * 0.004
        + 0.9 * sin(p.x * 0.043 + 1.0) * sin(p.y * 0.051 + 0.3)
        + 0.35 * sin(p.x * 0.12 + p.y * 0.08);
    let along = dot(p, DA);
    let side = dot(p, NA);
    let tb = max(abs(along - 2.0) / 10.0, abs(side) / 8.0);
    h = mix(h, 1.6, 1.0 - smoothstep(1.0, 1.6, tb));
    let ge = ell(p, GREEN, GREEN_R);
    let gh = 4.6 - (p.x - 298.0) * 0.012 + (p.y - 132.0) * 0.02;
    h = mix(h, gh, 1.0 - smoothstep(1.0, 1.9, ge));
    h = h - 0.7 * (1.0 - smoothstep(0.55, 1.0, bunker(p)));
    // The condos sit up on a bank to the left of the first leg.
    h = h + 2.5 * smoothstep(38.0, 50.0, side) * step(along, 230.0);
    return h;
}

fn houseZone(c: vec2<f32>) -> bool {
    let side = dot(c, NA);
    return side > 46.0 && side < 110.0 && dot(c, DA) < 220.0 && dot(c, DA) > -40.0;
}

fn treeChance(c: vec2<f32>) -> f32 {
    if (dot(c, c) < 900.0) { return 0.0; }
    let l = lie(c);
    if (l == TREES) { return 0.8; }
    if (l == OOB) {
        if (houseZone(c)) { return 0.12; }
        if (dot(c, NA) > 36.0 && dot(c, NA) < 46.0) { return 0.0; }
        return 0.55;
    }
    if (l == ROUGH && min(seg2(c, TEE, CORNER), seg2(c, CORNER, GREEN)) > 900.0) { return 0.15; }
    return 0.0;
}

// The tree in p's cell, if it has one: (distance, id, centre x, centre z).
fn tree(p: vec3<f32>) -> vec4<f32> {
    let cid = floor(p.xz / TREE_CELL);
    let centre = (cid + 0.5) * TREE_CELL
        + (vec2<f32>(hash(cid + 7.1), hash(cid + 3.3)) - 0.5) * 1.2;
    if (!(hash(cid + 0.37) < treeChance(centre))) { return vec4<f32>(1e5, 0.0, 0.0, 0.0); }
    let g = height(centre);
    let r = 2.2 + 0.8 * hash(cid + 1.9);
    let cy = g + 2.2 + r * 1.25;
    let q = p - vec3<f32>(centre.x, cy, centre.y);
    let k = length(q / vec3<f32>(r, r * 1.25, r));
    let canopy = (k - 1.0) * r + 0.22 * sin(q.x * 2.7) * sin(q.y * 2.3) * sin(q.z * 2.9);
    let trunk = max(length(p.xz - centre) - 0.3, max(p.y - cy, g - p.y));
    if (trunk < canopy) { return vec4<f32>(trunk, 3.0, centre); }
    return vec4<f32>(canopy, 2.0, centre);
}

fn house(p: vec3<f32>) -> f32 {
    let cid = floor(p.xz / HOUSE_CELL);
    let centre = (cid + 0.5) * HOUSE_CELL + (vec2<f32>(hash(cid + 2.2), hash(cid + 5.4)) - 0.5) * 3.0;
    if (!houseZone(centre) || hash(cid + 9.0) > 0.8) { return 1e5; }
    let g = height(centre);
    let half = vec3<f32>(7.0 + 2.0 * hash(cid + 4.0), 4.5, 5.5);
    let q = abs(p - vec3<f32>(centre.x, g + 3.5, centre.y)) - half;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

// How far the ray can go in p's cell before it enters the next one.
fn cellExit(p: vec2<f32>, d: vec2<f32>, size: f32) -> f32 {
    let lo = floor(p / size) * size;
    var tx = 1e5;
    var tz = 1e5;
    if (abs(d.x) > 1e-5) { tx = (select(lo.x, lo.x + size, d.x > 0.0) - p.x) / d.x; }
    if (abs(d.y) > 1e-5) { tz = (select(lo.y, lo.y + size, d.y > 0.0) - p.y) / d.y; }
    return min(tx, tz) + 0.02;
}

fn ballPos() -> vec3<f32> {
    let b = vec2<f32>(u.ball_x, u.ball_z);
    return vec3<f32>(b.x, height(b) + 0.06 + u.ball_y, b.y);
}

// The nearest thing: (distance, id). Ids: 1 ground, 2 canopy, 3 trunk,
// 4 house, 5 pole, 6 flag, 7 ball.
fn scene(p: vec3<f32>, t: f32, rd: vec3<f32>) -> vec3<f32> {
    let g = height(p.xz);
    var d = (p.y - g) * 0.45;
    var id = 1.0;
    var cap = 1e5;
    if (p.y - g < BAND) {
        let tr = tree(p);
        if (tr.x < d) { d = tr.x * 0.8; id = tr.y; }
        let h = house(p);
        if (h < d) { d = h; id = 4.0; }
        cap = min(cellExit(p.xz, rd.xz, TREE_CELL), cellExit(p.xz, rd.xz, HOUSE_CELL));
    }
    let thick = max(0.04, t * 0.0014);
    let hp = height(PIN);
    let q = p - vec3<f32>(PIN.x, hp, PIN.y);
    let pole = max(length(q.xz) - thick, max(-q.y, q.y - 2.7));
    if (pole < d) { d = pole; id = 5.0; }
    let wave = 0.08 * sin(q.x * 5.0 + u.time * 0.004);
    let fq = abs(q - vec3<f32>(0.55, 2.4, wave)) - vec3<f32>(0.55, 0.3, thick * 0.6);
    let flag = length(max(fq, vec3<f32>(0.0))) + min(max(fq.x, max(fq.y, fq.z)), 0.0);
    if (flag < d) { d = flag; id = 6.0; }
    let ball = length(p - ballPos()) - max(0.05, t * 0.0035);
    if (ball < d) { d = ball; id = 7.0; }
    return vec3<f32>(d, id, cap);
}

fn sky(rd: vec3<f32>) -> vec3<f32> {
    let e = rd.y;
    var col = mix(vec3<f32>(0.72, 0.84, 0.96), vec3<f32>(0.30, 0.52, 0.90), clamp(e * 2.5, 0.0, 1.0));
    if (e > 0.0) {
        let c = rd.xz / (e + 0.08) * 0.9;
        let n = fbm(c * 1.3 + vec2<f32>(3.0, 1.0));
        let cloud = smoothstep(0.55, 0.78, n);
        col = mix(col, vec3<f32>(0.97, 0.97, 0.99) - 0.18 * (1.0 - fbm(c * 3.0)), cloud * 0.9);
    }
    // A far tree line around the course.
    let az = atan2(rd.z, rd.x);
    let line = 0.025 + 0.03 * fbm(vec2<f32>(az * 9.0, 2.0)) + 0.01 * noise(vec2<f32>(az * 60.0, 1.0));
    if (e < line) {
        let leaf = 0.8 + 0.4 * hash(floor(vec2<f32>(az * 400.0, e * 400.0)));
        col = mix(vec3<f32>(0.20, 0.30, 0.22), vec3<f32>(0.14, 0.27, 0.12) * leaf, smoothstep(0.0, line, line - e + 0.01));
    }
    return col;
}

fn groundColour(p: vec2<f32>, t: f32) -> vec3<f32> {
    let l = lie(p);
    let along = dot(p, DA);
    var col = vec3<f32>(0.25, 0.47, 0.14);
    if (l == FAIRWAY) {
        var stripe = fract(along / 14.0);
        if (seg2(p, CORNER, GREEN) < seg2(p, TEE, CORNER)) { stripe = fract(p.x / 14.0); }
        col = select(vec3<f32>(0.36, 0.62, 0.19), vec3<f32>(0.40, 0.67, 0.22), stripe < 0.5);
    } else if (l == PUTTING) {
        col = vec3<f32>(0.42, 0.72, 0.27);
    } else if (l == TEEBOX) {
        col = vec3<f32>(0.40, 0.68, 0.24);
    } else if (l == SAND) {
        col = vec3<f32>(0.88, 0.84, 0.70);
    } else if (l == OOB) {
        col = vec3<f32>(0.22, 0.40, 0.14);
    } else if (l == TREES) {
        col = vec3<f32>(0.20, 0.36, 0.12);
    }
    // The fringe round the green.
    let ge = ell(p, GREEN, GREEN_R);
    if (ge >= 1.0 && ge < 1.35 && l != SAND) { col = vec3<f32>(0.38, 0.66, 0.22); }
    // The cart path, south of the second leg and behind the tee.
    let s = p.y - 131.0;
    if ((s < -21.0 && s > -24.0 && p.x > 150.0 && p.x < 335.0)
        || (dot(p, NA) > 32.0 && dot(p, NA) < 35.0 && along > -20.0 && along < 200.0)) {
        col = vec3<f32>(0.56, 0.56, 0.54);
    }
    // Grass grain, fading with distance so it does not crawl.
    let grain = hash(floor(p * 6.0)) * 0.6 + hash(floor(p * 2.0)) * 0.4;
    col = col * (1.0 + (grain - 0.5) * 0.2 * clamp(1.0 - t / 260.0, 0.15, 1.0));
    col = col * (0.92 + 0.16 * noise(p * 0.15));
    return col;
}

fn groundNormal(p: vec2<f32>) -> vec3<f32> {
    let e = 0.3;
    let hx = height(p + vec2<f32>(e, 0.0)) - height(p - vec2<f32>(e, 0.0));
    let hz = height(p + vec2<f32>(0.0, e)) - height(p - vec2<f32>(0.0, e));
    return normalize(vec3<f32>(-hx, 2.0 * e, -hz));
}

fn treeShadow(p: vec3<f32>) -> f32 {
    var s = 1.0;
    for (var i = 1; i < 9; i = i + 1) {
        let q = p + SUN * (f32(i) * 1.3);
        let d = tree(q).x;
        s = min(s, clamp(d * 1.5, 0.0, 1.0));
    }
    return 0.55 + 0.45 * s;
}

fn ringTint(p: vec2<f32>, col: vec3<f32>) -> vec3<f32> {
    if (u.ring <= 0.0 || u.mode == 1.0) { return col; }
    let aim = vec2<f32>(u.aim_x, u.aim_z);
    let c = vec2<f32>(u.ball_x, u.ball_z) + aim * u.ring;
    let r = max(1.2, u.ring * 0.05);
    let d = abs(length(p - c) - r);
    let w = max(0.25, r * 0.12);
    return mix(col, vec3<f32>(1.0, 0.92, 0.2), (1.0 - smoothstep(w * 0.5, w, d)) * 0.85);
}

fn shade(ro: vec3<f32>, rd: vec3<f32>, t: f32, id: f32) -> vec3<f32> {
    let p = ro + rd * t;
    var col = vec3<f32>(0.0);
    if (id == 1.0) {
        let n = groundNormal(p.xz);
        col = groundColour(p.xz, t);
        col = ringTint(p.xz, col);
        let lit = 0.72 + 0.4 * max(dot(n, SUN), 0.0);
        col = col * lit * treeShadow(p + n * 0.1);
        let b = ballPos();
        let shadowAt = b.xz + SUN.xz * -(b.y - height(b.xz)) / SUN.y;
        if (length(p.xz - shadowAt) < max(0.06, t * 0.004)) { col = col * 0.45; }
    } else if (id == 2.0) {
        let tr = tree(p);
        let g = height(tr.zw);
        let r = 2.6;
        let q = p - vec3<f32>(tr.z, g + 2.2 + r * 1.25, tr.w);
        let n = normalize(q / vec3<f32>(1.0, 1.5, 1.0));
        let leaf = hash(floor(p.xz * 2.5 + p.y * 3.1));
        let base = mix(vec3<f32>(0.10, 0.25, 0.08), vec3<f32>(0.20, 0.36, 0.12), hash(floor(tr.zw)));
        col = base * (0.45 + 0.75 * max(dot(n, SUN), 0.0)) * (0.8 + 0.4 * leaf);
    } else if (id == 3.0) {
        col = vec3<f32>(0.30, 0.22, 0.14);
    } else if (id == 4.0) {
        let cid = floor(p.xz / HOUSE_CELL);
        let g = height((cid + 0.5) * HOUSE_CELL);
        if (p.y > g + 6.6) {
            col = vec3<f32>(0.30, 0.31, 0.33);
        } else {
            let win = step(0.6, fract(p.x * 0.5)) * step(0.5, fract(p.y * 0.5 + 0.2));
            col = mix(vec3<f32>(0.62, 0.58, 0.50), vec3<f32>(0.20, 0.22, 0.26), win * 0.7);
        }
    } else if (id == 5.0) {
        col = vec3<f32>(0.95, 0.95, 0.92);
    } else if (id == 6.0) {
        col = vec3<f32>(0.90, 0.12, 0.10);
    } else if (id == 7.0) {
        col = vec3<f32>(1.0, 1.0, 1.0);
    }
    let haze = vec3<f32>(0.70, 0.80, 0.88);
    return mix(col, haze, 1.0 - exp(-t * 0.0022));
}

fn view3d(px: vec2<f32>, res: vec2<f32>) -> vec3<f32> {
    let aim = normalize(vec2<f32>(u.aim_x, u.aim_z));
    let putting = u.mode == 2.0;
    let back = select(8.0, 7.0, putting);
    let lift = select(2.0, 1.8, putting);
    let pitch = select(0.06, 0.075, putting);
    let camXZ = vec2<f32>(u.cam_x, u.cam_z) - aim * back;
    let ground = max(height(camXZ), height(vec2<f32>(u.cam_x, u.cam_z)));
    let ro = vec3<f32>(camXZ.x, ground + lift, camXZ.y);
    let fwd = normalize(vec3<f32>(aim.x, -pitch, aim.y));
    let right = normalize(cross(fwd, vec3<f32>(0.0, 1.0, 0.0)));
    let up = cross(right, fwd);
    let uv = (vec2<f32>(px.x, res.y - px.y) - res * 0.5) / res.y;
    let rd = normalize(fwd * 1.55 + right * uv.x + up * uv.y);
    var t = 0.05;
    var id = 0.0;
    for (var i = 0; i < 400; i = i + 1) {
        let p = ro + rd * t;
        let s = scene(p, t, rd);
        if (s.x < 0.0008 * t) { id = s.y; break; }
        t = t + min(max(s.x, 0.0008 * t), s.z);
        if (t > 900.0) { break; }
    }
    if (id == 0.0) { return sky(rd); }
    return shade(ro, rd, t, id);
}

fn overhead(px: vec2<f32>, res: vec2<f32>) -> vec3<f32> {
    let s = max(410.0 / res.x, 250.0 / res.y);
    let w = vec2<f32>(160.0, 80.0) + vec2<f32>(px.x - res.x * 0.5, res.y * 0.5 - px.y) * s;
    var col = groundColour(w, 400.0);
    let n = groundNormal(w);
    col = col * (0.75 + 0.45 * max(dot(n, normalize(vec3<f32>(-0.5, 0.7, 0.5))), 0.0));
    // Tree crowns and condo roofs from above.
    let cid = floor(w / TREE_CELL);
    let centre = (cid + 0.5) * TREE_CELL + (vec2<f32>(hash(cid + 7.1), hash(cid + 3.3)) - 0.5) * 1.2;
    if (hash(cid + 0.37) < treeChance(centre)) {
        let r = 2.2 + 0.8 * hash(cid + 1.9);
        let d = length(w - centre);
        if (d < r) { col = vec3<f32>(0.10, 0.24, 0.08) * (1.1 - 0.5 * d / r); }
    }
    let p3 = vec3<f32>(w.x, height(w) + 3.5, w.y);
    if (house(p3) < 0.0) { col = vec3<f32>(0.32, 0.33, 0.36); }
    let aim = normalize(vec2<f32>(u.aim_x, u.aim_z));
    let b = vec2<f32>(u.ball_x, u.ball_z);
    if (u.ring > 0.0) {
        let line = seg2(w, b, b + aim * u.ring);
        if (line < (s * 1.2) * (s * 1.2)) { col = vec3<f32>(1.0, 0.92, 0.2); }
    }
    if (length(w - PIN) < s * 4.0) { col = vec3<f32>(0.9, 0.12, 0.1); }
    if (length(w - b) < s * 4.0) { col = vec3<f32>(1.0); }
    return col;
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
    // Chunky pixels: about 480 lines, as on a VGA screen.
    let size = max(1.0, floor(res.y / 480.0));
    let px = (floor(f.position.xy / size) + 0.5) * size;
    var col: vec3<f32>;
    if (u.mode == 1.0) {
        col = overhead(px, res);
    } else {
        col = view3d(px, res);
    }
    // A VGA-ish palette: 6 bits a channel, ordered dither.
    let cell = floor(f.position.xy / size);
    col = floor(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0)) * 31.0 + bayer(cell)) / 31.0;
    return vec4<f32>(col, 1.0);
}
