//! wgpu renderer.
//!
//! A frame is split into passes. Before each pass that contains glass, the
//! region behind that glass is copied from the scene and blurred into a mip
//! chain, which the glass samples. Glass can therefore refract everything below
//! it, including other glass. Commands that do not overlap are free to move to
//! an earlier pass, so a window full of controls needs only a few.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use bytemuck::{Pod, Zeroable};

use crate::paint::{Cmd, DrawData, GShapeInst, GlassInst, ImageId, MaskInst, ShapeInst};
use crate::text::{ATLAS_SIZE, AtlasUpload, SPRITE_ATLAS_SIZE};

const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgb10a2Unorm;
const MIPS: u32 = 6;
const PAD: u32 = 32;
const MAX_PROBES: u64 = 512;
const INFO_STRIDE: u64 = 256;
const MAX_PASSES: u64 = 64;

/// Device and queue, shared by all windows.
pub struct Gpu {
    // Kept for as long as the device and its surfaces live.
    #[allow(dead_code)]
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Gpu {
    pub fn instance() -> wgpu::Instance {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::PRIMARY;
        if cfg!(not(debug_assertions)) {
            desc.flags = wgpu::InstanceFlags::empty();
        }
        wgpu::Instance::new(desc.with_env())
    }

    pub fn new(
        instance: wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
        low_power: bool,
    ) -> Result<Self, String> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: if low_power {
                wgpu::PowerPreference::LowPower
            } else {
                wgpu::PowerPreference::HighPerformance
            },
            compatible_surface: surface,
            ..Default::default()
        }))
        .map_err(|e| format!("no graphics adapter: {e}"))?;
        let features = adapter.features() & wgpu::Features::TIMESTAMP_QUERY;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("rust-glass"),
            required_features: features,
            required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            ..Default::default()
        }))
        .map_err(|e| format!("no graphics device: {e}"))?;
        Ok(Self { instance, adapter, device, queue })
    }

    pub fn describe(&self) -> String {
        let i = self.adapter.get_info();
        format!("{} ({:?})", i.name, i.backend)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    size: [f32; 2],
    tex_size: [f32; 2],
    light: [f32; 2],
    time: f32,
    ppp: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct StatInfo {
    tex_size: [f32; 2],
    size: [f32; 2],
    first: u32,
    pad: [u32; 3],
}

#[derive(Clone, Copy, Debug)]
struct Px {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

impl Px {
    const NONE: Px = Px { x0: 1e9, y0: 1e9, x1: -1e9, y1: -1e9 };
    fn new(a: [f32; 4]) -> Self {
        Self { x0: a[0], y0: a[1], x1: a[2], y1: a[3] }
    }
    fn union(&self, o: &Px) -> Px {
        Px { x0: self.x0.min(o.x0), y0: self.y0.min(o.y0), x1: self.x1.max(o.x1), y1: self.y1.max(o.y1) }
    }
    fn intersect(&self, o: &Px) -> Px {
        Px { x0: self.x0.max(o.x0), y0: self.y0.max(o.y0), x1: self.x1.min(o.x1), y1: self.y1.min(o.y1) }
    }
    fn grow(&self, v: f32) -> Px {
        Px { x0: self.x0 - v, y0: self.y0 - v, x1: self.x1 + v, y1: self.y1 + v }
    }
    fn empty(&self) -> bool {
        self.x1 <= self.x0 || self.y1 <= self.y0
    }
}

#[derive(Clone, Copy, Debug)]
enum Item {
    Shapes { start: u32, end: u32, image: ImageId },
    Glass(u32),
}

#[derive(Default)]
struct Pass {
    items: Vec<Item>,
    capture: Option<Px>,
    max_lod: u32,
    probes: std::ops::Range<u32>,
}

/// A pass while it is being collected.
#[derive(Default)]
struct Work {
    pass: Pass,
    shapes: Vec<ShapeInst>,
    probes: Vec<u32>,
}

#[derive(Default)]
struct Plan {
    passes: Vec<Pass>,
    /// Shapes reordered by pass.
    shapes: Vec<ShapeInst>,
    /// Indices into the probe list, reordered by pass.
    probes: Vec<u32>,
}

const TILE: f32 = 8.0;

/// Highest pass that has drawn into each tile (plus one), and the pass whose
/// shadow covers it.
#[derive(Default)]
struct Grid {
    w: usize,
    h: usize,
    cells: Vec<[u8; 2]>,
}

impl Grid {
    fn reset(&mut self, screen: Px) {
        self.w = (screen.x1 / TILE).ceil() as usize;
        self.h = (screen.y1 / TILE).ceil() as usize;
        self.cells.clear();
        self.cells.resize(self.w * self.h, [0; 2]);
    }
    fn span(&self, r: &Px) -> (usize, usize, usize, usize) {
        let f = |v: f32, n: usize| ((v / TILE).max(0.0) as usize).min(n);
        let c = |v: f32, n: usize| ((v / TILE).ceil().max(0.0) as usize).min(n);
        (f(r.x0, self.w), f(r.y0, self.h), c(r.x1, self.w), c(r.y1, self.h))
    }
    fn query(&self, r: &Px) -> (u8, u8) {
        let (x0, y0, x1, y1) = self.span(r);
        let mut m = (0, 0);
        for y in y0..y1 {
            for c in &self.cells[y * self.w + x0..y * self.w + x1] {
                m = (m.0.max(c[0]), m.1.max(c[1]));
            }
        }
        m
    }
    fn mark(&mut self, r: &Px, v: u8) {
        self.each(r, |c| c[0] = c[0].max(v));
    }
    fn shade(&mut self, r: &Px, v: u8) {
        if !r.empty() {
            self.each(r, |c| c[1] = c[1].max(v));
        }
    }
    fn each(&mut self, r: &Px, f: impl Fn(&mut [u8; 2])) {
        let (x0, y0, x1, y1) = self.span(r);
        for y in y0..y1 {
            for c in &mut self.cells[y * self.w + x0..y * self.w + x1] {
                f(c);
            }
        }
    }
}

/// What the renderer did in the last frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameStats {
    /// Render passes. Glass needs a pass to read what is below it.
    pub passes: u32,
    /// Pixels copied for glass to read, in total.
    pub captured_px: u64,
    /// Shapes, glyphs and images drawn.
    pub shapes: u32,
    /// Glass elements drawn.
    pub glasses: u32,
    /// GPU time in milliseconds, smoothed. `None` where the GPU cannot
    /// measure time, such as some virtual machines.
    pub gpu_ms: Option<f32>,
}

struct Readback {
    buffer: wgpu::Buffer,
    keys: Vec<u64>,
    /// 0 free, 1 pending, 2 ready, 3 failed
    state: Arc<AtomicU8>,
}

struct Timing {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    state: Arc<AtomicU8>,
    period: f32,
}

struct Targets {
    size: (u32, u32),
    tex: (u32, u32),
    scene: wgpu::Texture,
    scene_view: wgpu::TextureView,
    backdrop: wgpu::Texture,
    mip_views: Vec<wgpu::TextureView>,
    mip_groups: Vec<wgpu::BindGroup>,
    present_group: wgpu::BindGroup,
    glass_group: wgpu::BindGroup,
    stats_group: wgpu::BindGroup,
}

struct Growable {
    buffer: wgpu::Buffer,
    label: &'static str,
}

impl Growable {
    fn new(device: &wgpu::Device, label: &'static str, size: u64) -> Self {
        Self { buffer: Self::make(device, label, size), label }
    }
    fn make(device: &wgpu::Device, label: &str, size: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
    /// Returns true when the buffer was reallocated.
    fn write<T: Pod>(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[T]) -> bool {
        if data.is_empty() {
            return false;
        }
        let bytes: &[u8] = bytemuck::cast_slice(data);
        let grown = bytes.len() as u64 > self.buffer.size();
        if grown {
            self.buffer = Self::make(device, self.label, (bytes.len() as u64).next_power_of_two());
        }
        queue.write_buffer(&self.buffer, 0, bytes);
        grown
    }
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    shape_pipe: wgpu::RenderPipeline,
    glass_pipe: wgpu::RenderPipeline,
    blur_pipe: wgpu::RenderPipeline,
    present_pipe: wgpu::RenderPipeline,
    stats_pipe: wgpu::ComputePipeline,
    shape_layout: wgpu::BindGroupLayout,
    glass_layout: wgpu::BindGroupLayout,
    backdrop_layout: wgpu::BindGroupLayout,
    blur_layout: wgpu::BindGroupLayout,
    image_layout: wgpu::BindGroupLayout,
    stats_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    globals: wgpu::Buffer,
    shapes: Growable,
    glasses: Growable,
    gshapes: Growable,
    masks: Growable,
    shape_group: wgpu::BindGroup,
    glass_group: wgpu::BindGroup,
    atlas: wgpu::Texture,
    sprites: wgpu::Texture,
    atlas_group: wgpu::BindGroup,
    images: HashMap<u32, (wgpu::Texture, wgpu::BindGroup)>,
    next_image: u32,
    targets: Option<Targets>,
    probes: wgpu::Buffer,
    stats: wgpu::Buffer,
    infos: wgpu::Buffer,
    readbacks: Vec<Readback>,
    timing: Option<Timing>,
    grid: Grid,
    pub frame_stats: FrameStats,
}

impl Renderer {
    pub fn new(gpu: &Gpu, output: wgpu::TextureFormat) -> Self {
        let device = gpu.device.clone();
        let queue = gpu.queue.clone();

        let common = include_str!("common.wgsl");
        let module = |label: &str, src: String| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            })
        };
        let shape_mod = module("shape", format!("{common}\n{}", include_str!("shape.wgsl")));
        let glass_mod = module("glass", format!("{common}\n{}", include_str!("glass.wgsl")));
        let blur_mod = module("blur", include_str!("blur.wgsl").into());
        let stats_mod = module("stats", include_str!("stats.wgsl").into());

        let vf = wgpu::ShaderStages::VERTEX_FRAGMENT;
        let fr = wgpu::ShaderStages::FRAGMENT;
        let cs = wgpu::ShaderStages::COMPUTE;
        let uniform = |binding, vis, dynamic| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: vis,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: dynamic,
                min_binding_size: None,
            },
            count: None,
        };
        let storage = |binding, vis, read_only| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: vis,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let texture = |binding, vis| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: vis,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let sampler_entry = |binding, vis| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: vis,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };
        let layout = |label: &str, entries: &[wgpu::BindGroupLayoutEntry]| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some(label), entries })
        };
        let shape_layout = layout(
            "shape",
            &[uniform(0, vf, false), storage(1, vf, true), sampler_entry(2, fr), storage(3, vf, true)],
        );
        let glass_layout = layout(
            "glass",
            &[
                uniform(0, vf, false),
                storage(1, vf, true),
                sampler_entry(2, fr),
                storage(3, vf, true),
                storage(4, vf, true),
            ],
        );
        let atlas_layout = layout("atlas", &[texture(0, fr), texture(1, fr)]);
        let image_layout = layout("image", &[texture(0, fr)]);
        let backdrop_layout = layout("backdrop", &[texture(0, fr)]);
        let blur_layout = layout("blur", &[texture(0, fr), sampler_entry(1, fr)]);
        let stats_layout = layout(
            "stats",
            &[
                storage(0, cs, true),
                storage(1, cs, false),
                texture(2, cs),
                sampler_entry(3, cs),
                uniform(4, cs, true),
            ],
        );

        let pipe_layout = |groups: &[Option<&wgpu::BindGroupLayout>]| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: groups,
                immediate_size: 0,
            })
        };
        let premultiplied = wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING;
        let pipe = |label: &str,
                    layout: &wgpu::PipelineLayout,
                    m: &wgpu::ShaderModule,
                    fs: &str,
                    topo,
                    format,
                    blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: m,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState { topology: topo, ..Default::default() },
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: m,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::COLOR,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        use wgpu::PrimitiveTopology::{TriangleList, TriangleStrip};
        let shape_pipe = pipe(
            "shape",
            &pipe_layout(&[Some(&shape_layout), Some(&atlas_layout), Some(&image_layout)]),
            &shape_mod,
            "fs",
            TriangleStrip,
            SCENE_FORMAT,
            Some(premultiplied),
        );
        let glass_pipe = pipe(
            "glass",
            &pipe_layout(&[Some(&glass_layout), Some(&backdrop_layout)]),
            &glass_mod,
            "fs",
            TriangleStrip,
            SCENE_FORMAT,
            Some(premultiplied),
        );
        let blur_pl = pipe_layout(&[Some(&blur_layout)]);
        let blur_pipe = pipe("blur", &blur_pl, &blur_mod, "fs", TriangleList, SCENE_FORMAT, None);
        let present_pipe = pipe("present", &blur_pl, &blur_mod, "present", TriangleList, output, None);
        let stats_pipe = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("stats"),
            layout: Some(&pipe_layout(&[Some(&stats_layout)])),
            module: &stats_mod,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });

        let buffer = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        use wgpu::BufferUsages as U;
        let globals = buffer("globals", std::mem::size_of::<Globals>() as u64, U::UNIFORM | U::COPY_DST);
        let shapes = Growable::new(&device, "shapes", 2048 * std::mem::size_of::<ShapeInst>() as u64);
        let glasses = Growable::new(&device, "glasses", 128 * std::mem::size_of::<GlassInst>() as u64);
        let gshapes = Growable::new(&device, "gshapes", 256 * std::mem::size_of::<GShapeInst>() as u64);
        let masks = Growable::new(&device, "masks", 64 * std::mem::size_of::<MaskInst>() as u64);
        let probes = buffer("probes", MAX_PROBES * 16, U::STORAGE | U::COPY_DST);
        let stats = buffer("stats", MAX_PROBES * 8, U::STORAGE | U::COPY_SRC);
        let infos = buffer("infos", MAX_PASSES * INFO_STRIDE, U::UNIFORM | U::COPY_DST);

        let tex2d = |label: &str, size: u32, format| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        };
        let atlas = tex2d("glyphs", ATLAS_SIZE, wgpu::TextureFormat::R8Unorm);
        let sprites = tex2d("sprites", SPRITE_ATLAS_SIZE, wgpu::TextureFormat::Rgba8Unorm);
        let atlas_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atlas"),
            layout: &atlas_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas.create_view(&Default::default())),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&sprites.create_view(&Default::default())),
                },
            ],
        });

        let shape_group = Self::group4(&device, &shape_layout, &globals, &masks, &sampler, &[&shapes]);
        let glass_group =
            Self::group4(&device, &glass_layout, &globals, &masks, &sampler, &[&glasses, &gshapes]);

        let readbacks = (0..3)
            .map(|_| Readback {
                buffer: buffer("stats readback", MAX_PROBES * 8, U::COPY_DST | U::MAP_READ),
                keys: Vec::new(),
                state: Arc::new(AtomicU8::new(0)),
            })
            .collect();

        let timing = device.features().contains(wgpu::Features::TIMESTAMP_QUERY).then(|| Timing {
            set: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("timing"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            }),
            resolve: buffer("timing", 16, U::QUERY_RESOLVE | U::COPY_SRC),
            read: buffer("timing read", 16, U::COPY_DST | U::MAP_READ),
            state: Arc::new(AtomicU8::new(0)),
            period: queue.get_timestamp_period(),
        });

        let mut r = Self {
            device,
            queue,
            shape_pipe,
            glass_pipe,
            blur_pipe,
            present_pipe,
            stats_pipe,
            shape_layout,
            glass_layout,
            backdrop_layout,
            blur_layout,
            image_layout,
            stats_layout,
            sampler,
            globals,
            shapes,
            glasses,
            gshapes,
            masks,
            shape_group,
            glass_group,
            atlas,
            sprites,
            atlas_group,
            images: HashMap::new(),
            next_image: 0,
            targets: None,
            probes,
            stats,
            infos,
            readbacks,
            timing,
            grid: Grid::default(),
            frame_stats: FrameStats::default(),
        };
        r.add_image(1, 1, &[255; 4]);
        r
    }

    fn group4(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        globals: &wgpu::Buffer,
        masks: &Growable,
        sampler: &wgpu::Sampler,
        rest: &[&Growable],
    ) -> wgpu::BindGroup {
        let mut entries = vec![
            wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: masks.buffer.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
        ];
        for (i, b) in rest.iter().enumerate() {
            entries
                .push(wgpu::BindGroupEntry { binding: 3 + i as u32, resource: b.buffer.as_entire_binding() });
        }
        device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries: &entries })
    }

    /// Upload an RGBA8 image (not premultiplied).
    pub fn add_image(&mut self, w: u32, h: u32, rgba: &[u8]) -> ImageId {
        assert_eq!(rgba.len(), (w * h * 4) as usize, "image data does not match its size");
        let levels = 32 - w.max(h).max(1).leading_zeros();
        let t = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("image"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.write_texture(&t, 0, 0, 0, w, h, 4, rgba);
        // Smaller versions, so scaled down images stay smooth.
        let (mut lw, mut lh) = (w, h);
        let mut level: Vec<u8> = Vec::new();
        for m in 1..levels {
            let src: &[u8] = if m == 1 { rgba } else { &level };
            let (nw, nh, next) = halve(src, lw, lh);
            self.write_texture(&t, m, 0, 0, nw, nh, 4, &next);
            (lw, lh, level) = (nw, nh, next);
        }
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("image"),
            layout: &self.image_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&t.create_view(&Default::default())),
            }],
        });
        let id = self.next_image;
        self.next_image += 1;
        self.images.insert(id, (t, group));
        ImageId(id)
    }

    pub fn remove_image(&mut self, id: ImageId) {
        if id.0 != 0 {
            self.images.remove(&id.0);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn write_texture(
        &self,
        t: &wgpu::Texture,
        mip: u32,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        bpp: u32,
        data: &[u8],
    ) {
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: t,
                mip_level: mip,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * bpp), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
    }

    pub fn upload_atlas(&mut self, uploads: &mut Vec<AtlasUpload>) {
        for u in uploads.drain(..) {
            let (t, bpp) = if u.color { (&self.sprites, 4) } else { (&self.atlas, 1) };
            let mut data = u.data;
            if u.color {
                // The sampler interpolates, so store premultiplied.
                for p in data.as_chunks_mut::<4>().0 {
                    let a = p[3] as u32;
                    for c in &mut p[..3] {
                        *c = (*c as u32 * a / 255) as u8;
                    }
                }
            }
            self.write_texture(t, 0, u.x, u.y, u.w, u.h, bpp, &data);
        }
    }

    fn ensure_targets(&mut self, w: u32, h: u32) {
        if self.targets.as_ref().is_some_and(|t| t.size == (w, h)) {
            return;
        }
        let d = &self.device;
        let tex = (w.next_multiple_of(PAD), h.next_multiple_of(PAD));
        let extent = wgpu::Extent3d { width: tex.0, height: tex.1, depth_or_array_layers: 1 };
        use wgpu::TextureUsages as T;
        let make = |label, mips, usage| {
            d.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: extent,
                mip_level_count: mips,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: SCENE_FORMAT,
                usage,
                view_formats: &[],
            })
        };
        let scene = make("scene", 1, T::RENDER_ATTACHMENT | T::TEXTURE_BINDING | T::COPY_SRC);
        let backdrop = make("backdrop", MIPS, T::RENDER_ATTACHMENT | T::TEXTURE_BINDING | T::COPY_DST);
        let scene_view = scene.create_view(&Default::default());
        let full = backdrop.create_view(&Default::default());
        let mip_views: Vec<_> = (0..MIPS)
            .map(|i| {
                backdrop.create_view(&wgpu::TextureViewDescriptor {
                    base_mip_level: i,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let blur_group = |v: &wgpu::TextureView| {
            d.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.blur_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(v) },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            })
        };
        let mip_groups = mip_views.iter().map(blur_group).collect();
        let present_group = blur_group(&scene_view);
        let glass_group = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("backdrop"),
            layout: &self.backdrop_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&full),
            }],
        });
        let stats_group = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("stats"),
            layout: &self.stats_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.probes.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.stats.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&full) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.infos,
                        offset: 0,
                        size: std::num::NonZeroU64::new(std::mem::size_of::<StatInfo>() as u64),
                    }),
                },
            ],
        });
        self.targets = Some(Targets {
            size: (w, h),
            tex,
            scene,
            scene_view,
            backdrop,
            mip_views,
            mip_groups,
            present_group,
            glass_group,
            stats_group,
        });
    }

    /// Split the command list into passes, see the module docs.
    ///
    /// Every command gets the earliest pass that still draws it after everything
    /// it overlaps. Glass additionally needs a pass after the content it covers,
    /// so that content is part of its capture. Overlap is tracked on a coarse
    /// tile grid, which keeps planning linear in the painted area.
    fn plan(data: &DrawData, screen: Px, grid: &mut Grid) -> Plan {
        grid.reset(screen);
        let mut plan = Plan::default();
        let mut work: Vec<Work> = vec![Work::default()];
        let grow = |n: usize, work: &mut Vec<Work>| {
            while work.len() <= n {
                work.push(Work::default());
            }
        };
        for cmd in &data.cmds {
            match *cmd {
                Cmd::Shapes { start, end, image } => {
                    for s in &data.shapes[start as usize..end as usize] {
                        let e = s.soft * 2.5 + s.stroke + 1.0;
                        let r = Px {
                            x0: s.rect[0].min(s.rect[2]) - e,
                            y0: s.rect[1].min(s.rect[3]) - e,
                            x1: s.rect[0].max(s.rect[2]) + e,
                            y1: s.rect[1].max(s.rect[3]) + e,
                        }
                        .intersect(&Px::new(s.clip))
                        .intersect(&screen);
                        if r.empty() {
                            continue;
                        }
                        let (drawn, shade) = grid.query(&r);
                        let level = drawn.saturating_sub(1).max(shade);
                        grid.mark(&r, level + 1);
                        grow(level as usize, &mut work);
                        let w = &mut work[level as usize];
                        let at = w.shapes.len() as u32;
                        w.shapes.push(*s);
                        match w.pass.items.last_mut() {
                            Some(Item::Shapes { end, image: im, .. }) if *im == image => *end = at + 1,
                            _ => w.pass.items.push(Item::Shapes { start: at, end: at + 1, image }),
                        }
                    }
                }
                Cmd::Glass { index, probe } => {
                    let g = &data.glasses[index as usize];
                    let mut body = Px::NONE;
                    for s in &data.gshapes[g.shape_start as usize..(g.shape_start + g.shape_count) as usize] {
                        body = body.union(&Px {
                            x0: s.center[0] - s.half[0],
                            y0: s.center[1] - s.half[1],
                            x1: s.center[0] + s.half[0],
                            y1: s.center[1] + s.half[1],
                        });
                    }
                    let clip = Px::new(g.clip).intersect(&screen);
                    let b = body.grow(1.0).intersect(&clip);
                    if b.empty() {
                        continue;
                    }
                    let (drawn, shade) = grid.query(&b);
                    let level = drawn.max(shade).max(1).min(MAX_PASSES as u8 - 1);
                    // Later shapes stay on top of the visible part of the shadow.
                    let mut sh = body.grow(g.shadow_radius * 0.55);
                    sh.y1 += g.shadow_y;
                    grid.shade(&sh.intersect(&clip), level);
                    grid.mark(&b, level + 1);
                    grow(level as usize, &mut work);
                    let w = &mut work[level as usize];
                    w.pass.items.push(Item::Glass(index));
                    let half = (b.x1 - b.x0).min(b.y1 - b.y0) * 0.5;
                    let mut r = g.blur;
                    if g.light_spill > 0.0 {
                        r = r.max(14.0 * data.ppp);
                    }
                    if g.shadow > 0.0 && g.shadow_tint > 0.0 {
                        r = r.max((half * 1.6).max(10.0 * data.ppp));
                    }
                    if g.dark_edge > 0.0 {
                        r = r.max((half * 0.85).max(6.0 * data.ppp));
                    }
                    let mut lod = (1.0 + r * 0.75).log2().ceil();
                    if probe != u32::MAX {
                        w.probes.push(probe);
                        lod = lod.max(((b.x1 - b.x0).max(b.y1 - b.y0) / 8.0).log2().ceil().min(4.0));
                    }
                    let lod = lod.clamp(0.0, (MIPS - 1) as f32) as u32;
                    w.pass.max_lod = w.pass.max_lod.max(lod);
                    let reach =
                        3.0 * (1u32 << lod) as f32 + g.inner_height * 0.8 + g.outer_amount + 6.0 * data.ppp;
                    let c = w.pass.capture.unwrap_or(Px::NONE);
                    w.pass.capture = Some(c.union(&b.grow(reach)));
                }
            }
        }
        for mut w in work {
            let base = plan.shapes.len() as u32;
            plan.shapes.append(&mut w.shapes);
            for i in &mut w.pass.items {
                if let Item::Shapes { start, end, .. } = i {
                    *start += base;
                    *end += base;
                }
            }
            let first = plan.probes.len() as u32;
            plan.probes.append(&mut w.probes);
            w.pass.probes = first..plan.probes.len() as u32;
            plan.passes.push(w.pass);
        }
        plan
    }

    /// Collect finished backdrop measurements: (key, mean, deviation).
    pub fn poll_stats(&mut self) -> Vec<(u64, f32, f32)> {
        let _ = self.device.poll(wgpu::PollType::Poll);
        let mut out = Vec::new();
        for r in &mut self.readbacks {
            match r.state.load(Ordering::Acquire) {
                2 => {
                    if let Ok(view) = r.buffer.slice(..).get_mapped_range() {
                        let v: &[[f32; 2]] = bytemuck::cast_slice(&view);
                        out.extend(r.keys.iter().zip(v).map(|(k, s)| (*k, s[0], s[1])));
                    }
                    r.buffer.unmap();
                    r.state.store(0, Ordering::Release);
                }
                3 => r.state.store(0, Ordering::Release),
                _ => {}
            }
        }
        if let Some(t) = &self.timing
            && t.state.load(Ordering::Acquire) == 2
        {
            if let Ok(view) = t.read.slice(..).get_mapped_range() {
                let v: &[u64] = bytemuck::cast_slice(&view);
                let ms = v[1].saturating_sub(v[0]) as f32 * t.period / 1e6;
                let s = &mut self.frame_stats.gpu_ms;
                *s = Some(s.map_or(ms, |old| old * 0.9 + ms * 0.1));
            }
            t.read.unmap();
            t.state.store(0, Ordering::Release);
        }
        out
    }

    /// Measurements are still in flight.
    pub fn stats_pending(&self) -> bool {
        self.readbacks.iter().any(|r| r.state.load(Ordering::Acquire) != 0)
    }

    pub fn render(&mut self, data: &DrawData, target: &wgpu::TextureView, w: u32, h: u32) {
        self.ensure_targets(w, h);
        let (dev, q) = (self.device.clone(), self.queue.clone());
        let tex = self.targets.as_ref().unwrap().tex;
        let screen = Px { x0: 0.0, y0: 0.0, x1: tex.0 as f32, y1: tex.1 as f32 };
        let mut grid = std::mem::take(&mut self.grid);
        let plan = Self::plan(data, screen, &mut grid);
        self.grid = grid;
        let masks_new = self.masks.write(&dev, &q, &data.masks);
        if self.shapes.write(&dev, &q, &plan.shapes) | masks_new {
            self.shape_group = Self::group4(
                &dev,
                &self.shape_layout,
                &self.globals,
                &self.masks,
                &self.sampler,
                &[&self.shapes],
            );
        }
        if self.glasses.write(&dev, &q, &data.glasses)
            | self.gshapes.write(&dev, &q, &data.gshapes)
            | masks_new
        {
            self.glass_group = Self::group4(
                &dev,
                &self.glass_layout,
                &self.globals,
                &self.masks,
                &self.sampler,
                &[&self.glasses, &self.gshapes],
            );
        }
        let t = self.targets.as_ref().unwrap();
        let size = [w as f32, h as f32];
        let tex_size = [t.tex.0 as f32, t.tex.1 as f32];
        q.write_buffer(
            &self.globals,
            0,
            bytemuck::bytes_of(&Globals {
                size,
                tex_size,
                light: data.light,
                time: data.time,
                ppp: data.ppp,
            }),
        );
        let n_probes = plan.probes.len().min(MAX_PROBES as usize);
        if n_probes > 0 {
            let rects: Vec<[f32; 4]> =
                plan.probes[..n_probes].iter().map(|p| data.probes[*p as usize].1).collect();
            q.write_buffer(&self.probes, 0, bytemuck::cast_slice(&rects));
        }

        let passes = &plan.passes;
        let mut infos = vec![0u8; passes.len().min(MAX_PASSES as usize) * INFO_STRIDE as usize];
        for (i, p) in passes.iter().take(MAX_PASSES as usize).enumerate() {
            let info = StatInfo { tex_size, size, first: p.probes.start, pad: [0; 3] };
            let at = i * INFO_STRIDE as usize;
            infos[at..at + std::mem::size_of::<StatInfo>()].copy_from_slice(bytemuck::bytes_of(&info));
        }
        q.write_buffer(&self.infos, 0, &infos);

        let mut enc = dev.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        let time_this = self.timing.as_ref().filter(|t| t.state.load(Ordering::Acquire) == 0);
        let mut fs = FrameStats {
            passes: passes.len() as u32,
            shapes: plan.shapes.len() as u32,
            glasses: data.glasses.len() as u32,
            gpu_ms: self.frame_stats.gpu_ms,
            captured_px: 0,
        };

        for (pi, p) in passes.iter().enumerate() {
            if let Some(c) = p.capture {
                let align = |v: f32| (v.max(0.0) as u32) / PAD * PAD;
                let x0 = align(c.x0).min(t.tex.0);
                let y0 = align(c.y0).min(t.tex.1);
                let x1 = (c.x1.max(0.0).ceil() as u32).next_multiple_of(PAD).min(t.tex.0);
                let y1 = (c.y1.max(0.0).ceil() as u32).next_multiple_of(PAD).min(t.tex.1);
                if x1 > x0 && y1 > y0 {
                    fs.captured_px += (x1 - x0) as u64 * (y1 - y0) as u64;
                    let origin = wgpu::Origin3d { x: x0, y: y0, z: 0 };
                    let info = |texture| wgpu::TexelCopyTextureInfo {
                        texture,
                        mip_level: 0,
                        origin,
                        aspect: wgpu::TextureAspect::All,
                    };
                    enc.copy_texture_to_texture(
                        info(&t.scene),
                        info(&t.backdrop),
                        wgpu::Extent3d { width: x1 - x0, height: y1 - y0, depth_or_array_layers: 1 },
                    );
                    for lod in 1..=p.max_lod {
                        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("blur"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &t.mip_views[lod as usize],
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Load,
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            ..Default::default()
                        });
                        rp.set_scissor_rect(
                            x0 >> lod,
                            y0 >> lod,
                            ((x1 - x0) >> lod).max(1),
                            ((y1 - y0) >> lod).max(1),
                        );
                        rp.set_pipeline(&self.blur_pipe);
                        rp.set_bind_group(0, &t.mip_groups[lod as usize - 1], &[]);
                        rp.draw(0..3, 0..1);
                    }
                    let probes = p.probes.start..p.probes.end.min(n_probes as u32);
                    if !probes.is_empty() && pi < MAX_PASSES as usize {
                        let mut cp = enc.begin_compute_pass(&Default::default());
                        cp.set_pipeline(&self.stats_pipe);
                        cp.set_bind_group(0, &t.stats_group, &[(pi as u64 * INFO_STRIDE) as u32]);
                        cp.dispatch_workgroups(probes.len() as u32, 1, 1);
                    }
                }
            }
            let load = if pi == 0 {
                let c = data.clear;
                wgpu::LoadOp::Clear(wgpu::Color { r: c[0] as f64, g: c[1] as f64, b: c[2] as f64, a: 1.0 })
            } else {
                wgpu::LoadOp::Load
            };
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &t.scene_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
                })],
                timestamp_writes: match (time_this, pi) {
                    (Some(tm), 0) => Some(wgpu::RenderPassTimestampWrites {
                        query_set: &tm.set,
                        beginning_of_pass_write_index: Some(0),
                        end_of_pass_write_index: None,
                    }),
                    _ => None,
                },
                ..Default::default()
            });
            let mut mode = 0;
            for item in &p.items {
                match *item {
                    Item::Shapes { start, end, image } => {
                        if mode != 1 {
                            mode = 1;
                            rp.set_pipeline(&self.shape_pipe);
                            rp.set_bind_group(0, &self.shape_group, &[]);
                            rp.set_bind_group(1, &self.atlas_group, &[]);
                        }
                        let img = self.images.get(&image.0).or_else(|| self.images.get(&0));
                        if let Some((_, g)) = img {
                            rp.set_bind_group(2, g, &[]);
                            rp.draw(0..4, start..end);
                        }
                    }
                    Item::Glass(index) => {
                        let g = &data.glasses[index as usize];
                        let vis = Px::new(g.bounds).intersect(&Px::new(g.clip)).intersect(&screen);
                        if vis.empty() {
                            continue;
                        }
                        if mode != 2 {
                            mode = 2;
                            rp.set_pipeline(&self.glass_pipe);
                            rp.set_bind_group(0, &self.glass_group, &[]);
                            rp.set_bind_group(1, &t.glass_group, &[]);
                        }
                        rp.draw(0..4, index..index + 1);
                    }
                }
            }
        }

        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("present"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                timestamp_writes: time_this.map(|tm| wgpu::RenderPassTimestampWrites {
                    query_set: &tm.set,
                    beginning_of_pass_write_index: None,
                    end_of_pass_write_index: Some(1),
                }),
                ..Default::default()
            });
            rp.set_pipeline(&self.present_pipe);
            rp.set_bind_group(0, &t.present_group, &[]);
            rp.draw(0..3, 0..1);
        }

        if let Some(tm) = time_this {
            enc.resolve_query_set(&tm.set, 0..2, &tm.resolve, 0);
            enc.copy_buffer_to_buffer(&tm.resolve, 0, &tm.read, 0, 16);
        }
        let slot = (n_probes > 0)
            .then(|| self.readbacks.iter().position(|r| r.state.load(Ordering::Acquire) == 0))
            .flatten();
        if let Some(i) = slot {
            enc.copy_buffer_to_buffer(&self.stats, 0, &self.readbacks[i].buffer, 0, n_probes as u64 * 8);
        }
        let timed = time_this.is_some();
        q.submit([enc.finish()]);

        let arm = |buffer: &wgpu::Buffer, state: &Arc<AtomicU8>| {
            state.store(1, Ordering::Release);
            let s = state.clone();
            buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                s.store(if r.is_ok() { 2 } else { 3 }, Ordering::Release);
            });
        };
        if let Some(i) = slot {
            let r = &mut self.readbacks[i];
            r.keys = plan.probes[..n_probes].iter().map(|p| data.probes[*p as usize].0).collect();
            arm(&r.buffer, &r.state);
        }
        if timed && let Some(tm) = &self.timing {
            arm(&tm.read, &tm.state);
        }
        self.frame_stats = fs;
    }
}

/// Box filtered half size image.
fn halve(src: &[u8], w: u32, h: u32) -> (u32, u32, Vec<u8>) {
    let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
    let mut out = Vec::with_capacity((nw * nh * 4) as usize);
    for y in 0..nh {
        for x in 0..nw {
            let (x0, y0) = ((x * 2).min(w - 1), (y * 2).min(h - 1));
            let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
            for c in 0..4 {
                let at = |x: u32, y: u32| src[((y * w + x) * 4 + c) as usize] as u32;
                out.push(((at(x0, y0) + at(x1, y0) + at(x0, y1) + at(x1, y1) + 2) / 4) as u8);
            }
        }
    }
    (nw, nh, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glass_at(data: &mut DrawData, x: f32, probe: bool) {
        let mut g: GlassInst = Zeroable::zeroed();
        g.clip = [-1e6, -1e6, 1e6, 1e6];
        g.shape_start = data.gshapes.len() as u32;
        g.shape_count = 1;
        let mut s: GShapeInst = Zeroable::zeroed();
        s.center = [x, 50.0];
        s.half = [20.0, 20.0];
        data.gshapes.push(s);
        let p = if probe {
            data.probes.push((1, [0.0; 4]));
            data.probes.len() as u32 - 1
        } else {
            u32::MAX
        };
        data.cmds.push(Cmd::Glass { index: data.glasses.len() as u32, probe: p });
        data.glasses.push(g);
    }

    const SCREEN: Px = Px { x0: 0.0, y0: 0.0, x1: 400.0, y1: 400.0 };

    fn plan(d: &DrawData) -> Plan {
        Renderer::plan(d, SCREEN, &mut Grid::default())
    }

    fn shape_at(data: &mut DrawData, x: f32, w: f32) {
        let mut s: ShapeInst = Zeroable::zeroed();
        s.rect = [x, 45.0, x + w, 55.0];
        s.clip = [-1e6, -1e6, 1e6, 1e6];
        let at = data.shapes.len() as u32;
        data.shapes.push(s);
        data.cmds.push(Cmd::Shapes { start: at, end: at + 1, image: ImageId(0) });
    }

    #[test]
    fn halving_averages_and_stops_at_one_pixel() {
        let (w, h, px) = halve(&[0, 0, 0, 255, 200, 100, 50, 255], 2, 1);
        assert_eq!((w, h), (1, 1));
        assert_eq!(px, [100, 50, 25, 255]);
    }

    #[test]
    fn separate_glass_shares_one_capture() {
        let mut d = DrawData { ppp: 1.0, ..Default::default() };
        glass_at(&mut d, 50.0, true);
        glass_at(&mut d, 150.0, true);
        let p = plan(&d);
        assert_eq!(p.passes.len(), 2);
        assert_eq!(p.passes[1].probes, 0..2);
    }

    #[test]
    fn stacked_glass_gets_its_own_capture() {
        let mut d = DrawData { ppp: 1.0, ..Default::default() };
        glass_at(&mut d, 50.0, true);
        glass_at(&mut d, 55.0, true);
        let p = plan(&d);
        assert_eq!(p.passes.len(), 3);
        assert_eq!(p.passes[2].probes, 1..2);
        assert_eq!(p.probes, [0, 1]);
    }

    #[test]
    fn offscreen_glass_costs_nothing() {
        let mut d = DrawData { ppp: 1.0, ..Default::default() };
        glass_at(&mut d, -500.0, true);
        glass_at(&mut d, 50.0, true);
        let p = plan(&d);
        assert_eq!(p.passes.len(), 2);
        assert_eq!(p.probes, [1]);
    }

    #[test]
    fn track_and_thumb_pairs_do_not_chain() {
        // Track, thumb, track, thumb: later tracks move before all thumbs.
        let mut d = DrawData { ppp: 1.0, ..Default::default() };
        for x in [60.0, 200.0, 340.0] {
            shape_at(&mut d, x - 40.0, 80.0);
            glass_at(&mut d, x, true);
        }
        assert_eq!(plan(&d).passes.len(), 2);
    }

    #[test]
    fn label_stays_above_its_glass() {
        let mut d = DrawData { ppp: 1.0, ..Default::default() };
        glass_at(&mut d, 50.0, true);
        shape_at(&mut d, 45.0, 10.0);
        let p = plan(&d);
        assert!(matches!(p.passes[1].items[..], [Item::Glass(0), Item::Shapes { .. }]));
    }

    #[test]
    fn shape_between_two_glasses_is_refracted() {
        let mut d = DrawData { ppp: 1.0, ..Default::default() };
        glass_at(&mut d, 50.0, true);
        shape_at(&mut d, 45.0, 10.0);
        glass_at(&mut d, 50.0, true);
        let p = plan(&d);
        assert_eq!(p.passes.len(), 3);
        assert!(matches!(p.passes[2].items[..], [Item::Glass(1)]));
    }
}
