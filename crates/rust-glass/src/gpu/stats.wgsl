// Measures what is behind each glass element: mean luminance and contrast.
// The CPU reads the result a frame or two later and adapts tint, shadow and
// label color. One workgroup per element.

struct Probe {
    rect: vec4f,    // pixels
};

struct Stat {
    mean: f32,
    dev: f32,
};

struct Info {
    tex_size: vec2f,
    size: vec2f,
    first: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
};

@group(0) @binding(0) var<storage, read> probes: array<Probe>;
@group(0) @binding(1) var<storage, read_write> stats: array<Stat>;
@group(0) @binding(2) var backdrop: texture_2d<f32>;
@group(0) @binding(3) var samp: sampler;
@group(0) @binding(4) var<uniform> info: Info;

const N: i32 = 8;

@compute @workgroup_size(1)
fn main(@builtin(workgroup_id) id: vec3u) {
    let i = info.first + id.x;
    let r = probes[i].rect;
    let size = max(r.zw - r.xy, vec2f(1.0));
    let lod = clamp(log2(max(size.x, size.y) / f32(N)), 0.0, 4.0);
    var m = 0.0;
    var m2 = 0.0;
    for (var y = 0; y < N; y++) {
        for (var x = 0; x < N; x++) {
            let t = (vec2f(f32(x), f32(y)) + 0.5) / f32(N);
            let p = clamp(mix(r.xy, r.zw, t), vec2f(0.5), info.size - 0.5);
            let c = textureSampleLevel(backdrop, samp, p / info.tex_size, lod).rgb;
            let l = dot(c, vec3f(0.299, 0.587, 0.114));
            m += l;
            m2 += l * l;
        }
    }
    let k = f32(N * N);
    m /= k;
    stats[i] = Stat(m, sqrt(max(m2 / k - m * m, 0.0)));
}
