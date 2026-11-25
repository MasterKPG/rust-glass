// The glass material. One draw covers shadow, lens, face, edge and glow.
//
// Optics: the surface is flat in the middle and curves down near its outline.
// A ray through the curved part is bent towards the middle, so pixels near the
// edge show the backdrop from further inside (compressed, and mirrored right at
// the rim). `inner_*` controls that bend. `outer_*` adds a thin opposite ring
// that pulls in light from outside the shape, `pinch` a wide opposite zone
// further in, as in a drop with a raised shoulder. Red bends more than blue.

struct Glass {
    bounds: vec4f,
    clip: vec4f,
    // x: 0 bends equally in all directions, 1 only sideways. w: tint follow.
    tint: vec4f,
    shape_start: u32,
    shape_count: u32,
    mask: u32,
    fuse: f32,

    inner_amount: f32,
    inner_height: f32,
    outer_amount: f32,
    outer_height: f32,

    chroma: f32,
    blur: f32,          // radius in pixels
    magnify: f32,
    opacity: f32,

    face_lo: f32,       // the backdrop range is remapped to lo..hi
    face_hi: f32,
    saturation: f32,
    light_bend: f32,    // extra rotation of the light, radians

    rim: f32,
    rim_width: f32,
    dark_edge: f32,
    bleed: f32,

    inner_glow: f32,
    inner_shadow: f32,
    lit: f32,           // even brightening, press feedback
    rim_base: f32,      // part of the rim that does not depend on the light

    pinch: f32,         // outward bend in the middle of the curved part
    pinch_width: f32,
    origin: vec2f,      // center of magnification

    shadow: f32,
    shadow_radius: f32,
    shadow_y: f32,
    shadow_tint: f32,
};

struct GShape {
    center: vec2f,
    half: vec2f,
    radius: vec2f,
    power: f32,
    tint_a: f32,
    tint: vec4f,
};

@group(0) @binding(3) var<storage, read> glasses: array<Glass>;
@group(0) @binding(4) var<storage, read> gshapes: array<GShape>;
@group(1) @binding(0) var backdrop: texture_2d<f32>;

struct VOut {
    @builtin(position) pos: vec4f,
    @location(0) p: vec2f,
    @location(1) @interpolate(flat) i: u32,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
    let g = glasses[ii];
    var o: VOut;
    o.p = mix(max(g.bounds.xy, g.clip.xy), min(g.bounds.zw, g.clip.zw), quad_corner(vi));
    o.pos = to_ndc(o.p);
    o.i = ii;
    return o;
}

struct Field {
    d: f32,
    n: vec2f,
    tint: vec4f,
    half: f32,
};

// Union of the group's shapes with a smooth minimum, so nearby shapes merge
// like drops of liquid.
fn field(start: u32, count: u32, k: f32, p: vec2f) -> Field {
    let s0 = gshapes[start];
    let f0 = sd_squircle(p - s0.center, s0.half, s0.radius, s0.power);
    var f: Field;
    f.d = f0.x;
    f.n = f0.yz;
    f.tint = s0.tint;
    f.half = min(s0.half.x, s0.half.y);
    for (var i = 1u; i < count; i++) {
        let s = gshapes[start + i];
        let b = sd_squircle(p - s.center, s.half, s.radius, s.power);
        let kk = max(k, 0.001);
        let t = clamp(0.5 + 0.5 * (b.x - f.d) / kk, 0.0, 1.0);
        f.n = mix(b.yz, f.n, t);
        f.tint = mix(s.tint, f.tint, t);
        f.half = mix(min(s.half.x, s.half.y), f.half, t);
        f.d = mix(b.x, f.d, t) - kk * t * (1.0 - t);
    }
    f.n = f.n / max(length(f.n), 1e-4);
    return f;
}

fn field_d(start: u32, count: u32, k: f32, p: vec2f) -> f32 {
    let s0 = gshapes[start];
    var d = sd_squircle(p - s0.center, s0.half, s0.radius, s0.power).x;
    for (var i = 1u; i < count; i++) {
        let s = gshapes[start + i];
        let b = sd_squircle(p - s.center, s.half, s.radius, s.power).x;
        let kk = max(k, 0.001);
        let t = clamp(0.5 + 0.5 * (b - d) / kk, 0.0, 1.0);
        d = mix(b, d, t) - kk * t * (1.0 - t);
    }
    return d;
}

// Profile of a quarter circle bevel: 1 at the rim, falling steeply to 0 at the
// end of the curved part.
fn bevel(t: f32) -> f32 {
    let u = 1.0 - clamp(t, 0.0, 1.0);
    return 1.0 - sqrt(max(1.0 - u * u, 0.0));
}

// Displacement along the outward normal at `depth` pixels below the outline.
fn lens(g: Glass, depth: f32, side: f32) -> f32 {
    var d = g.outer_amount * bevel(depth / max(g.outer_height, 0.5))
        - g.inner_amount * mix(1.0, side, g.tint.x) * bevel(depth / max(g.inner_height, 0.5));
    if (g.pinch > 0.0) {
        let h = sin(3.14159265 * clamp(depth / max(g.pinch_width, 1.0), 0.0, 1.0));
        d += g.pinch * mix(1.0, 1.0 - side, g.tint.x) * h * h;
    }
    return d;
}

fn gauss(x: f32) -> f32 {
    return exp(-x * x);
}

fn bicubic(uv: vec2f, lod: f32) -> vec3f {
    let dim = vec2f(textureDimensions(backdrop, i32(lod)));
    let x = uv * dim - 0.5;
    let f = fract(x);
    let i = floor(x);
    let w0 = (1.0 - f) * (1.0 - f) * (1.0 - f) / 6.0;
    let w1 = (4.0 - 6.0 * f * f + 3.0 * f * f * f) / 6.0;
    let w3 = f * f * f / 6.0;
    let w2 = 1.0 - w0 - w1 - w3;
    let g0 = w0 + w1;
    let g1 = w2 + w3;
    let h0 = (i - 0.5 + w1 / g0) / dim;
    let h1 = (i + 1.5 + w3 / g1) / dim;
    let a = textureSampleLevel(backdrop, samp, vec2f(h0.x, h0.y), lod).rgb;
    let b = textureSampleLevel(backdrop, samp, vec2f(h1.x, h0.y), lod).rgb;
    let c = textureSampleLevel(backdrop, samp, vec2f(h0.x, h1.y), lod).rgb;
    let d = textureSampleLevel(backdrop, samp, vec2f(h1.x, h1.y), lod).rgb;
    return mix(mix(d, c, g0.x), mix(b, a, g0.x), g0.y);
}

// Backdrop at pixel `p`, blurred with a radius of about `r` pixels.
fn sample_bg(p: vec2f, r: f32) -> vec3f {
    let q = clamp(p, vec2f(0.5), G.size - 0.5);
    let uv = q / G.tex_size;
    if (r < 0.35) {
        return textureSampleLevel(backdrop, samp, uv, 0.0).rgb;
    }
    let lod = clamp(log2(1.0 + r * 0.75), 0.0, 5.0);
    let l0 = floor(lod);
    let a = bicubic(uv, l0);
    if (lod - l0 < 0.02) {
        return a;
    }
    var first = a;
    if (l0 < 0.5) {
        first = textureSampleLevel(backdrop, samp, uv, 0.0).rgb;
    }
    return mix(first, bicubic(uv, min(l0 + 1.0, 5.0)), lod - l0);
}

// Cheaper version for very soft lookups: one level only.
fn sample_wide(p: vec2f, r: f32) -> vec3f {
    let q = clamp(p, vec2f(0.5), G.size - 0.5);
    return bicubic(q / G.tex_size, clamp(round(log2(1.0 + r * 0.75)), 0.0, 5.0));
}

fn vibrance(c: vec3f, s: f32) -> vec3f {
    return max(mix(vec3f(luma(c)), c, s), vec3f(0.0));
}

@fragment
fn fs(in: VOut) -> @location(0) vec4f {
    let g = glasses[in.i];
    let p = in.p;
    let f = field(g.shape_start, g.shape_count, g.fuse, p);
    let d = f.d;
    let n = f.n;
    let cov = edge_aa(d);

    // Shadow, under the glass and tinted by the light that passed through it.
    var under = vec4f(0.0);
    if (cov < 1.0 && g.shadow > 0.001) {
        let ds = field_d(g.shape_start, g.shape_count, g.fuse, p - vec2f(0.0, g.shadow_y));
        let soft = 1.0 / (1.0 + exp(ds / (g.shadow_radius * 0.38)));
        let contact = exp(-max(d, 0.0) / (1.2 * G.ppp)) * min(g.dark_edge, 0.3) * 0.30;
        let a = clamp(g.shadow * soft + contact * g.opacity, 0.0, 1.0);
        var rgb = vec3f(0.0);
        if (g.shadow_tint > 0.0 && a > 0.004) {
            // Light that crossed the body: from the middle, widely spread.
            let q = p - n * (max(d, 0.0) + f.half * 0.95);
            let spread_px = max(f.half * 1.6, 10.0 * G.ppp);
            let bg = vibrance(sample_wide(mix(q, g.origin, 0.55), spread_px), 2.4);
            rgb = min(bg * bg * 1.05, vec3f(1.0)) * g.shadow_tint;
        }
        under = vec4f(rgb * a, a);
    }

    var out = under;
    if (cov > 0.0) {
        let depth = max(-d, 0.0);
        let op = g.opacity;

        // Virtual light in element space.
        let cb = cos(g.light_bend);
        let sb = sin(g.light_bend);
        let light = vec2f(G.light.x * cb - G.light.y * sb, G.light.x * sb + G.light.y * cb);
        let facing = dot(n, light);
        let near = max(facing, 0.0);
        let far = max(-facing, 0.0);

        // Lens. Dispersion flips on the far side, where light leaves the body.
        let chroma = g.chroma * mix(1.0, -0.55, smoothstep(0.0, 0.5, far));
        let base = g.origin + (p - g.origin) / max(g.magnify, 0.05);
        let side = n.x * n.x * n.x * n.x;
        let shift = lens(g, depth, side);
        // How many backdrop pixels one screen pixel covers along the normal.
        // Near the rim this grows quickly, so those pixels are supersampled.
        let foot = abs(1.0 - lens(g, depth + 0.5, side) + lens(g, max(depth - 0.5, 0.0), side));
        // The rim of real glass stays clearer than its body.
        var blur = g.blur * mix(0.45, 1.0, smoothstep(0.0, max(g.inner_height, 1.0), depth));

        var c: vec3f;
        let spread = abs(shift * chroma);
        if (blur < 3.0 && (spread > 0.35 || foot > 1.3)) {
            var sum = vec3f(0.0);
            var wsum = vec3f(0.0);
            let rr = max(select(blur, 0.0, blur < 1.2), min(foot * 0.09, 2.0));
            for (var i = 0; i < 8; i++) {
                let t = (f32(i) + 0.5) / 8.0;
                // Wavelength and sub pixel depth run through different orders.
                let sub = (f32((i * 5 + 2) % 8) + 0.5) / 8.0 - 0.5;
                let w = max(vec3f(1.0) - abs(vec3f(t) - vec3f(0.0, 0.5, 1.0)) * 1.9, vec3f(0.0));
                let k = lens(g, max(depth + sub, 0.0), side) * (1.0 + chroma * (1.0 - 2.0 * t)) - sub;
                sum += sample_bg(base + n * k, rr) * w;
                wsum += w;
            }
            c = sum / wsum;
        } else {
            blur = max(blur, min((foot - 1.0) * 0.4, 6.0));
            if (spread > 0.35) {
                c.r = sample_bg(base + n * shift * (1.0 + chroma), blur).r;
                c.g = sample_bg(base + n * shift, blur).g;
                c.b = sample_bg(base + n * shift * (1.0 - chroma), blur).b;
            } else {
                c = sample_bg(base + n * shift, blur);
            }
        }

        // Light from the wider surroundings spills onto the edge.
        if (g.bleed > 0.0) {
            let e = g.bleed * exp(-depth / max(g.inner_height * 0.8, 1.0));
            if (e > 0.008) {
                let q = p + n * (g.inner_height * 0.8 + 6.0 * G.ppp);
                c = mix(c, vibrance(sample_wide(q, 14.0 * G.ppp), 1.5), e);
            }
        }

        // Face: vibrancy, range compression, tint.
        c = vibrance(c, mix(1.0, g.saturation, op));
        let lum = luma(c);
        let lo = mix(0.0, g.face_lo, op);
        let hi = mix(1.0, g.face_hi, op);
        c = vec3f(lo) + c * (hi - lo);

        let ta = f.tint.a * op;
        if (ta > 0.0) {
            // Colored glass keeps its hue but follows the brightness behind it.
            let toned = f.tint.rgb * (0.86 + 0.26 * lum) + vec3f(0.05 * lum);
            c = mix(c, mix(f.tint.rgb, toned, g.tint.a), ta);
        }

        // Directional light. The near side gets a soft shade, the far side the
        // light that was focused through the body.
        let body = max(min(f.half * 0.95, 34.0 * G.ppp), 1.0);
        let e = 1.0 - smoothstep(0.0, body, depth);
        c = c * (1.0 - g.inner_shadow * e * e * near * op);
        c = mix(c, vec3f(1.0), g.inner_glow * e * e * far * op);

        // Dark line just inside the outline, then the specular rim on top. Both
        // are soft profiles of the signed distance, so they stay even around
        // corners. The rim is where rays turn the most, so the dark line takes
        // the color of the whole body behind it.
        let sd = -d;
        let edge = gauss(max(sd - 0.18 * G.ppp, 0.0) / max(0.40 * G.ppp, 0.7));
        if (edge > 0.01 && g.dark_edge > 0.0) {
            let q = mix(p - n * f.half, g.origin, 0.8);
            let body_c = vibrance(sample_wide(q, max(f.half * 0.85, 6.0 * G.ppp)), 10.0);
            let k = g.dark_edge * edge * op * (1.0 - 0.92 * smoothstep(0.1, 0.8, far));
            c = mix(c, body_c * 0.5, k);
        }
        let rw = max(g.rim_width, 0.9);
        let line = gauss((sd - rw * 0.55) / (rw * 0.68));
        let halo = exp(-depth / max(2.0 * G.ppp, 0.8)) * min(g.inner_glow * 2.4, 0.6) * smoothstep(0.0, 0.5, far);
        let dir = (1.0 - 0.9 * g.dark_edge) * pow(near, 1.6) + pow(far, 1.2);
        let key = g.rim_base + (1.0 - g.rim_base) * min(dir, 1.0);
        let rim_rgb = mix(vec3f(1.0), min(vibrance(c, 1.4) * 1.5 + 0.15, vec3f(1.0)), 0.25);
        let rim_a = clamp(g.rim * (key * line + dir * halo), 0.0, 1.0) * op;
        c = mix(c, rim_rgb, rim_a);

        c = mix(c, vec3f(1.0), g.lit * op);

        let fade = smoothstep(0.0, 0.25, op);
        let a = cov * fade;
        out = vec4f(clamp(c, vec3f(0.0), vec3f(1.0)) * a, a) + under * (1.0 - a);
    }

    return out * mask_coverage(g.mask, p) * clip_coverage(g.clip, p);
}
