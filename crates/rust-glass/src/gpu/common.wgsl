// Shared by every pipeline. Coordinates are physical pixels, y down.

struct Globals {
    size: vec2f,        // visible size
    tex_size: vec2f,    // padded texture size
    light: vec2f,       // unit vector pointing towards the light
    time: f32,
    ppp: f32,           // physical pixels per point
    pointer: vec2f,
    pointer_glow: f32,  // strength of the press glow that spills on neighbours
    pad0: f32,
};

struct Mask {
    rect: vec4f,
    radius: vec2f,
    power: f32,
    invert: f32,
    feather: vec4f,     // left, top, right, bottom
};

@group(0) @binding(0) var<uniform> G: Globals;
@group(0) @binding(1) var<storage, read> masks: array<Mask>;
@group(0) @binding(2) var samp: sampler;

// Signed distance and outward normal of a rounded rectangle with continuous
// curvature corners (a superellipse of exponent `n`). x = distance, yz = normal.
fn sd_squircle(p: vec2f, half: vec2f, r_in: vec2f, n: f32) -> vec3f {
    let r = max(min(r_in, half), vec2f(0.001));
    let s = select(vec2f(-1.0), vec2f(1.0), p >= vec2f(0.0));
    let q = abs(p) - (half - r);
    if (q.x > 0.0 && q.y > 0.0) {
        let u = q / r;
        let l = pow(pow(u.x, n) + pow(u.y, n), 1.0 / n);
        let k = pow(l, 1.0 - n);
        let g = vec2f(pow(u.x, n - 1.0), pow(u.y, n - 1.0)) * k / r;
        let gl = max(length(g), 1e-5);
        return vec3f((l - 1.0) / gl, s * g / gl);
    }
    let e = q - r;
    if (e.x > e.y) {
        return vec3f(e.x, s.x, 0.0);
    }
    return vec3f(e.y, 0.0, s.y);
}

fn mask_coverage(index: u32, p: vec2f) -> f32 {
    if (index == 0u) {
        return 1.0;
    }
    let m = masks[index - 1u];
    let c = (m.rect.xy + m.rect.zw) * 0.5;
    let h = (m.rect.zw - m.rect.xy) * 0.5;
    let d = sd_squircle(p - c, h, m.radius, m.power).x;
    var a = clamp(0.5 - d, 0.0, 1.0);
    if (m.invert > 0.5) {
        a = 1.0 - a;
    }
    let f = m.feather;
    if (f.x > 0.0) { a *= smoothstep(0.0, 1.0, (p.x - m.rect.x) / f.x); }
    if (f.y > 0.0) { a *= smoothstep(0.0, 1.0, (p.y - m.rect.y) / f.y); }
    if (f.z > 0.0) { a *= smoothstep(0.0, 1.0, (m.rect.z - p.x) / f.z); }
    if (f.w > 0.0) { a *= smoothstep(0.0, 1.0, (m.rect.w - p.y) / f.w); }
    return a;
}

fn clip_coverage(clip: vec4f, p: vec2f) -> f32 {
    if (p.x < clip.x || p.y < clip.y || p.x > clip.z || p.y > clip.w) {
        return 0.0;
    }
    return 1.0;
}

fn luma(c: vec3f) -> f32 {
    return dot(c, vec3f(0.299, 0.587, 0.114));
}

fn quad_corner(vi: u32) -> vec2f {
    return vec2f(f32(vi & 1u), f32((vi >> 1u) & 1u));
}

fn to_ndc(p: vec2f) -> vec4f {
    let q = p / G.tex_size * 2.0 - 1.0;
    return vec4f(q.x, -q.y, 0.0, 1.0);
}
