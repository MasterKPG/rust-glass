//! Painting: shapes, text, images, masks and glass. Widgets paint in points.
//! The draw list stores pixels, ready for the GPU.

use std::rc::Rc;

use bytemuck::{Pod, Zeroable};

use crate::color::Color;
use crate::geometry::{Rect, Vec2, lerp, vec2};
use crate::icon::Icon;
use crate::material::{Corner, Glass, Material};
use crate::text::{TextEngine, TextLayout};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub(crate) struct ShapeInst {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    pub color: [f32; 4],
    pub color2: [f32; 4],
    pub radius: [f32; 2],
    pub stroke: f32,
    pub soft: f32,
    pub clip: [f32; 4],
    pub kind: u32,
    pub mask: u32,
    pub power: f32,
    pub pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub(crate) struct MaskInst {
    pub rect: [f32; 4],
    pub radius: [f32; 2],
    pub power: f32,
    pub invert: f32,
    pub feather: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub(crate) struct GShapeInst {
    pub center: [f32; 2],
    pub half: [f32; 2],
    pub radius: [f32; 2],
    pub power: f32,
    pub pad: f32,
    pub tint: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub(crate) struct GlassInst {
    pub bounds: [f32; 4],
    pub clip: [f32; 4],
    pub tint: [f32; 4],
    pub shape_start: u32,
    pub shape_count: u32,
    pub mask: u32,
    pub fuse: f32,
    pub inner_amount: f32,
    pub inner_height: f32,
    pub outer_amount: f32,
    pub outer_height: f32,
    pub dispersion: f32,
    pub blur: f32,
    pub magnify: f32,
    pub opacity: f32,
    pub face_lo: f32,
    pub face_hi: f32,
    pub saturation: f32,
    pub light_bend: f32,
    pub rim: f32,
    pub rim_width: f32,
    pub dark_edge: f32,
    pub light_spill: f32,
    pub inner_glow: f32,
    pub inner_shadow: f32,
    pub brightness: f32,
    pub rim_base: f32,
    pub pinch: f32,
    pub pinch_width: f32,
    pub origin: [f32; 2],
    pub shadow: f32,
    pub shadow_radius: f32,
    pub shadow_y: f32,
    pub shadow_tint: f32,
}

const RRECT: u32 = 0;
const GLYPH: u32 = 1;
const SPRITE: u32 = 2;
const IMAGE: u32 = 3;
const ARC: u32 = 4;
const LINE: u32 = 5;

/// Handle to an uploaded image. Index 0 is a white pixel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct ImageId(pub u32);

#[derive(Clone, Copy, Debug)]
pub(crate) enum Cmd {
    Shapes { start: u32, end: u32, image: ImageId },
    Glass { index: u32, probe: u32 },
}

#[derive(Default)]
pub(crate) struct LayerList {
    pub shapes: Vec<ShapeInst>,
    pub glasses: Vec<(GlassInst, Option<u64>)>,
    /// (is_glass, index, image)
    order: Vec<(bool, u32, ImageId)>,
}

/// Flattened frame, consumed by the renderer.
#[derive(Default)]
pub(crate) struct DrawData {
    pub shapes: Vec<ShapeInst>,
    pub glasses: Vec<GlassInst>,
    pub gshapes: Vec<GShapeInst>,
    pub masks: Vec<MaskInst>,
    pub cmds: Vec<Cmd>,
    /// Keys of measured elements, in probe order.
    pub probes: Vec<(u64, [f32; 4])>,
    pub clear: [f32; 4],
    pub light: [f32; 2],
    pub time: f32,
    pub ppp: f32,
}

#[derive(Default)]
pub(crate) struct Paint {
    pub ppp: f32,
    pub layers: Vec<LayerList>,
    pub data: DrawData,
    /// Global material adjustments, from the theme.
    pub glass_tint: f32,
    pub reduce_transparency: bool,
    pub increase_contrast: bool,
}

impl Paint {
    pub fn begin(&mut self, ppp: f32) {
        self.ppp = ppp;
        for l in &mut self.layers {
            l.shapes.clear();
            l.glasses.clear();
            l.order.clear();
        }
        let d = &mut self.data;
        d.shapes.clear();
        d.glasses.clear();
        d.gshapes.clear();
        d.masks.clear();
        d.cmds.clear();
        d.probes.clear();
    }

    pub fn ensure_layer(&mut self, i: usize) {
        while self.layers.len() <= i {
            self.layers.push(LayerList::default());
        }
    }

    /// Concatenate layers in the given order.
    pub fn flatten(&mut self, order: &[usize]) {
        let d = &mut self.data;
        for &li in order {
            let l = &self.layers[li];
            for &(is_glass, idx, image) in &l.order {
                if is_glass {
                    let (g, key) = l.glasses[idx as usize];
                    let index = d.glasses.len() as u32;
                    d.glasses.push(g);
                    let probe = match key {
                        Some(k) => {
                            let s = d.gshapes[g.shape_start as usize];
                            d.probes.push((
                                k,
                                [
                                    s.center[0] - s.half[0],
                                    s.center[1] - s.half[1],
                                    s.center[0] + s.half[0],
                                    s.center[1] + s.half[1],
                                ],
                            ));
                            d.probes.len() as u32 - 1
                        }
                        None => u32::MAX,
                    };
                    d.cmds.push(Cmd::Glass { index, probe });
                } else {
                    let at = d.shapes.len() as u32;
                    d.shapes.push(l.shapes[idx as usize]);
                    if let Some(Cmd::Shapes { end, image: im, .. }) = d.cmds.last_mut()
                        && *end == at
                        && (*im == image || image.0 == u32::MAX || im.0 == u32::MAX)
                    {
                        *end += 1;
                        if im.0 == u32::MAX {
                            *im = image;
                        }
                    } else {
                        d.cmds.push(Cmd::Shapes { start: at, end: at + 1, image });
                    }
                }
            }
        }
        for c in &mut d.cmds {
            if let Cmd::Shapes { image, .. } = c
                && image.0 == u32::MAX
            {
                *image = ImageId(0);
            }
        }
    }
}

/// Marks a shape that does not care which image is bound.
const ANY_IMAGE: ImageId = ImageId(u32::MAX);

/// Draws shapes, text, images and glass into one layer. Get one from
/// [`Ui::painter`](crate::Ui::painter) or
/// [`Context::painter`](crate::Context::painter). All positions are in points.
pub struct Painter<'a> {
    pub(crate) paint: &'a mut Paint,
    pub(crate) text: &'a mut TextEngine,
    /// The layer painted into.
    pub layer: usize,
    /// Nothing is painted outside this rectangle.
    pub clip: Rect,
    /// Mask applied to everything painted. 0 is none.
    pub mask: u32,
    /// Opacity of everything painted, from 0 to 1.
    pub opacity: f32,
}

/// How far the edges of a mask fade out, in points. 0 is a hard edge.
#[derive(Clone, Copy, Debug, Default)]
pub struct Feather {
    /// Fade at the left edge.
    pub left: f32,
    /// Fade at the top edge.
    pub top: f32,
    /// Fade at the right edge.
    pub right: f32,
    /// Fade at the bottom edge.
    pub bottom: f32,
}

impl Painter<'_> {
    fn px(&self, r: Rect) -> [f32; 4] {
        let s = self.paint.ppp;
        [r.min.x * s, r.min.y * s, r.max.x * s, r.max.y * s]
    }

    fn base(&self, r: Rect, color: Color) -> ShapeInst {
        ShapeInst {
            rect: self.px(r),
            uv: [0.0; 4],
            color: color.fade(self.opacity).to_array(),
            color2: color.fade(self.opacity).to_array(),
            radius: [0.0; 2],
            stroke: 0.0,
            soft: 0.0,
            clip: self.px(self.clip),
            kind: RRECT,
            mask: self.mask,
            power: 2.0,
            pad: 0.0,
        }
    }

    fn push(&mut self, s: ShapeInst, image: ImageId) {
        if s.color[3] <= 0.0 && s.color2[3] <= 0.0 {
            return;
        }
        let e = if s.kind == RRECT { s.soft * 2.5 } else { 0.0 } + s.stroke;
        let c = s.clip;
        let (x0, x1) = (s.rect[0].min(s.rect[2]) - e, s.rect[0].max(s.rect[2]) + e);
        let (y0, y1) = (s.rect[1].min(s.rect[3]) - e, s.rect[1].max(s.rect[3]) + e);
        if x1 < c[0] || y1 < c[1] || x0 > c[2] || y0 > c[3] {
            return;
        }
        let l = &mut self.paint.layers[self.layer];
        l.order.push((false, l.shapes.len() as u32, image));
        l.shapes.push(s);
    }

    fn round(&self, s: &mut ShapeInst, r: Rect, corner: Corner) {
        let (rad, n) = corner.resolve(r.size());
        s.radius = [rad.x * self.paint.ppp, rad.y * self.paint.ppp];
        s.power = n;
    }

    /// Adds a mask and returns its number, for [`Painter::with_mask`]. Paint
    /// shows inside `rect`, or outside with `invert`. `feather` fades the
    /// edges of the rectangle.
    pub fn add_mask(&mut self, rect: Rect, corner: impl Into<Corner>, invert: bool, feather: Feather) -> u32 {
        let (rad, n) = corner.into().resolve(rect.size());
        let s = self.paint.ppp;
        let f = feather;
        let m = MaskInst {
            rect: self.px(rect),
            radius: [rad.x * s, rad.y * s],
            power: n,
            invert: invert as u32 as f32,
            feather: [f.left * s, f.top * s, f.right * s, f.bottom * s],
        };
        self.paint.data.masks.push(m);
        self.paint.data.masks.len() as u32
    }

    /// Runs `f` with `mask` applied to everything it paints.
    pub fn with_mask(&mut self, mask: u32, f: impl FnOnce(&mut Painter)) {
        let old = std::mem::replace(&mut self.mask, mask);
        f(self);
        self.mask = old;
    }

    /// Fills a rectangle.
    pub fn fill(&mut self, rect: Rect, corner: impl Into<Corner>, color: Color) {
        let mut s = self.base(rect, color);
        self.round(&mut s, rect, corner.into());
        self.push(s, ANY_IMAGE);
    }

    /// Fills a rectangle with a linear gradient from `start` to `end`.
    /// `direction` is in coordinates where the rectangle runs from 0 to 1, so
    /// `vec2(0.0, 1.0)` goes from top to bottom.
    pub fn gradient(
        &mut self,
        rect: Rect,
        corner: impl Into<Corner>,
        start: Color,
        end: Color,
        direction: Vec2,
    ) {
        let mut s = self.base(rect, start);
        self.round(&mut s, rect, corner.into());
        s.color2 = end.fade(self.opacity).to_array();
        s.uv = [direction.x, direction.y, 0.0, 0.0];
        self.push(s, ANY_IMAGE);
    }

    /// Draws the outline of a rectangle, centered on its edge.
    pub fn stroke(&mut self, rect: Rect, corner: impl Into<Corner>, width: f32, color: Color) {
        let mut s = self.base(rect, color);
        self.round(&mut s, rect, corner.into());
        s.stroke = (width * self.paint.ppp).max(0.75);
        self.push(s, ANY_IMAGE);
    }

    /// Draws a soft shadow of a rectangle, blurred by `softness` points.
    pub fn shadow(&mut self, rect: Rect, corner: impl Into<Corner>, softness: f32, color: Color) {
        let mut s = self.base(rect, color);
        self.round(&mut s, rect, corner.into());
        s.soft = softness * self.paint.ppp;
        self.push(s, ANY_IMAGE);
    }

    /// Draws a line from `from` to `to` with round ends.
    pub fn line(&mut self, from: Vec2, to: Vec2, width: f32, color: Color) {
        let mut s = self.base(Rect { min: from, max: to }, color);
        s.stroke = (width * 0.5 * self.paint.ppp).max(0.35);
        s.kind = LINE;
        self.push(s, ANY_IMAGE);
    }

    /// Draws an arc with round ends around `center`. Angles are in radians,
    /// clockwise from the right. The color runs from `start_color` to
    /// `end_color` along the arc.
    #[allow(clippy::too_many_arguments)]
    pub fn arc(
        &mut self,
        center: Vec2,
        radius: f32,
        width: f32,
        start: f32,
        sweep: f32,
        start_color: Color,
        end_color: Color,
    ) {
        let mut s = self.base(Rect::from_center_size(center, Vec2::splat(radius * 2.0)), start_color);
        s.color2 = end_color.fade(self.opacity).to_array();
        s.radius = [start, sweep.clamp(0.0, std::f32::consts::TAU)];
        s.stroke = width * 0.5 * self.paint.ppp;
        s.kind = ARC;
        self.push(s, ANY_IMAGE);
    }

    /// Draws the part `uv` (0 to 1 on both axes) of an image into `rect`,
    /// multiplied by `tint`.
    pub fn image(&mut self, rect: Rect, corner: impl Into<Corner>, image: ImageId, uv: Rect, tint: Color) {
        let mut s = self.base(rect, tint);
        self.round(&mut s, rect, corner.into());
        s.uv = [uv.min.x, uv.min.y, uv.max.x, uv.max.y];
        s.kind = IMAGE;
        self.push(s, image);
    }

    /// Draws laid out text with its top left corner at `position`.
    pub fn text(&mut self, position: Vec2, layout: &Rc<TextLayout>, color: Color) {
        self.text_scaled(position, layout, color, 1.0, position);
    }

    /// Draws text scaled around `origin`. It is rasterized close to its final
    /// size, so it stays sharp while a control grows or shrinks.
    pub fn text_scaled(
        &mut self,
        position: Vec2,
        layout: &Rc<TextLayout>,
        color: Color,
        scale: f32,
        origin: Vec2,
    ) {
        let (pos, l, c, about) = (position, layout, color, origin);
        if c.a * self.opacity <= 0.002 {
            return;
        }
        let ppp = self.paint.ppp;
        let moving = (scale - 1.0).abs() > 1e-3;
        let want = l.font_size * ppp * scale;
        let px = if moving { (want * 2.0).round().max(2.0) * 0.5 } else { l.font_size * ppp };
        let residual = if moving { want / px } else { 1.0 };
        for g in &l.glyphs {
            let p = about + (pos + g.pos - about) * scale;
            let mut at = p * ppp;
            if !moving {
                at.y = at.y.round();
            }
            let Some((e, o)) = self.text.glyph(g, px, at) else { continue };
            let min = if moving { at + (o - at + e.offset) * residual } else { o + e.offset };
            let max = min + e.size * residual;
            let mut s = self.base(Rect::default(), c);
            s.rect = [min.x, min.y, max.x, max.y];
            s.uv = e.uv;
            s.kind = if e.color { SPRITE } else { GLYPH };
            self.push(s, ANY_IMAGE);
        }
    }

    /// Draws text centered in `rect`, scaled around the center.
    pub fn text_centered(&mut self, rect: Rect, layout: &Rc<TextLayout>, color: Color, scale: f32) {
        let pos = rect.center() - layout.size * 0.5;
        self.text_scaled(pos, layout, color, scale, rect.center());
    }

    /// Draws an icon `size` points wide, centered on `center`.
    pub fn icon(&mut self, center: Vec2, size: f32, icon: Icon, color: Color) {
        let c = color;
        let want = size * self.paint.ppp;
        // Even pixel sizes; small residual scale while animating.
        let px = ((want / 2.0).round() * 2.0).max(4.0);
        let Some(e) = self.text.icon(icon, px as u32) else { return };
        let at = center * self.paint.ppp;
        let mut min = at - Vec2::splat(want * 0.5);
        if (want - px).abs() < 0.51 {
            min = vec2(min.x.round(), min.y.round());
        }
        let mut s = self.base(Rect::default(), c);
        s.rect = [min.x, min.y, min.x + want, min.y + want];
        s.uv = e.uv;
        s.kind = GLYPH;
        self.push(s, ANY_IMAGE);
    }

    /// Paints a glass element. It bends and blurs everything painted before
    /// it, on this layer and below.
    pub fn glass(&mut self, g: &Glass) {
        if g.presence <= 0.001 || g.shapes.is_empty() || self.opacity <= 0.001 {
            return;
        }
        let s = self.paint.ppp;
        let m: &Material = &g.material;
        // The setting runs from clear over the preset to fully tinted.
        let setting = self.paint.glass_tint.clamp(0.0, 1.0);
        let preset = crate::theme::Theme::DEFAULT_GLASS_TINT;
        let frost = ((setting - preset) / (1.0 - preset)).max(0.0) * m.glass_tint_response;
        let clear = ((preset - setting) / preset).max(0.0) * m.glass_tint_response;
        let solid_ui = if self.paint.reduce_transparency { m.glass_tint_response } else { 0.0 };
        let presence = g.presence * self.opacity;
        let start = self.paint.data.gshapes.len() as u32;
        let mut min_half = f32::MAX;
        for sh in &g.shapes {
            let size = sh.rect.size().max(Vec2::splat(0.5));
            let (rad, n) = sh.corner.resolve(size);
            let c = sh.rect.center();
            min_half = min_half.min(size.x.min(size.y) * 0.5);
            let tint = sh.tint.or(m.tint).unwrap_or(Color::TRANSPARENT);
            // Do not let a missing tint drag neighbours towards black.
            let tint = if tint.a <= 0.0 { m.tint.unwrap_or(Color::WHITE).alpha(0.0) } else { tint };
            self.paint.data.gshapes.push(GShapeInst {
                center: [c.x * s, c.y * s],
                half: [size.x * 0.5 * s, size.y * 0.5 * s],
                radius: [rad.x * s, rad.y * s],
                power: n,
                pad: 0.0,
                tint: tint.to_array(),
            });
        }
        // The curved part cannot be wider than the element, and the bend scales
        // with it so small controls keep the same look.
        let max_depth = min_half * 0.98;
        let k = (max_depth / m.depth.max(0.1)).min(1.0);
        let depth = m.depth * k;
        let refraction = m.refraction * k.max(0.35) * presence;

        let d = g.darkness;
        let mut lo = lerp(m.light_range.0, m.dark_range.0, d);
        let mut hi = lerp(m.light_range.1, m.dark_range.1, d);
        let solid = (solid_ui * 0.9).max(frost * 0.72);
        let target = lerp(0.97, 0.13, d);
        if m.tint.is_none() {
            lo *= 1.0 - 0.05 * clear;
            hi = lerp(hi, 1.0, 0.25 * clear);
        }
        // The frost setting moves from clear to fully tinted.
        if m.tint.is_none() || m.tint_follow > 0.5 {
            lo = lerp(lo, target, solid);
            hi = lerp(hi, lerp(1.0, 0.18, d), solid);
        }
        let blur = m.blur * (1.0 + frost * 1.5) * (1.0 - 0.62 * clear) + 12.0 * solid_ui;
        let bounds = g.bounds();
        let grow = m.shadow_radius * 2.0 + m.shadow_y.abs() + 3.0;
        let shadow = (m.shadow * (1.0 + g.shadow_boost)).min(0.6);
        let hc = if self.paint.increase_contrast { 0.5 } else { 0.0 };
        let c = bounds.center();
        let inst = GlassInst {
            bounds: self.px(bounds.expand(grow + g.fuse * 0.3)),
            clip: self.px(self.clip),
            tint: [m.sideways, m.corner_dispersion, 0.0, m.tint_follow],
            shape_start: start,
            shape_count: g.shapes.len() as u32,
            mask: self.mask,
            fuse: g.fuse * s,
            inner_amount: refraction * s,
            inner_height: depth * s,
            outer_amount: m.outer_refraction * k * presence * s,
            outer_height: m.outer_depth * k * s,
            dispersion: m.dispersion,
            blur: blur * s * presence.sqrt(),
            magnify: lerp(1.0, m.magnify, presence),
            opacity: presence,
            face_lo: lo,
            face_hi: hi,
            saturation: m.saturation,
            light_bend: g.light_bend,
            rim: (m.rim * lerp(1.0, 0.72, d)).min(1.0),
            rim_width: m.rim_width * s.max(1.0),
            dark_edge: (m.dark_edge + hc).min(0.9),
            light_spill: m.light_spill * presence,
            inner_glow: m.inner_glow * lerp(1.0, 0.6, d),
            inner_shadow: m.inner_shadow,
            brightness: g.brightness,
            rim_base: m.rim_base,
            pinch: m.pinch * k * presence * s,
            pinch_width: m.pinch_width * k * s,
            origin: [c.x * s, c.y * s],
            shadow: shadow * presence,
            shadow_radius: m.shadow_radius * s,
            shadow_y: m.shadow_y * s,
            shadow_tint: m.shadow_tint,
        };
        let l = &mut self.paint.layers[self.layer];
        l.order.push((true, l.glasses.len() as u32, ImageId(0)));
        l.glasses.push((inst, g.stat_key));
    }
}
