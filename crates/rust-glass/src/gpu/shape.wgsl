// Everything that is not glass: fills, strokes, shadows, text, icons, images.

struct Shape {
    rect: vec4f,
    uv: vec4f,          // texture coords, or gradient direction in xy
    color: vec4f,
    color2: vec4f,
    radius: vec2f,
    stroke: f32,
    soft: f32,
    clip: vec4f,
    kind: u32,
    mask: u32,
    power: f32,
    pad: f32,
};

@group(0) @binding(3) var<storage, read> shapes: array<Shape>;
@group(1) @binding(0) var glyphs: texture_2d<f32>;
@group(1) @binding(1) var sprites: texture_2d<f32>;
@group(2) @binding(0) var image: texture_2d<f32>;

const RRECT: u32 = 0u;
const GLYPH: u32 = 1u;
const SPRITE: u32 = 2u;
const IMAGE: u32 = 3u;
const ARC: u32 = 4u;
const LINE: u32 = 5u;
// The inside of glass drawn at half resolution: premultiplied, scaled up.
const HALF: u32 = 6u;

struct VOut {
    @builtin(position) pos: vec4f,
    @location(0) p: vec2f,
    @location(1) t: vec2f,
    @location(2) @interpolate(flat) i: u32,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
    let s = shapes[ii];
    let c = quad_corner(vi);
    var lo = s.rect.xy;
    var hi = s.rect.zw;
    if (s.kind == LINE) {
        let r = s.stroke + 1.0;
        lo = min(s.rect.xy, s.rect.zw) - r;
        hi = max(s.rect.xy, s.rect.zw) + r;
    } else if (s.kind == RRECT || s.kind == ARC) {
        let e = s.soft * 2.5 + 1.0;
        lo -= e;
        hi += e;
    }
    var o: VOut;
    o.p = mix(lo, hi, c);
    o.t = mix(s.uv.xy, s.uv.zw, c);
    o.pos = to_ndc(o.p);
    o.i = ii;
    return o;
}

@fragment
fn fs(in: VOut) -> @location(0) vec4f {
    let s = shapes[in.i];
    let p = in.p;
    var cov = 1.0;
    var color = s.color;
    let center = (s.rect.xy + s.rect.zw) * 0.5;
    let half = (s.rect.zw - s.rect.xy) * 0.5;

    if (s.kind == HALF) {
        let t = textureSampleLevel(image, samp, in.t, 0.0);
        return t * clip_coverage(s.clip, p) * mask_coverage(s.mask, p);
    }
    if (s.kind == GLYPH) {
        cov = textureSampleLevel(glyphs, samp, in.t, 0.0).r;
    } else if (s.kind == SPRITE) {
        let t = textureSampleLevel(sprites, samp, in.t, 0.0);
        color = vec4f(t.rgb / max(t.a, 1e-4), t.a * s.color.a);
    } else if (s.kind == LINE) {
        let a = s.rect.xy;
        let ab = s.rect.zw - a;
        let h = clamp(dot(p - a, ab) / max(dot(ab, ab), 1e-5), 0.0, 1.0);
        let d = length(p - a - ab * h) - s.stroke;
        cov = edge_aa(d);
    } else if (s.kind == ARC) {
        // radius.x = start angle, radius.y = sweep, round caps
        let q = p - center;
        let rr = min(half.x, half.y) - s.stroke;
        var ang = atan2(q.y, q.x) - s.radius.x;
        ang = ang - floor(ang / 6.2831853) * 6.2831853;
        var d: f32;
        if (ang <= s.radius.y) {
            d = abs(length(q) - rr) - s.stroke;
        } else {
            let e0 = vec2f(cos(s.radius.x), sin(s.radius.x)) * rr;
            let e = s.radius.x + s.radius.y;
            let e1 = vec2f(cos(e), sin(e)) * rr;
            d = min(length(q - e0), length(q - e1)) - s.stroke;
        }
        cov = edge_aa(d);
        // The round cap before the start keeps the start color.
        var at = clamp(ang / max(s.radius.y, 1e-3), 0.0, 1.0);
        if (ang > s.radius.y && 6.2831853 - ang < ang - s.radius.y) {
            at = 0.0;
        }
        color = mix(s.color, s.color2, at);
    } else {
        let d = sd_squircle(p - center, half, s.radius, s.power).x;
        if (s.stroke > 0.0) {
            cov = edge_aa(abs(d + s.stroke * 0.5) - s.stroke * 0.5);
        } else if (s.soft > 0.0) {
            cov = 1.0 / (1.0 + exp(d / (s.soft * 0.42)));
        } else {
            cov = edge_aa(d);
        }
        if (s.kind == IMAGE) {
            let texels = abs(s.uv.zw - s.uv.xy) * vec2f(textureDimensions(image));
            let ratio = texels / max(half * 2.0, vec2f(1.0));
            let lod = max(log2(max(max(ratio.x, ratio.y), 0.001)), 0.0);
            let t = textureSampleLevel(image, samp, in.t, lod);
            color = vec4f(t.rgb * s.color.rgb, t.a * s.color.a);
        } else {
            let t = clamp(dot((p - s.rect.xy) / max(half * 2.0, vec2f(1.0)), s.uv.xy), 0.0, 1.0);
            color = mix(s.color, s.color2, t);
        }
    }

    let a = color.a * cov * clip_coverage(s.clip, p) * mask_coverage(s.mask, p);
    return vec4f(color.rgb * a, a);
}
