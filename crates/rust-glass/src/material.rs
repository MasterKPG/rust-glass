//! The glass material: corner shapes, material parameters and presets, and
//! glass elements ready to paint.

use crate::color::Color;
use crate::geometry::{Rect, Vec2, lerp, vec2};

/// The shape of a rectangle's corners.
///
/// Rounded corners are continuous: the curvature ramps up instead of jumping
/// where the straight edge ends, as on Apple platforms. A plain number
/// converts to [`Corner::Radius`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Corner {
    /// Continuous corners that look like a circular corner of this radius.
    Radius(f32),
    /// Fully rounded ends, slightly squared off the way iOS draws them.
    Capsule,
    /// A capsule of size `base` stretched to the size of the rectangle, each
    /// axis on its own. `power` above 2 squares the shoulders off.
    Stretched {
        /// Size of the capsule before stretching.
        base: Vec2,
        /// Exponent of the corner curve. 2 is a circle.
        power: f32,
    },
    /// Circular corners of this radius, without the continuous curve.
    Circular(f32),
}

impl From<f32> for Corner {
    fn from(radius: f32) -> Self {
        Corner::Radius(radius)
    }
}

impl Corner {
    /// Returns the corner radii and the exponent of the corner curve for a
    /// rectangle of `size`.
    pub fn resolve(self, size: Vec2) -> (Vec2, f32) {
        let half = Vec2::splat(size.x.min(size.y) * 0.5);
        match self {
            Corner::Capsule => {
                // Extend the curve along the long side for a smooth transition.
                let mut r = half * 1.16;
                r = r.min(size * 0.5);
                // Nearly square shapes stay circles.
                let k = ((size.x.max(size.y) / size.x.min(size.y).max(0.1)) - 1.0).clamp(0.0, 0.3) / 0.3;
                (half.lerp(r, k), lerp(2.0, 2.18, k))
            }
            Corner::Stretched { base, power } => {
                let (r, _) = Corner::Capsule.resolve(base);
                (vec2(r.x * size.x / base.x.max(0.1), r.y * size.y / base.y.max(0.1)), power)
            }
            Corner::Radius(r) => {
                // A superellipse of extent 1.35r and exponent 2.83 touches a
                // circle of radius r on the diagonal.
                let want = r * 1.35;
                let room = (half.x - r).max(0.0) / (want - r).max(0.01);
                let k = room.clamp(0.0, 1.0);
                (Vec2::splat(lerp(r.min(half.x), want, k).max(0.01)), lerp(2.0, 2.83, k))
            }
            Corner::Circular(r) => (Vec2::splat(r.min(half.x)), 2.0),
        }
    }
}

/// The optical parameters of glass. Lengths are in points.
///
/// The surface is flat in the middle and curves down near its outline.
/// Light passing through the curved part bends towards the middle, so the
/// edge shows the backdrop from further inside. Start from a preset such as
/// [`Material::REGULAR`] and change what you need.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    /// How far the curved edge bends the backdrop towards the middle.
    pub refraction: f32,
    /// Width of the curved edge, measured inwards from the outline.
    pub depth: f32,
    /// Bend in the opposite direction, in a thin ring right at the outline.
    pub outer_refraction: f32,
    /// Width of the outer ring.
    pub outer_depth: f32,
    /// Outward bend in a wide zone further in, like the raised shoulder of a
    /// drop of liquid.
    pub pinch: f32,
    /// Width of the pinch zone.
    pub pinch_width: f32,
    /// 0 bends equally in all directions. At 1 the main bend works only
    /// sideways and the pinch only up and down.
    pub sideways: f32,
    /// Separation of colors at the edge, as a fraction of the bend.
    pub dispersion: f32,
    /// 0 separates colors evenly around the outline. At 1 only the corners
    /// do, with the order of the colors alternating from corner to corner.
    pub corner_dispersion: f32,
    /// Blur radius of the backdrop.
    pub blur: f32,
    /// Magnification of the backdrop. 1 shows it at its real size.
    pub magnify: f32,
    /// Range the backdrop brightness is mapped to on light glass, as
    /// `(darkest, brightest)`.
    pub light_range: (f32, f32),
    /// Range the backdrop brightness is mapped to on dark glass.
    pub dark_range: (f32, f32),
    /// Color saturation of the backdrop. 1 keeps it unchanged.
    pub saturation: f32,
    /// Strength of the bright edge that catches the light.
    pub rim: f32,
    /// Width of the bright edge.
    pub rim_width: f32,
    /// Part of the bright edge that shows all around, whatever the light
    /// direction.
    pub rim_base: f32,
    /// Strength of the thin dark line just inside the outline.
    pub dark_edge: f32,
    /// Light from the surroundings that spills onto the edge.
    pub light_spill: f32,
    /// Glow on the side facing away from the light.
    pub inner_glow: f32,
    /// Shade on the side facing the light.
    pub inner_shadow: f32,
    /// Opacity of the drop shadow.
    pub shadow: f32,
    /// Blur radius of the drop shadow.
    pub shadow_radius: f32,
    /// Downward offset of the drop shadow.
    pub shadow_y: f32,
    /// How much of the color of the light passing through the shadow carries,
    /// from 0 (gray) to 1.
    pub shadow_tint: f32,
    /// Colored glass. The alpha is the strength of the color.
    pub tint: Option<Color>,
    /// How much the tint follows the brightness of the backdrop. 0 is a flat
    /// fill.
    pub tint_follow: f32,
    /// How much [`Theme::glass_tint`](crate::Theme::glass_tint) and reduced
    /// transparency change this material, from 0 to 1. Use 0 for glass that
    /// must stay clear, such as a lens above its own label.
    pub glass_tint_response: f32,
}

impl Material {
    /// Controls and bars. Readable over any content.
    pub const REGULAR: Material = Material {
        refraction: 17.0,
        depth: 13.0,
        outer_refraction: 3.0,
        outer_depth: 4.0,
        pinch: 0.0,
        pinch_width: 20.0,
        sideways: 0.0,
        dispersion: 0.10,
        corner_dispersion: 0.0,
        blur: 4.2,
        magnify: 1.0,
        light_range: (0.62, 1.0),
        dark_range: (0.06, 0.52),
        saturation: 1.45,
        rim: 0.75,
        rim_width: 0.8,
        rim_base: 0.34,
        dark_edge: 0.22,
        light_spill: 0.18,
        inner_glow: 0.16,
        inner_shadow: 0.05,
        shadow: 0.16,
        shadow_radius: 16.0,
        shadow_y: 6.0,
        shadow_tint: 0.35,
        tint: None,
        tint_follow: 1.0,
        glass_tint_response: 1.0,
    };

    /// Clear glass that stays transparent, for controls over photos and video.
    pub const CLEAR: Material = Material {
        refraction: 24.0,
        depth: 18.0,
        outer_refraction: 4.0,
        outer_depth: 4.0,
        dispersion: 0.20,
        blur: 0.5,
        light_range: (0.10, 1.0),
        dark_range: (0.04, 0.92),
        saturation: 1.25,
        rim: 0.85,
        inner_glow: 0.12,
        inner_shadow: 0.07,
        ..Material::REGULAR
    };

    /// Menus, sheets, windows and panels: thicker, with a deeper shadow and
    /// softer light.
    pub const THICK: Material = Material {
        refraction: 34.0,
        depth: 26.0,
        outer_refraction: 4.0,
        outer_depth: 5.0,
        dispersion: 0.10,
        blur: 11.0,
        light_range: (0.68, 1.0),
        dark_range: (0.09, 0.44),
        saturation: 1.6,
        shadow: 0.26,
        shadow_radius: 34.0,
        shadow_y: 14.0,
        light_spill: 0.26,
        inner_glow: 0.10,
        ..Material::REGULAR
    };

    /// The lens a slider, switch or segment thumb turns into while it is held.
    pub const LENS: Material = Material {
        refraction: 20.0,
        depth: 9.4,
        outer_refraction: 6.0,
        outer_depth: 1.6,
        dispersion: 0.17,
        blur: 0.0,
        light_range: (0.15, 1.0),
        dark_range: (0.06, 1.0),
        saturation: 1.06,
        rim: 1.0,
        rim_width: 0.75,
        rim_base: 0.06,
        dark_edge: 0.92,
        light_spill: 0.0,
        inner_glow: 0.20,
        inner_shadow: 0.09,
        shadow: 0.17,
        shadow_radius: 9.0,
        shadow_y: 5.0,
        shadow_tint: 0.95,
        glass_tint_response: 0.0,
        ..Material::REGULAR
    };

    /// The selection of a tab bar while it is held: a clear blob that stands
    /// out from the bar. A thin rim shows the bar from further inside, and
    /// the wide zone behind it pulls the edge of the bar inwards.
    pub const BLOB: Material = Material {
        refraction: 20.0,
        depth: 3.5,
        outer_refraction: 0.0,
        outer_depth: 2.0,
        pinch: 4.1,
        pinch_width: 20.0,
        dispersion: 0.75,
        corner_dispersion: 1.0,
        light_range: (0.0, 1.0),
        dark_range: (0.0, 1.0),
        saturation: 1.05,
        rim: 0.85,
        rim_width: 0.6,
        rim_base: 0.62,
        dark_edge: 0.0,
        inner_glow: 0.05,
        inner_shadow: 0.0,
        shadow: 0.13,
        shadow_radius: 11.0,
        shadow_y: 5.0,
        shadow_tint: 0.2,
        ..Material::LENS
    };

    /// Bends the tab labels under [`Material::BLOB`]: near its outline they
    /// smear inwards.
    pub const BLOB_LABELS: Material = Material {
        refraction: 23.0,
        depth: 16.0,
        outer_refraction: 0.0,
        pinch: 0.0,
        sideways: 0.5,
        dispersion: 0.18,
        corner_dispersion: 0.0,
        saturation: 1.0,
        inner_glow: 0.0,
        shadow: 0.0,
        ..Material::BLOB
    };

    /// Glass that only blurs: no bend, edge, light, shadow or tint. See
    /// [`Painter::blur`](crate::Painter::blur).
    pub fn blur(radius: f32) -> Material {
        Material {
            refraction: 0.0,
            depth: 0.1,
            outer_refraction: 0.0,
            outer_depth: 0.1,
            pinch: 0.0,
            dispersion: 0.0,
            corner_dispersion: 0.0,
            blur: radius,
            magnify: 1.0,
            light_range: (0.0, 1.0),
            dark_range: (0.0, 1.0),
            saturation: 1.0,
            rim: 0.0,
            rim_base: 0.0,
            dark_edge: 0.0,
            light_spill: 0.0,
            inner_glow: 0.0,
            inner_shadow: 0.0,
            shadow: 0.0,
            shadow_radius: 0.0,
            shadow_y: 0.0,
            shadow_tint: 0.0,
            tint: None,
            glass_tint_response: 0.0,
            ..Material::REGULAR
        }
    }

    /// Returns the material colored with `color`. Its alpha is the strength.
    pub fn tinted(mut self, color: Color) -> Self {
        self.tint = Some(color);
        self
    }

    /// Returns a material that looks the same on an element `k` times as
    /// large.
    pub fn scaled(mut self, k: f32) -> Material {
        self.refraction *= k;
        self.depth *= k;
        self.outer_refraction *= k;
        self.outer_depth *= k;
        self.pinch *= k;
        self.pinch_width *= k;
        self.rim_width *= k.max(1.0).sqrt();
        self.shadow_radius *= k;
        self.shadow_y *= k;
        self
    }

    /// Mixes towards `other`: `t` = 0 gives `self`, 1 gives `other`. Used
    /// when one kind of glass turns into another.
    pub fn lerp(&self, other: &Material, t: f32) -> Material {
        let o = other;
        let l = |a: f32, b: f32| lerp(a, b, t);
        let l2 = |a: (f32, f32), b: (f32, f32)| (l(a.0, b.0), l(a.1, b.1));
        Material {
            refraction: l(self.refraction, o.refraction),
            depth: l(self.depth, o.depth),
            outer_refraction: l(self.outer_refraction, o.outer_refraction),
            outer_depth: l(self.outer_depth, o.outer_depth),
            pinch: l(self.pinch, o.pinch),
            pinch_width: l(self.pinch_width, o.pinch_width),
            sideways: l(self.sideways, o.sideways),
            dispersion: l(self.dispersion, o.dispersion),
            corner_dispersion: l(self.corner_dispersion, o.corner_dispersion),
            blur: l(self.blur, o.blur),
            magnify: l(self.magnify, o.magnify),
            light_range: l2(self.light_range, o.light_range),
            dark_range: l2(self.dark_range, o.dark_range),
            saturation: l(self.saturation, o.saturation),
            rim: l(self.rim, o.rim),
            rim_width: l(self.rim_width, o.rim_width),
            rim_base: l(self.rim_base, o.rim_base),
            dark_edge: l(self.dark_edge, o.dark_edge),
            light_spill: l(self.light_spill, o.light_spill),
            inner_glow: l(self.inner_glow, o.inner_glow),
            inner_shadow: l(self.inner_shadow, o.inner_shadow),
            shadow: l(self.shadow, o.shadow),
            shadow_radius: l(self.shadow_radius, o.shadow_radius),
            shadow_y: l(self.shadow_y, o.shadow_y),
            shadow_tint: l(self.shadow_tint, o.shadow_tint),
            tint: match (self.tint, o.tint) {
                (None, None) => None,
                (Some(a), Some(b)) => Some(a.lerp(b, t)),
                (Some(a), None) => Some(a.fade(1.0 - t)),
                (None, Some(b)) => Some(b.fade(t)),
            },
            tint_follow: l(self.tint_follow, o.tint_follow),
            glass_tint_response: l(self.glass_tint_response, o.glass_tint_response),
        }
    }
}

/// One shape of a glass element. Shapes of one element that come close fuse
/// like drops of liquid.
#[derive(Clone, Copy, Debug)]
pub struct GlassShape {
    /// Where the shape is.
    pub rect: Rect,
    /// Its corners.
    pub corner: Corner,
    /// A color for this shape only, instead of the material tint.
    pub tint: Option<Color>,
}

impl GlassShape {
    /// Creates a shape.
    pub fn new(rect: Rect, corner: impl Into<Corner>) -> Self {
        Self { rect, corner: corner.into(), tint: None }
    }
    /// Colors this shape only.
    pub fn tint(mut self, color: Option<Color>) -> Self {
        self.tint = color;
        self
    }
}

/// A glass element ready to paint with [`Painter::glass`](crate::Painter::glass).
///
/// Call [`Context::prepare_glass`](crate::Context::prepare_glass) before
/// painting, so the element adapts to what is behind it.
#[derive(Clone, Debug)]
pub struct Glass {
    /// The shapes of the element. Usually one.
    pub shapes: Vec<GlassShape>,
    /// The material.
    pub material: Material,
    /// Distance in points at which neighbouring shapes start to merge.
    pub fuse: f32,
    /// How formed the element is, from 0 (absent) to 1. It scales the optics,
    /// not just the opacity, so glass appears by thickening.
    pub presence: f32,
    /// 0 uses the light range of the material, 1 the dark range.
    pub darkness: f32,
    /// Extra shadow over busy content.
    pub shadow_boost: f32,
    /// Even brightening, used as press feedback.
    pub brightness: f32,
    /// Extra rotation of the highlights, in radians.
    pub light_bend: f32,
    /// Measure the backdrop for this slot.
    pub(crate) stat_key: Option<u64>,
}

impl Glass {
    /// Creates an element of one shape.
    pub fn new(rect: Rect, corner: impl Into<Corner>, material: Material) -> Self {
        Self::group(vec![GlassShape::new(rect, corner)], material)
    }
    /// Creates an element of several shapes that fuse where they come close.
    pub fn group(shapes: Vec<GlassShape>, material: Material) -> Self {
        Self {
            shapes,
            material,
            fuse: 12.0,
            presence: 1.0,
            darkness: 0.0,
            shadow_boost: 0.0,
            brightness: 0.0,
            light_bend: 0.0,
            stat_key: None,
        }
    }
    /// Sets [`Glass::presence`].
    pub fn presence(mut self, presence: f32) -> Self {
        self.presence = presence.clamp(0.0, 1.0);
        self
    }
    /// Sets [`Glass::fuse`].
    pub fn fuse(mut self, distance: f32) -> Self {
        self.fuse = distance;
        self
    }
    /// Sets [`Glass::darkness`].
    pub fn darkness(mut self, darkness: f32) -> Self {
        self.darkness = darkness.clamp(0.0, 1.0);
        self
    }
    /// Returns the rectangle around all shapes.
    pub fn bounds(&self) -> Rect {
        self.shapes.iter().fold(Rect::NOTHING, |a, s| a.union(s.rect))
    }
}
