//! wgpu renderer.
//!
//! A frame is split into passes. Before each pass that contains glass, the
//! region behind that glass is copied from the scene and blurred into a mip
//! chain, which the glass samples. Glass can therefore refract everything below
//! it, including other glass. Commands that do not overlap are free to move to
//! an earlier pass, so a window full of controls needs only a few.
//!
//! The scene texture keeps the last frame. Each frame is compared with the
//! last one on a grid of tiles, and only the part that changed is drawn
//! again, grown to cover every glass element that sees it. A frame where only
//! a small meter moves redraws the meter. Set `RUST_GLASS_FULL_REDRAW=1` to
//! draw every frame in full.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use bytemuck::{Pod, Zeroable};

use crate::paint::{Cmd, DrawData, GShapeInst, GlassInst, ImageId, MaskInst, ShapeInst};
use crate::text::{ATLAS_SIZE, SPRITE_ATLAS_SIZE, TextEngine};

const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgb10a2Unorm;
const MIPS: u32 = 6;
const PAD: u32 = 32;
const MAX_PROBES: u64 = 512;
const INFO_STRIDE: u64 = 256;
const MAX_PASSES: u64 = 64;
/// Format of the half resolution target. It needs a real alpha channel.
const HALF_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// Glass blurred by at least this many pixels has a smooth inside, drawn at
/// half resolution.
const HALF_BLUR: f32 = 6.0;
/// The inside must be at least this large in pixels on both axes.
const HALF_MIN: f32 = 64.0;
/// Shape kind of the inside of glass drawn at half resolution.
const HALF_SHAPE: u32 = 6;

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
    /// The quality step: 0 full, 1 reduced, 2 low.
    quality: f32,
    pad: [f32; 3],
}

/// How far the renderer may simplify glass to keep up with the display.
///
/// Set it with [`Context::quality`](crate::Context::quality). The step in
/// effect is in [`FrameStats::quality`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Quality {
    /// Full quality, a step lower while large frames take longer than
    /// [`Quality::AUTO_BUDGET_MS`] on the GPU, and back up once there is room
    /// again. The default for windows. It needs a GPU that can time frames;
    /// elsewhere it stays at full quality.
    #[default]
    Auto,
    /// Always full quality. [`Headless`](crate::headless::Headless) starts
    /// with it, so renders are the same on every machine.
    Full,
    /// Glass casts no shadow, so it covers only its own body. This saves the
    /// most: glass costs by the pixels it covers.
    Reduced,
    /// The least work: no shadow, no rainbow edges, no light spilled onto
    /// edges, and blur read with plain filtering.
    Low,
}

impl Quality {
    /// GPU time per frame above which [`Quality::Auto`] steps down.
    pub const AUTO_BUDGET_MS: f32 = 14.0;

    fn step(self) -> u32 {
        match self {
            Quality::Auto | Quality::Full => 0,
            Quality::Reduced => 1,
            Quality::Low => 2,
        }
    }

    fn from_step(step: u32) -> Self {
        match step {
            0 => Quality::Full,
            1 => Quality::Reduced,
            _ => Quality::Low,
        }
    }
}

/// Frames in a row over the budget before [`Quality::Auto`] steps down.
const AUTO_OVER: u32 = 3;
/// Frames in a row with room to spare before it steps back up.
const AUTO_UNDER: u32 = 120;

/// The automatic quality step: follows the GPU time of frames that redraw a
/// large part of the window.
#[derive(Default)]
struct AutoQuality {
    step: u32,
    over: u32,
    under: u32,
}

impl AutoQuality {
    /// Takes the time of one frame. Returns true when the step changed.
    fn feed(&mut self, ms: f32) -> bool {
        let budget = Quality::AUTO_BUDGET_MS;
        if ms > budget {
            self.under = 0;
            self.over += 1;
            if self.over >= AUTO_OVER && self.step < 2 {
                self.step += 1;
                self.over = 0;
                return true;
            }
        } else if ms * 1.8 < budget {
            // A step up costs more than this frame did; only with room to spare.
            self.over = 0;
            self.under += 1;
            if self.under >= AUTO_UNDER && self.step > 0 {
                self.step -= 1;
                self.under = 0;
                return true;
            }
        } else {
            self.over = 0;
            self.under = 0;
        }
        false
    }
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
    fn intersects(&self, o: &Px) -> bool {
        !self.intersect(o).empty()
    }
    fn area(&self) -> f32 {
        if self.empty() { 0.0 } else { (self.x1 - self.x0) * (self.y1 - self.y0) }
    }
    /// Grown outwards to multiples of `step`.
    fn align(&self, step: f32) -> Px {
        Px {
            x0: (self.x0 / step).floor() * step,
            y0: (self.y0 / step).floor() * step,
            x1: (self.x1 / step).ceil() * step,
            y1: (self.y1 / step).ceil() * step,
        }
    }
}

/// A rectangle of the scene captured for glass to read, and the first level
/// of the blur chain its glass reads. A capture that starts at level 1 skips
/// the full size copy: its first level is filtered from the scene directly.
#[derive(Clone, Copy, Debug)]
struct Capture {
    rect: Px,
    first: u32,
}

/// Adds `c` to a list of captures, merged with the ones it touches. Glass far
/// apart gets separate captures, so two small controls in opposite corners do
/// not copy the whole window between them.
fn add_capture(list: &mut Vec<Capture>, c: Capture) {
    let mut c = c;
    while let Some(i) = list.iter().position(|o| o.rect.grow(PAD as f32).intersects(&c.rect)) {
        let o = list.swap_remove(i);
        c = Capture { rect: c.rect.union(&o.rect), first: c.first.min(o.first) };
    }
    list.push(c);
}

/// The first level of the blur chain a glass element reads: 1 for glass
/// blurred by enough that the shader never reads the sharp backdrop, and 0
/// for clear glass. `probe` is the rectangle its backdrop is measured in, if
/// it is measured.
fn first_level(g: &GlassInst, probe: Option<[f32; 4]>) -> u32 {
    // Every lookup of the shader blurs by at least 0.45 of the radius, at the
    // rim, and only a blur under 4 / 3 pixels reads the sharp level.
    if g.blur * 0.45 < 1.5 {
        return 0;
    }
    // The measurement reads the sharp level for rectangles under 16 pixels.
    match probe {
        Some(r) if (r[2] - r[0]).max(r[3] - r[1]) < 16.0 => 0,
        _ => 1,
    }
}

/// The mip level a glass element reads its blur from, and how far around its
/// body it reads the backdrop.
fn glass_reach(g: &GlassInst, b: &Px, probe: bool, ppp: f32) -> (u32, f32) {
    let half = (b.x1 - b.x0).min(b.y1 - b.y0) * 0.5;
    let mut r = g.blur;
    if g.light_spill > 0.0 {
        r = r.max(14.0 * ppp);
    }
    if g.shadow > 0.0 && g.shadow_tint > 0.0 {
        r = r.max((half * 1.6).max(10.0 * ppp));
    }
    if g.dark_edge > 0.0 {
        r = r.max((half * 0.85).max(6.0 * ppp));
    }
    let mut lod = (1.0 + r * 0.75).log2().ceil();
    if probe {
        lod = lod.max(((b.x1 - b.x0).max(b.y1 - b.y0) / 8.0).log2().ceil().min(4.0));
    }
    let lod = lod.clamp(0.0, (MIPS - 1) as f32) as u32;
    // How far the lens moves a sample away from the pixel: the outward bend,
    // the inner bend where its side flips, and the pinch, all spread by the
    // rainbow of dispersion.
    let bend = (g.outer_amount + g.inner_amount + 1.6 * g.pinch) * (1.0 + g.dispersion.abs());
    // Four texels of the blur level: the blur chain reads a little past the
    // edge of a capture, and glass must not see what lies beyond it.
    let reach = 4.0 * (1u32 << lod) as f32 + g.inner_height * 0.8 + bend + 6.0 * ppp;
    (lod, reach)
}

/// The inside of a glass element that can be drawn at half resolution, in
/// whole pixels: the part of its body where nothing changes faster than the
/// blur. The lens, rim, edge band and dark line all live in the margin left
/// around it.
fn interior(data: &DrawData, g: &GlassInst, ppp: f32) -> Option<Px> {
    if g.shape_count != 1 || g.blur < HALF_BLUR || g.opacity <= 0.0 {
        return None;
    }
    let s = data.gshapes.get(g.shape_start as usize)?;
    let curve = g.inner_height.max(g.outer_height).max(g.pinch_width).max(2.0 * ppp);
    let inset = curve + 8.0 * g.rim_width.max(0.9) + 4.0 * ppp + 0.3 * s.radius[0].max(s.radius[1]);
    let r = Px {
        x0: (s.center[0] - s.half[0] + inset).ceil(),
        y0: (s.center[1] - s.half[1] + inset).ceil(),
        x1: (s.center[0] + s.half[0] - inset).floor(),
        y1: (s.center[1] + s.half[1] - inset).floor(),
    };
    (r.x1 - r.x0 >= HALF_MIN && r.y1 - r.y0 >= HALF_MIN).then_some(r)
}

/// The body of a glass element: the union of its shapes.
fn glass_body(data: &DrawData, g: &GlassInst) -> Px {
    let mut body = Px::NONE;
    for s in &data.gshapes[g.shape_start as usize..(g.shape_start + g.shape_count) as usize] {
        body = body.union(&Px {
            x0: s.center[0] - s.half[0],
            y0: s.center[1] - s.half[1],
            x1: s.center[0] + s.half[0],
            y1: s.center[1] + s.half[1],
        });
    }
    body
}

/// Where a shape can draw, before its clip.
fn shape_extent(s: &ShapeInst) -> Px {
    let e = s.soft * 2.5 + s.stroke + 1.0;
    Px {
        x0: s.rect[0].min(s.rect[2]) - e,
        y0: s.rect[1].min(s.rect[3]) - e,
        x1: s.rect[0].max(s.rect[2]) + e,
        y1: s.rect[1].max(s.rect[3]) + e,
    }
}

/// FNV-1a over 32 bit words.
fn hash_bytes(h: u64, bytes: &[u8]) -> u64 {
    let mut h = h;
    for w in bytes.chunks(4) {
        let mut v = [0u8; 4];
        v[..w.len()].copy_from_slice(w);
        h = (h ^ u32::from_le_bytes(v) as u64).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

const DAMAGE_TILE: f32 = 32.0;

/// Finds the part of the window that changed since the last frame, and the
/// glass that has to read its backdrop again.
///
/// Every tile of the window keeps a hash of what is drawn on it, in the order
/// of the passes. A glass element sees only the passes before its own, so
/// its input is the hash of the tiles it reads at that point. Glass whose
/// input and look are the same as in the last frame keeps the backdrop it
/// captured then, even when it is drawn again because something next to it
/// changed.
#[derive(Default)]
struct Damage {
    w: usize,
    h: usize,
    tiles: Vec<u64>,
    prev: Vec<u64>,
    masks: Vec<u64>,
    globals: u64,
    /// The scene texture holds the whole last frame, and the backdrops of
    /// the passes hold what their glass read.
    valid: bool,
    /// Per glass in plan order: its look, its input and its pass.
    inputs: Vec<(u64, u64, u32)>,
    prev_inputs: Vec<(u64, u64, u32)>,
    /// For each glass in plan order: it reads its backdrop again.
    dirty: Vec<bool>,
    looks: Vec<u64>,
    moved: Vec<Px>,
}

/// The result of [`Damage::find`].
struct Found {
    /// The area to draw again, aligned to whole capture blocks.
    area: Px,
}

impl Damage {
    /// Tiles under `r`: first and last column and row, the last ones excluded.
    fn span(&self, r: &Px) -> (usize, usize, usize, usize) {
        let f = |v: f32, n: usize| ((v / DAMAGE_TILE).max(0.0) as usize).min(n);
        let c = |v: f32, n: usize| ((v / DAMAGE_TILE).ceil().max(0.0) as usize).min(n);
        (f(r.x0, self.w), f(r.y0, self.h), c(r.x1, self.w), c(r.y1, self.h))
    }

    fn mark(&mut self, r: &Px, item: u64) {
        let (x0, y0, x1, y1) = self.span(r);
        for y in y0..y1 {
            for t in &mut self.tiles[y * self.w + x0..y * self.w + x1] {
                *t = (t.rotate_left(7) ^ item).wrapping_mul(0x9e37_79b9_7f4a_7c15);
            }
        }
    }

    /// Hash of the tiles under `r` as they are now.
    fn read(&self, r: &Px) -> u64 {
        let (x0, y0, x1, y1) = self.span(r);
        let mut h = hash_bytes(0, bytemuck::bytes_of(&[x0 as u32, y0 as u32, x1 as u32, y1 as u32]));
        for y in y0..y1 {
            for t in &self.tiles[y * self.w + x0..y * self.w + x1] {
                h = (h ^ t).wrapping_mul(0x0100_0000_01b3);
            }
        }
        h
    }

    /// Returns what to draw again, or `None` when nothing changed. The first
    /// frame, and any after [`Self::valid`] was cleared, is drawn in full.
    fn find(&mut self, data: &DrawData, plan: &Plan, screen: Px) -> Option<Found> {
        let (w, h) = ((screen.x1 / DAMAGE_TILE).ceil() as usize, (screen.y1 / DAMAGE_TILE).ceil() as usize);
        if (w, h) != (self.w, self.h) {
            (self.w, self.h) = (w, h);
            self.valid = false;
        }
        let mut gh = hash_bytes(0, bytemuck::cast_slice(&data.clear));
        gh = hash_bytes(gh, bytemuck::cast_slice(&data.light));
        gh = hash_bytes(gh, bytemuck::bytes_of(&data.ppp));
        if gh != self.globals {
            self.globals = gh;
            self.valid = false;
        }
        self.tiles.clear();
        self.tiles.resize(w * h, 0);
        self.masks.clear();
        for m in &data.masks {
            let parent = if m.parent == 0 { 0 } else { self.masks[m.parent as usize - 1] };
            self.masks.push(hash_bytes(parent ^ 0xcbf2_9ce4_8422_2325, bytemuck::bytes_of(m)));
        }
        let mask =
            |masks: &[u64], i: u32| if i == 0 { 0 } else { masks.get(i as usize - 1).copied().unwrap_or(1) };
        self.inputs.clear();
        let mut dirty = std::mem::take(&mut self.dirty);
        let mut looks = std::mem::take(&mut self.looks);
        // Glass that reads again may look different, which glass in later
        // passes that reads over it must see.
        let mut moved = std::mem::take(&mut self.moved);
        dirty.clear();
        moved.clear();
        for (level, pass) in plan.passes.iter().enumerate() {
            // What the glass of this pass reads: the passes before it.
            looks.clear();
            for u in &pass.glass {
                let g = &data.glasses[u.index as usize];
                let shapes = &data.gshapes[g.shape_start as usize..(g.shape_start + g.shape_count) as usize];
                let look = hash_bytes(mask(&self.masks, g.mask), bytemuck::bytes_of(g));
                let look = hash_bytes(look, bytemuck::cast_slice(shapes));
                let key = (look, self.read(&u.reads), level as u32);
                let i = self.inputs.len();
                let again = !self.valid
                    || self.prev_inputs.get(i) != Some(&key)
                    || moved.iter().any(|m| m.intersects(&u.reads));
                dirty.push(again);
                self.inputs.push(key);
                looks.push(look);
            }
            let first = dirty.len() - pass.glass.len();
            for (k, u) in pass.glass.iter().enumerate() {
                if dirty[first + k] {
                    moved.push(u.drawn);
                }
            }
            let mut k = 0;
            for item in &pass.items {
                match *item {
                    Item::Shapes { start, end, image } => {
                        for s in &plan.shapes[start as usize..end as usize] {
                            let r = shape_extent(s).intersect(&Px::new(s.clip)).intersect(&screen);
                            if !r.empty() {
                                let item = hash_bytes(
                                    mask(&self.masks, s.mask) ^ image.0 as u64,
                                    bytemuck::bytes_of(s),
                                );
                                self.mark(&r, item);
                            }
                        }
                    }
                    Item::Glass(_) => {
                        self.mark(&pass.glass[k].drawn, looks[k]);
                        k += 1;
                    }
                }
            }
        }
        std::mem::swap(&mut self.inputs, &mut self.prev_inputs);
        std::mem::swap(&mut self.tiles, &mut self.prev);
        self.looks = looks;
        self.moved = moved;
        if !self.valid {
            self.valid = true;
            self.dirty = dirty;
            return Some(Found { area: screen });
        }
        let mut area = Px::NONE;
        for y in 0..h {
            for x in 0..w {
                if self.tiles[y * w + x] != self.prev[y * w + x] {
                    let (fx, fy) = (x as f32 * DAMAGE_TILE, y as f32 * DAMAGE_TILE);
                    area = area.union(&Px { x0: fx, y0: fy, x1: fx + DAMAGE_TILE, y1: fy + DAMAGE_TILE });
                }
            }
        }
        // Glass that reads again needs everything it reads drawn this frame.
        let mut i = 0;
        for pass in &plan.passes {
            for u in &pass.glass {
                if dirty[i] {
                    area = area.union(&u.reads).union(&u.drawn);
                }
                i += 1;
            }
        }
        self.dirty = dirty;
        if area.empty() {
            return None;
        }
        Some(Found { area: area.align(PAD as f32).intersect(&screen) })
    }
}

#[derive(Clone, Copy, Debug)]
enum Item {
    Shapes { start: u32, end: u32, image: ImageId },
    Glass(u32),
}

/// A glass element in a pass.
#[derive(Clone, Copy, Debug)]
struct GlassUse {
    index: u32,
    /// Index into the probe list of the frame, or `u32::MAX`.
    probe: u32,
    /// What it draws over, its shadow included.
    drawn: Px,
    /// What it reads from the backdrop.
    reads: Px,
    lod: u32,
    /// The first level of the blur chain it reads, see [`first_level`].
    first: u32,
}

#[derive(Default)]
struct Pass {
    items: Vec<Item>,
    /// The glass items of the pass, in the same order.
    glass: Vec<GlassUse>,
}

#[derive(Default)]
struct Plan {
    passes: Vec<Pass>,
    /// Shapes reordered by pass.
    shapes: Vec<ShapeInst>,
    /// The shapes of each pass while planning.
    pass_shapes: Vec<Vec<ShapeInst>>,
    /// Emptied passes of earlier frames, kept so planning allocates nothing.
    spare: Vec<Pass>,
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
    /// GPU time of the last measured frame, not smoothed.
    pub gpu_ms_last: Option<f32>,
    /// Pixels drawn again. Only the part of the window that changed is drawn.
    pub redrawn_px: u64,
    /// The quality step the frame was drawn with: `Full`, `Reduced` or `Low`.
    pub quality: Quality,
}

/// Lists a frame fills, kept from frame to frame.
#[derive(Default)]
struct Scratch {
    /// The captures of every pass, and where each pass's start and end.
    captures: Vec<Capture>,
    capture_ranges: Vec<std::ops::Range<usize>>,
    caps: Vec<Capture>,
    lods: Vec<u32>,
    probe_list: Vec<u32>,
    probe_ranges: Vec<std::ops::Range<u32>>,
    rects: Vec<[f32; 4]>,
    infos: Vec<u8>,
    copied: Vec<(u32, u32, u32, u32, u32)>,
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
    /// Whether the timed frame redrew enough of the window to judge the
    /// quality step by.
    large: bool,
}

struct Targets {
    size: (u32, u32),
    tex: (u32, u32),
    scene: wgpu::Texture,
    scene_view: wgpu::TextureView,
    present_group: wgpu::BindGroup,
    /// The inside of large blurred glass, at half resolution. Glass of one
    /// pass never overlaps, and each pass draws its own before it is read, so
    /// every pass shares it.
    half_view: wgpu::TextureView,
    /// `half_view` bound as an image of the shape pipeline.
    half_group: wgpu::BindGroup,
    /// The backdrop of each pass with glass, by pass.
    levels: Vec<Option<Level>>,
}

/// What the glass of one pass reads: the scene up to that pass, blurred into
/// a mip chain.
struct Level {
    backdrop: wgpu::Texture,
    mip_views: Vec<wgpu::TextureView>,
    mip_groups: Vec<wgpu::BindGroup>,
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
    half_pipe: wgpu::RenderPipeline,
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
    atlas_size: u32,
    sprites: wgpu::Texture,
    atlas_layout: wgpu::BindGroupLayout,
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
    damage: Damage,
    full_redraw: bool,
    /// Draws the inside of large blurred glass at half resolution.
    half: bool,
    /// The quality asked for, and the automatic step.
    quality: Quality,
    auto: AutoQuality,
    /// The glass of the frame with the insides chosen for half resolution.
    glass_scratch: Vec<GlassInst>,
    /// For each glass element, the shape that composites its inside, or
    /// `u32::MAX`.
    half_shapes: Vec<u32>,
    /// Lists of the last frame, kept so a frame allocates nothing.
    plan: Plan,
    scratch: Scratch,
    stats_out: Vec<(u64, f32, f32)>,
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
                    (vs, fs): (&str, &str),
                    topo,
                    format,
                    blend,
                    constants: &[(&str, f64)]| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: m,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState { topology: topo, ..Default::default() },
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: m,
                    entry_point: Some(fs),
                    compilation_options: wgpu::PipelineCompilationOptions { constants, ..Default::default() },
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        // The scene keeps no alpha; the half target needs it.
                        write_mask: if format == HALF_FORMAT {
                            wgpu::ColorWrites::ALL
                        } else {
                            wgpu::ColorWrites::COLOR
                        },
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
            ("vs", "fs"),
            TriangleStrip,
            SCENE_FORMAT,
            Some(premultiplied),
            &[],
        );
        let glass_pl = pipe_layout(&[Some(&glass_layout), Some(&backdrop_layout)]);
        let glass_pipe = pipe(
            "glass",
            &glass_pl,
            &glass_mod,
            ("vs", "fs"),
            TriangleStrip,
            SCENE_FORMAT,
            Some(premultiplied),
            &[],
        );
        let half_pipe = pipe(
            "glass half",
            &glass_pl,
            &glass_mod,
            ("vs_half", "fs_half"),
            TriangleStrip,
            HALF_FORMAT,
            None,
            &[],
        );
        let blur_pl = pipe_layout(&[Some(&blur_layout)]);
        let blur_pipe =
            pipe("blur", &blur_pl, &blur_mod, ("vs", "fs"), TriangleList, SCENE_FORMAT, None, &[]);
        let present_pipe =
            pipe("present", &blur_pl, &blur_mod, ("vs", "present"), TriangleList, output, None, &[]);
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
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        };
        let atlas = tex2d("glyphs", ATLAS_SIZE, wgpu::TextureFormat::R8Unorm);
        let sprites = tex2d("sprites", SPRITE_ATLAS_SIZE, wgpu::TextureFormat::Rgba8Unorm);
        let atlas_group = Self::atlas_group(&device, &atlas_layout, &atlas, &sprites);

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
            large: false,
        });

        let mut r = Self {
            device,
            queue,
            shape_pipe,
            glass_pipe,
            half_pipe,
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
            atlas_size: ATLAS_SIZE,
            sprites,
            atlas_layout,
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
            damage: Damage::default(),
            full_redraw: std::env::var_os("RUST_GLASS_FULL_REDRAW").is_some_and(|v| v != "0"),
            half: !std::env::var_os("RUST_GLASS_NO_HALF").is_some_and(|v| v != "0"),
            glass_scratch: Vec::new(),
            half_shapes: Vec::new(),
            plan: Plan::default(),
            scratch: Scratch::default(),
            stats_out: Vec::new(),
            quality: Quality::Auto,
            auto: AutoQuality::default(),
            frame_stats: FrameStats::default(),
        };
        r.add_image(1, 1, &[255; 4]);
        r
    }

    fn atlas_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        atlas: &wgpu::Texture,
        sprites: &wgpu::Texture,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atlas"),
            layout,
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
        })
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

    /// Uploads new glyphs, and follows the atlas when it grew or was emptied.
    pub fn upload_atlas(&mut self, text: &mut TextEngine) {
        if std::mem::take(&mut text.atlas_reset) {
            self.damage.valid = false;
        }
        if text.atlas_size != self.atlas_size {
            self.grow_atlas(text.atlas_size);
        }
        for u in text.uploads.drain(..) {
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

    /// Makes a larger glyph atlas that keeps the glyphs of the old one.
    fn grow_atlas(&mut self, size: u32) {
        let atlas = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyphs"),
            size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let old = self.atlas_size.min(size);
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_texture(
            self.atlas.as_image_copy(),
            atlas.as_image_copy(),
            wgpu::Extent3d { width: old, height: old, depth_or_array_layers: 1 },
        );
        self.queue.submit([enc.finish()]);
        self.atlas_group = Self::atlas_group(&self.device, &self.atlas_layout, &atlas, &self.sprites);
        self.atlas = atlas;
        self.atlas_size = size;
        log::debug!("glyph atlas grown to {size} x {size}");
    }

    fn ensure_targets(&mut self, w: u32, h: u32) {
        if self.targets.as_ref().is_some_and(|t| t.size == (w, h)) {
            return;
        }
        let tex = (w.next_multiple_of(PAD), h.next_multiple_of(PAD));
        let scene = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene"),
            size: wgpu::Extent3d { width: tex.0, height: tex.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SCENE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let scene_view = scene.create_view(&Default::default());
        let present_group = self.blur_group(&scene_view);
        let half = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("half"),
            size: wgpu::Extent3d { width: tex.0 / 2, height: tex.1 / 2, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HALF_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let half_view = half.create_view(&Default::default());
        let half_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("half"),
            layout: &self.image_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&half_view),
            }],
        });
        // New textures hold nothing yet.
        self.damage.valid = false;
        self.targets = Some(Targets {
            size: (w, h),
            tex,
            scene,
            scene_view,
            present_group,
            half_view,
            half_group,
            levels: Vec::new(),
        });
    }

    fn blur_group(&self, view: &wgpu::TextureView) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.blur_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        })
    }

    /// Makes the backdrop of pass `pass`. Each pass with glass keeps its own,
    /// so glass that reads nothing new keeps what it captured before.
    fn ensure_level(&mut self, pass: usize) {
        let t = self.targets.as_ref().expect("targets exist");
        if t.levels.get(pass).is_some_and(Option::is_some) {
            return;
        }
        let d = &self.device;
        let backdrop = d.create_texture(&wgpu::TextureDescriptor {
            label: Some("backdrop"),
            size: wgpu::Extent3d { width: t.tex.0, height: t.tex.1, depth_or_array_layers: 1 },
            mip_level_count: MIPS,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SCENE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
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
        let mip_groups = mip_views.iter().map(|v| self.blur_group(v)).collect();
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
        let level = Level { backdrop, mip_views, mip_groups, glass_group, stats_group };
        let t = self.targets.as_mut().expect("targets exist");
        while t.levels.len() <= pass {
            t.levels.push(None);
        }
        t.levels[pass] = Some(level);
        // The new backdrop holds nothing: its glass must capture.
        self.damage.valid = false;
    }

    /// Split the command list into passes, see the module docs.
    ///
    /// Every command gets the earliest pass that still draws it after everything
    /// it overlaps. Glass additionally needs a pass after the content it covers,
    /// so that content is part of its capture. Overlap is tracked on a coarse
    /// tile grid, which keeps planning linear in the painted area.
    #[cfg(test)]
    fn plan(data: &DrawData, screen: Px, grid: &mut Grid) -> Plan {
        let mut plan = Plan::default();
        Self::plan_into(data, screen, grid, &mut plan);
        plan
    }

    /// Plans into `plan`, reusing what it holds from the last frame.
    fn plan_into(data: &DrawData, screen: Px, grid: &mut Grid, plan: &mut Plan) {
        grid.reset(screen);
        let Plan { passes, shapes, pass_shapes, spare } = plan;
        for mut p in passes.drain(..) {
            p.items.clear();
            p.glass.clear();
            spare.push(p);
        }
        shapes.clear();
        for s in pass_shapes.iter_mut() {
            s.clear();
        }
        let grow = |n: usize,
                    passes: &mut Vec<Pass>,
                    spare: &mut Vec<Pass>,
                    pass_shapes: &mut Vec<Vec<ShapeInst>>| {
            while passes.len() <= n {
                passes.push(spare.pop().unwrap_or_default());
            }
            while pass_shapes.len() <= n {
                pass_shapes.push(Vec::new());
            }
        };
        grow(0, passes, spare, pass_shapes);
        for cmd in &data.cmds {
            match *cmd {
                Cmd::Shapes { start, end, image } => {
                    for s in &data.shapes[start as usize..end as usize] {
                        let r = shape_extent(s).intersect(&Px::new(s.clip)).intersect(&screen);
                        if r.empty() {
                            continue;
                        }
                        let (drawn, shade) = grid.query(&r);
                        let level = drawn.saturating_sub(1).max(shade);
                        grid.mark(&r, level + 1);
                        grow(level as usize, passes, spare, pass_shapes);
                        let list = &mut pass_shapes[level as usize];
                        let at = list.len() as u32;
                        list.push(*s);
                        let pass = &mut passes[level as usize];
                        match pass.items.last_mut() {
                            Some(Item::Shapes { end, image: im, .. }) if *im == image => *end = at + 1,
                            _ => pass.items.push(Item::Shapes { start: at, end: at + 1, image }),
                        }
                    }
                }
                Cmd::Glass { index, probe } => {
                    let g = &data.glasses[index as usize];
                    let body = glass_body(data, g);
                    let clip = Px::new(g.clip).intersect(&screen);
                    let b = body.grow(1.0).intersect(&clip);
                    let drawn = Px::new(g.bounds).intersect(&clip);
                    if b.empty() || drawn.empty() {
                        continue;
                    }
                    let (drawn_level, shade) = grid.query(&b);
                    let level = drawn_level.max(shade).max(1).min(MAX_PASSES as u8 - 1);
                    // Later shapes stay on top of the visible part of the shadow.
                    let mut sh = body.grow(g.shadow_radius * 0.55);
                    sh.y1 += g.shadow_y;
                    grid.shade(&sh.intersect(&clip), level);
                    grid.mark(&b, level + 1);
                    grow(level as usize, passes, spare, pass_shapes);
                    let pass = &mut passes[level as usize];
                    pass.items.push(Item::Glass(index));
                    let (lod, reach) = glass_reach(g, &b, probe != u32::MAX, data.ppp);
                    let reads = b.grow(reach).intersect(&screen);
                    let first = first_level(g, data.probes.get(probe as usize).map(|p| p.1));
                    pass.glass.push(GlassUse { index, probe, drawn, reads, lod, first });
                }
            }
        }
        for (pass, list) in passes.iter_mut().zip(pass_shapes.iter()) {
            let base = shapes.len() as u32;
            shapes.extend_from_slice(list);
            for i in &mut pass.items {
                if let Item::Shapes { start, end, .. } = i {
                    *start += base;
                    *end += base;
                }
            }
        }
    }

    /// Collect finished backdrop measurements: (key, mean, deviation).
    pub fn poll_stats(&mut self) -> &[(u64, f32, f32)] {
        let _ = self.device.poll(wgpu::PollType::Poll);
        let mut out = std::mem::take(&mut self.stats_out);
        out.clear();
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
                self.frame_stats.gpu_ms_last = Some(ms);
                // A frame that redrew a small part says little about the rest.
                if self.quality == Quality::Auto && t.large && self.auto.feed(ms) {
                    self.damage.valid = false;
                    log::debug!("quality step {} after a frame of {ms:.1} ms", self.auto.step);
                }
            }
            t.read.unmap();
            t.state.store(0, Ordering::Release);
        }
        self.stats_out = out;
        &self.stats_out
    }

    /// Draws every frame in full when `on`, as `RUST_GLASS_FULL_REDRAW=1`
    /// does, so frame timings measure the whole window.
    pub fn set_full_redraw(&mut self, on: bool) {
        self.full_redraw = on;
    }

    /// Measurements are still in flight.
    /// Draws the inside of large blurred glass at half resolution when `on`,
    /// the default.
    pub fn set_half_resolution(&mut self, on: bool) {
        if self.half != on {
            self.half = on;
            self.damage.valid = false;
        }
    }

    /// Sets how far glass may be simplified to keep up.
    pub fn set_quality(&mut self, quality: Quality) {
        if self.quality != quality {
            if self.quality.step() != quality.step() || quality == Quality::Auto {
                self.damage.valid = false;
            }
            self.quality = quality;
            self.auto = AutoQuality::default();
        }
    }

    /// The quality step in use: 0 full, 1 reduced, 2 low.
    fn quality_step(&self) -> u32 {
        if self.quality == Quality::Auto { self.auto.step } else { self.quality.step() }
    }

    pub fn stats_pending(&self) -> bool {
        self.readbacks.iter().any(|r| r.state.load(Ordering::Acquire) != 0)
    }

    pub fn render(&mut self, data: &DrawData, target: &wgpu::TextureView, w: u32, h: u32) {
        self.set_quality(data.quality);
        self.ensure_targets(w, h);
        let (dev, q) = (self.device.clone(), self.queue.clone());
        let tex = self.targets.as_ref().unwrap().tex;
        let screen = Px { x0: 0.0, y0: 0.0, x1: tex.0 as f32, y1: tex.1 as f32 };
        let mut grid = std::mem::take(&mut self.grid);
        let mut plan = std::mem::take(&mut self.plan);
        Self::plan_into(data, screen, &mut grid, &mut plan);
        self.grid = grid;
        // Backdrops for the passes with glass. A new one makes the frame full.
        for (pi, pass) in plan.passes.iter().enumerate() {
            if !pass.glass.is_empty() {
                self.ensure_level(pi);
            }
        }
        if self.full_redraw {
            self.damage.valid = false;
        }
        let area = match self.damage.find(data, &plan, screen) {
            Some(f) => f.area,
            None => Px { x0: 0.0, y0: 0.0, x1: 0.0, y1: 0.0 },
        };
        let dirty = std::mem::take(&mut self.damage.dirty);
        let mut sc = std::mem::take(&mut self.scratch);
        let nothing = area.empty();
        let whole =
            !nothing && area.x0 <= 0.0 && area.y0 <= 0.0 && area.x1 >= screen.x1 && area.y1 >= screen.y1;
        // A part of the window is cleared by a rectangle in the clear color,
        // drawn first in the first pass.
        let clear = (!whole && !nothing).then(|| {
            let c = data.clear;
            let r = area.grow(4.0);
            plan.shapes.push(ShapeInst {
                rect: [r.x0, r.y0, r.x1, r.y1],
                uv: [0.0; 4],
                color: [c[0], c[1], c[2], 1.0],
                color2: [c[0], c[1], c[2], 1.0],
                radius: [0.0; 2],
                stroke: 0.0,
                soft: 0.0,
                clip: [r.x0, r.y0, r.x1, r.y1],
                kind: 0,
                mask: 0,
                power: 2.0,
                pad: 0.0,
            });
            plan.shapes.len() as u32 - 1
        });

        // The insides drawn at half resolution, each composited by a shape.
        let mut glasses = std::mem::take(&mut self.glass_scratch);
        glasses.clear();
        glasses.extend_from_slice(&data.glasses);
        let mut half_shapes = std::mem::take(&mut self.half_shapes);
        half_shapes.clear();
        half_shapes.resize(glasses.len(), u32::MAX);
        // Below full quality glass casts no shadow, so each element covers
        // only its body. On integrated GPUs glass costs by the pixels it
        // covers more than by its math.
        if self.quality_step() >= 1 {
            for g in glasses.iter_mut() {
                if g.shadow > 0.0 {
                    let body = glass_body(data, g).grow(2.0 + g.fuse * 0.3);
                    let b = Px::new(g.bounds).intersect(&body);
                    g.bounds = [b.x0, b.y0, b.x1, b.y1];
                    g.shadow = 0.0;
                }
            }
        }
        for p in &plan.passes {
            for u in &p.glass {
                let g = &mut glasses[u.index as usize];
                let Some(r) = self.half.then(|| interior(data, g, data.ppp)).flatten() else { continue };
                g.interior = [r.x0, r.y0, r.x1, r.y1];
                half_shapes[u.index as usize] = plan.shapes.len() as u32;
                plan.shapes.push(ShapeInst {
                    rect: [r.x0, r.y0, r.x1, r.y1],
                    uv: [r.x0 / screen.x1, r.y0 / screen.y1, r.x1 / screen.x1, r.y1 / screen.y1],
                    color: [1.0; 4],
                    color2: [1.0; 4],
                    radius: [0.0; 2],
                    stroke: 0.0,
                    soft: 0.0,
                    clip: g.clip,
                    kind: HALF_SHAPE,
                    mask: g.mask,
                    power: 2.0,
                    pad: 0.0,
                });
            }
        }

        // What each pass captures: the reads of its glass that reads again,
        // with the measurements of that glass.
        sc.captures.clear();
        sc.capture_ranges.clear();
        sc.lods.clear();
        sc.probe_list.clear();
        sc.probe_ranges.clear();
        let Scratch { captures, capture_ranges, caps, lods, probe_list, probe_ranges, rects, infos, copied } =
            &mut sc;
        let mut gi = 0;
        for p in &plan.passes {
            caps.clear();
            let mut lod = 0;
            let first = probe_list.len() as u32;
            for u in &p.glass {
                let again = dirty.get(gi).copied().unwrap_or(false);
                gi += 1;
                if !again || nothing {
                    continue;
                }
                add_capture(caps, Capture { rect: u.reads.intersect(&area), first: u.first.min(u.lod) });
                lod = lod.max(u.lod);
                if u.probe != u32::MAX && probe_list.len() < MAX_PROBES as usize {
                    probe_list.push(u.probe);
                }
            }
            let start = captures.len();
            captures.extend_from_slice(caps);
            capture_ranges.push(start..captures.len());
            lods.push(lod);
            probe_ranges.push(first..probe_list.len() as u32);
        }

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
        if self.glasses.write(&dev, &q, &glasses) | self.gshapes.write(&dev, &q, &data.gshapes) | masks_new {
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
                quality: self.quality_step() as f32,
                pad: [0.0; 3],
            }),
        );
        let n_probes = probe_list.len();
        if n_probes > 0 {
            rects.clear();
            rects.extend(probe_list.iter().map(|p| data.probes[*p as usize].1));
            q.write_buffer(&self.probes, 0, bytemuck::cast_slice(rects));
        }

        let passes = if nothing { &[][..] } else { &plan.passes[..] };
        infos.clear();
        infos.resize(passes.len().min(MAX_PASSES as usize) * INFO_STRIDE as usize, 0);
        for (i, r) in probe_ranges.iter().take(passes.len().min(MAX_PASSES as usize)).enumerate() {
            let info = StatInfo { tex_size, size, first: r.start, pad: [0; 3] };
            let at = i * INFO_STRIDE as usize;
            infos[at..at + std::mem::size_of::<StatInfo>()].copy_from_slice(bytemuck::bytes_of(&info));
        }
        if !infos.is_empty() {
            q.write_buffer(&self.infos, 0, infos);
        }

        let mut enc = dev.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        let time_this = self.timing.as_ref().filter(|t| t.state.load(Ordering::Acquire) == 0);
        let mut fs = FrameStats {
            passes: passes.len() as u32,
            shapes: if nothing { 0 } else { plan.shapes.len() as u32 },
            glasses: data.glasses.len() as u32,
            gpu_ms: self.frame_stats.gpu_ms,
            gpu_ms_last: self.frame_stats.gpu_ms_last,
            captured_px: 0,
            redrawn_px: if nothing { 0 } else { area.area() as u64 },
            quality: Quality::from_step(self.quality_step()),
        };
        // Everything is drawn inside the changed area only.
        let scissor = {
            let x0 = (area.x0.max(0.0) as u32).min(t.tex.0);
            let y0 = (area.y0.max(0.0) as u32).min(t.tex.1);
            let x1 = (area.x1.max(0.0).ceil() as u32).min(t.tex.0);
            let y1 = (area.y1.max(0.0).ceil() as u32).min(t.tex.1);
            (x0, y0, x1.saturating_sub(x0).max(1), y1.saturating_sub(y0).max(1))
        };

        for (pi, p) in passes.iter().enumerate() {
            let level = t.levels.get(pi).and_then(Option::as_ref);
            copied.clear();
            if let Some(lv) = level {
                for cap in &captures[capture_ranges[pi].clone()] {
                    let c = cap.rect;
                    let align = |v: f32| (v.max(0.0) as u32) / PAD * PAD;
                    let x0 = align(c.x0).min(t.tex.0);
                    let y0 = align(c.y0).min(t.tex.1);
                    let x1 = (c.x1.max(0.0).ceil() as u32).next_multiple_of(PAD).min(t.tex.0);
                    let y1 = (c.y1.max(0.0).ceil() as u32).next_multiple_of(PAD).min(t.tex.1);
                    if x1 > x0 && y1 > y0 && cap.first > 0 {
                        // Filtered from the scene in the first blur step.
                        fs.captured_px += ((x1 - x0) as u64 * (y1 - y0) as u64) >> (2 * cap.first);
                        copied.push((x0, y0, x1, y1, cap.first));
                    } else if x1 > x0 && y1 > y0 {
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
                            info(&lv.backdrop),
                            wgpu::Extent3d { width: x1 - x0, height: y1 - y0, depth_or_array_layers: 1 },
                        );
                        copied.push((x0, y0, x1, y1, 0));
                    }
                }
                if !copied.is_empty() {
                    for lod in 1..=lods[pi] {
                        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("blur"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &lv.mip_views[lod as usize],
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Load,
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            ..Default::default()
                        });
                        rp.set_pipeline(&self.blur_pipe);
                        let mut from_scene = None;
                        for &(x0, y0, x1, y1, first) in copied.iter() {
                            // A capture without the full size copy reads the
                            // scene for its first level.
                            let scene = lod == 1 && first == 1;
                            if from_scene != Some(scene) {
                                from_scene = Some(scene);
                                let src =
                                    if scene { &t.present_group } else { &lv.mip_groups[lod as usize - 1] };
                                rp.set_bind_group(0, src, &[]);
                            }
                            rp.set_scissor_rect(
                                x0 >> lod,
                                y0 >> lod,
                                ((x1 - x0) >> lod).max(1),
                                ((y1 - y0) >> lod).max(1),
                            );
                            rp.draw(0..3, 0..1);
                        }
                    }
                    let probes = probe_ranges[pi].clone();
                    if !probes.is_empty() && pi < MAX_PASSES as usize {
                        let mut cp = enc.begin_compute_pass(&Default::default());
                        cp.set_pipeline(&self.stats_pipe);
                        cp.set_bind_group(0, &lv.stats_group, &[(pi as u64 * INFO_STRIDE) as u32]);
                        cp.dispatch_workgroups(probes.len() as u32, 1, 1);
                    }
                }
            }
            // The insides of this pass at half resolution, inside the changed
            // area grown by the reach of the filter that scales them up.
            let halves = level.is_some() && p.glass.iter().any(|u| half_shapes[u.index as usize] != u32::MAX);
            if halves {
                let (hw, hh) = (t.tex.0 / 2, t.tex.1 / 2);
                let x0 = ((area.x0 - 6.0).max(0.0) as u32 / 2).min(hw);
                let y0 = ((area.y0 - 6.0).max(0.0) as u32 / 2).min(hh);
                let x1 = ((area.x1 + 6.0).max(0.0) as u32).div_ceil(2).min(hw);
                let y1 = ((area.y1 + 6.0).max(0.0) as u32).div_ceil(2).min(hh);
                if x1 > x0 && y1 > y0 {
                    let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("half"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &t.half_view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                        })],
                        ..Default::default()
                    });
                    rp.set_scissor_rect(x0, y0, x1 - x0, y1 - y0);
                    rp.set_pipeline(&self.half_pipe);
                    rp.set_bind_group(0, &self.glass_group, &[]);
                    if let Some(lv) = level {
                        rp.set_bind_group(1, &lv.glass_group, &[]);
                    }
                    for u in &p.glass {
                        if half_shapes[u.index as usize] != u32::MAX {
                            rp.draw(0..4, u.index..u.index + 1);
                        }
                    }
                }
            }
            let load = if pi == 0 && whole {
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
            rp.set_scissor_rect(scissor.0, scissor.1, scissor.2, scissor.3);
            let mut mode = 0;
            if let (0, Some(c)) = (pi, clear)
                && let Some((_, g)) = self.images.get(&0)
            {
                mode = 1;
                rp.set_pipeline(&self.shape_pipe);
                rp.set_bind_group(0, &self.shape_group, &[]);
                rp.set_bind_group(1, &self.atlas_group, &[]);
                rp.set_bind_group(2, g, &[]);
                rp.draw(0..4, c..c + 1);
            }
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
                        let vis = Px::new(g.bounds).intersect(&Px::new(g.clip)).intersect(&area);
                        let Some(lv) = level else { continue };
                        if vis.empty() {
                            continue;
                        }
                        let half = half_shapes[index as usize];
                        if half != u32::MAX {
                            if mode != 1 {
                                rp.set_pipeline(&self.shape_pipe);
                                rp.set_bind_group(0, &self.shape_group, &[]);
                                rp.set_bind_group(1, &self.atlas_group, &[]);
                            }
                            rp.set_bind_group(2, &t.half_group, &[]);
                            rp.draw(0..4, half..half + 1);
                            mode = 1;
                        }
                        if mode != 2 {
                            mode = 2;
                            rp.set_pipeline(&self.glass_pipe);
                            rp.set_bind_group(0, &self.glass_group, &[]);
                            rp.set_bind_group(1, &lv.glass_group, &[]);
                        }
                        rp.draw(0..10, index..index + 1);
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
                    // Without scene passes the frame starts here.
                    beginning_of_pass_write_index: passes.is_empty().then_some(0),
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
            r.keys.clear();
            r.keys.extend(probe_list.iter().map(|p| data.probes[*p as usize].0));
            arm(&r.buffer, &r.state);
        }
        if timed && let Some(tm) = &mut self.timing {
            tm.large = area.area() >= screen.area() * 0.25;
            arm(&tm.read, &tm.state);
        }
        self.frame_stats = fs;
        self.glass_scratch = glasses;
        self.half_shapes = half_shapes;
        self.plan = plan;
        self.scratch = sc;
        self.damage.dirty = dirty;
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
        g.bounds = [x - 20.0, 30.0, x + 20.0, 70.0];
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

    #[test]
    fn only_large_blurred_glass_has_an_inside_at_half_resolution() {
        let mut d = DrawData { ppp: 1.0, ..Default::default() };
        glass_at(&mut d, 200.0, false);
        let sized = |d: &mut DrawData, half: f32, blur: f32| {
            d.gshapes[0].half = [half, half];
            d.gshapes[0].radius = [12.0, 12.0];
            d.glasses[0].blur = blur;
            d.glasses[0].opacity = 1.0;
            d.glasses[0].inner_height = 10.0;
            d.glasses[0].rim_width = 1.0;
        };
        sized(&mut d, 150.0, 10.0);
        let r = interior(&d, &d.glasses[0], 1.0).expect("a large blurred panel has an inside");
        // Whole pixels, inside the body by the curved edge and the rim.
        assert_eq!(r.x0, r.x0.round());
        assert!(r.x0 >= 50.0 + 10.0 + 8.0 && r.x1 <= 350.0 - 18.0, "{r:?}");
        // Clear glass shows sharp content through its middle.
        sized(&mut d, 150.0, 1.0);
        assert!(interior(&d, &d.glasses[0], 1.0).is_none());
        // A button is too small to gain from it.
        sized(&mut d, 30.0, 10.0);
        assert!(interior(&d, &d.glasses[0], 1.0).is_none());
    }

    #[test]
    fn automatic_quality_steps_down_on_slow_frames_and_back_up_with_room() {
        let mut q = AutoQuality::default();
        let over = Quality::AUTO_BUDGET_MS + 4.0;
        // One slow frame is not enough.
        assert!(!q.feed(over) && !q.feed(over));
        assert!(q.feed(over));
        assert_eq!(q.step, 1);
        // A frame within the budget restarts the count.
        q.feed(over);
        q.feed(over);
        q.feed(Quality::AUTO_BUDGET_MS - 1.0);
        assert!(!q.feed(over));
        // Frames with little room keep the step; plenty of room brings it back.
        for _ in 0..500 {
            q.feed(Quality::AUTO_BUDGET_MS * 0.8);
        }
        assert_eq!(q.step, 1);
        let fast = Quality::AUTO_BUDGET_MS * 0.3;
        let ups = (0..AUTO_UNDER).filter(|_| q.feed(fast)).count();
        assert_eq!((ups, q.step), (1, 0));
    }

    fn plan(d: &DrawData) -> Plan {
        Renderer::plan(d, SCREEN, &mut Grid::default())
    }

    fn probes(p: &Pass) -> Vec<u32> {
        p.glass.iter().filter(|u| u.probe != u32::MAX).map(|u| u.probe).collect()
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
        assert_eq!(probes(&p.passes[1]), [0, 1]);
    }

    #[test]
    fn stacked_glass_gets_its_own_capture() {
        let mut d = DrawData { ppp: 1.0, ..Default::default() };
        glass_at(&mut d, 50.0, true);
        glass_at(&mut d, 55.0, true);
        let p = plan(&d);
        assert_eq!(p.passes.len(), 3);
        assert_eq!(probes(&p.passes[1]), [0]);
        assert_eq!(probes(&p.passes[2]), [1]);
    }

    #[test]
    fn offscreen_glass_costs_nothing() {
        let mut d = DrawData { ppp: 1.0, ..Default::default() };
        glass_at(&mut d, -500.0, true);
        glass_at(&mut d, 50.0, true);
        let p = plan(&d);
        assert_eq!(p.passes.len(), 2);
        assert_eq!(probes(&p.passes[1]), [1]);
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
    fn far_apart_glass_gets_separate_captures() {
        let mut d = DrawData { ppp: 1.0, ..Default::default() };
        glass_at(&mut d, 50.0, true);
        glass_at(&mut d, 350.0, true);
        let p = plan(&d);
        let mut caps = Vec::new();
        for u in &p.passes[1].glass {
            add_capture(&mut caps, Capture { rect: u.reads, first: u.first });
        }
        assert_eq!(caps.len(), 2);
    }

    /// What [`Damage::find`] found, with the glass that reads again.
    struct Seen {
        area: Px,
        dirty: Vec<bool>,
    }

    #[test]
    fn damage_covers_what_changed_and_the_glass_that_sees_it() {
        let screen = Px { x0: 0.0, y0: 0.0, x1: 1024.0, y1: 512.0 };
        let frame = |x: f32| {
            let mut d = DrawData { ppp: 1.0, ..Default::default() };
            shape_at(&mut d, x, 10.0);
            glass_at(&mut d, 600.0, true);
            d
        };
        let mut damage = Damage::default();
        let mut find = |d: &DrawData| {
            let found = damage.find(d, &Renderer::plan(d, screen, &mut Grid::default()), screen);
            found.map(|f| Seen { area: f.area, dirty: damage.dirty.clone() })
        };
        // The first frame is drawn in full.
        let first = find(&frame(100.0)).unwrap();
        assert_eq!(first.area.area(), screen.area());
        assert_eq!(first.dirty, [true]);
        // Nothing changed.
        assert!(find(&frame(100.0)).is_none());
        // The shape moved, far from the glass: only its tiles, and the glass
        // keeps its backdrop.
        let far = find(&frame(110.0)).unwrap();
        assert!(far.area.x0 <= 100.0 && far.area.x1 >= 121.0 && far.area.x1 < 400.0, "{:?}", far.area);
        assert_eq!(far.dirty, [false]);
        // It moved under what the glass reads: the glass reads again, and all
        // it reads is drawn again.
        let near = find(&frame(570.0)).unwrap();
        assert_eq!(near.dirty, [true]);
        assert!(near.area.x0 <= 570.0 && near.area.x1 >= 620.0, "{:?}", near.area);
    }

    #[test]
    fn a_label_on_glass_does_not_make_it_read_again() {
        let screen = Px { x0: 0.0, y0: 0.0, x1: 1024.0, y1: 512.0 };
        // Glass with a label drawn over it, in a later pass.
        let frame = |shade: f32| {
            let mut d = DrawData { ppp: 1.0, ..Default::default() };
            glass_at(&mut d, 600.0, true);
            shape_at(&mut d, 595.0, 10.0);
            d.shapes.last_mut().unwrap().color = [shade, 0.0, 0.0, 1.0];
            d
        };
        let mut damage = Damage::default();
        let mut find = |d: &DrawData| {
            let found = damage.find(d, &Renderer::plan(d, screen, &mut Grid::default()), screen);
            found.map(|f| Seen { area: f.area, dirty: damage.dirty.clone() })
        };
        find(&frame(0.0));
        let changed = find(&frame(1.0)).unwrap();
        assert_eq!(changed.dirty, [false]);
    }

    #[test]
    fn blurred_glass_skips_the_sharp_copy() {
        let mut g: GlassInst = Zeroable::zeroed();
        // Clear glass reads the sharp backdrop.
        g.blur = 1.0;
        assert_eq!(first_level(&g, None), 0);
        // Blurred glass does not, even with a lens.
        g.blur = 6.0;
        g.inner_amount = 4.0;
        assert_eq!(first_level(&g, None), 1);
        assert_eq!(first_level(&g, Some([0.0, 0.0, 40.0, 40.0])), 1);
        // A small measured rectangle reads the sharp level.
        assert_eq!(first_level(&g, Some([0.0, 0.0, 10.0, 10.0])), 0);
        // A capture shared with clear glass keeps the copy.
        let mut caps = Vec::new();
        add_capture(&mut caps, Capture { rect: Px::new([0.0, 0.0, 100.0, 100.0]), first: 1 });
        add_capture(&mut caps, Capture { rect: Px::new([90.0, 0.0, 200.0, 100.0]), first: 0 });
        assert_eq!(caps.len(), 1);
        assert_eq!(caps[0].first, 0);
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
