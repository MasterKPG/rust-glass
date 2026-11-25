// Downsample chain for the backdrop (tent filter), and the final present copy.

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var s: sampler;

struct VOut {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VOut {
    let c = vec2f(f32((vi << 1u) & 2u), f32(vi & 2u));
    var o: VOut;
    o.pos = vec4f(c * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2f(c.x, 1.0 - c.y);
    return o;
}

@fragment
fn fs(in: VOut) -> @location(0) vec4f {
    let px = 1.0 / vec2f(textureDimensions(src));
    var c = textureSampleLevel(src, s, in.uv, 0.0) * 4.0;
    c += textureSampleLevel(src, s, in.uv + px * vec2f(-1.0, -1.0), 0.0);
    c += textureSampleLevel(src, s, in.uv + px * vec2f(1.0, -1.0), 0.0);
    c += textureSampleLevel(src, s, in.uv + px * vec2f(-1.0, 1.0), 0.0);
    c += textureSampleLevel(src, s, in.uv + px * vec2f(1.0, 1.0), 0.0);
    return c / 8.0;
}

// Copy to the swapchain with a dither from 10 to 8 bits.
@fragment
fn present(in: VOut) -> @location(0) vec4f {
    let p = in.pos.xy;
    let c = textureLoad(src, vec2i(p), 0).rgb;
    let n = fract(52.9829189 * fract(dot(p, vec2f(0.06711056, 0.00583715)))) - 0.5;
    return vec4f(c + n / 255.0, 1.0);
}
