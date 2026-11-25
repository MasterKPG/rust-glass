//! The glass material: parameters, presets and how they adapt to the backdrop.

use crate::color::Color;
use crate::geom::{Rect, Vec2, lerp};

/// Corner shape. Rounded corners are continuous (curvature ramps up instead of
/// jumping), capsules are slightly squared off the way iOS draws them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Corner {
    Radius(f32),
    Capsule,
    /// True circle or stadium, without smoothing.
    Round(f32),
}

impl From<f32> for Corner {
    fn from(r: f32) -> Self {
        Corner::Radius(r)
    }
}

impl Corner {
    /// Radii and superellipse exponent for a rectangle of the given size.
    pub fn resolve(self, size: Vec2) -> (Vec2, f32) {
        let half = Vec2::splat(size.x.min(size.y) * 0.5);
        match self {
            Corner::Capsule => {
                // Extend the curve along the long side for a G2 transition.
                let mut r = half * 1.16;
                r = r.min(size * 0.5);
                // Nearly square shapes stay circles.
                let k = ((size.x.max(size.y) / size.x.min(size.y).max(0.1)) - 1.0).clamp(0.0, 0.3) / 0.3;
                (half.lerp(r, k), lerp(2.0, 2.18, k))
            }
            Corner::Radius(r) => {
                // A superellipse of extent 1.35r and exponent 2.83 touches a
                // circle of radius r on the diagonal.
                let want = r * 1.35;
                let room = (half.x - r).max(0.0) / (want - r).max(0.01);
                let k = room.clamp(0.0, 1.0);
                (Vec2::splat(lerp(r.min(half.x), want, k).max(0.01)), lerp(2.0, 2.83, k))
            }
            Corner::Round(r) => (Vec2::splat(r.min(half.x)), 2.0),
        }
    }
}

/// Every parameter of the material, in points where a length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    /// How far the rim bends light towards the middle.
    pub refraction: f32,
    /// Width of the curved part, measured from the outline.
    pub depth: f32,
    /// Opposite bend in a thin ring at the outline.
    pub outer_refraction: f32,
    pub outer_depth: f32,
    /// Chromatic aberration, as a fraction of the bend.
    pub chroma: f32,
    pub blur: f32,
    pub magnify: f32,
    /// Backdrop range mapping on a bright backdrop.
    pub light_range: (f32, f32),
    /// Backdrop range mapping on a dark backdrop.
    pub dark_range: (f32, f32),
    pub saturation: f32,
    /// Specular edge strength and width.
    pub rim: f32,
    pub rim_width: f32,
    pub dark_edge: f32,
    /// Light spilling in from the surroundings.
    pub bleed: f32,
    pub inner_glow: f32,
    pub inner_shadow: f32,
    pub shadow: f32,
    pub shadow_radius: f32,
    pub shadow_y: f32,
    /// How much of the transmitted color the shadow carries.
    pub shadow_tint: f32,
    /// Colored glass. Alpha is the strength.
    pub tint: Option<Color>,
    /// How much the tint follows backdrop brightness (0 is a flat fill).
    pub tint_follow: f32,
    /// How much the frost and reduced transparency settings apply. A lens that
    /// sits above its own label must stay clear.
    pub frosts: f32,
}

impl Material {
    /// Controls and bars. Legible over anything.
    pub const REGULAR: Material = Material {
        refraction: 17.0,
        depth: 13.0,
        outer_refraction: 3.0,
        outer_depth: 4.0,
        chroma: 0.10,
        blur: 4.2,
        magnify: 1.0,
        light_range: (0.62, 1.0),
        dark_range: (0.06, 0.52),
        saturation: 1.45,
        rim: 0.75,
        rim_width: 0.8,
        dark_edge: 0.26,
        bleed: 0.18,
        inner_glow: 0.16,
        inner_shadow: 0.05,
        shadow: 0.16,
        shadow_radius: 16.0,
        shadow_y: 6.0,
        shadow_tint: 0.35,
        tint: None,
        tint_follow: 1.0,
        frosts: 1.0,
    };

    /// Permanently transparent, for media backgrounds.
    pub const CLEAR: Material = Material {
        refraction: 24.0,
        depth: 18.0,
        outer_refraction: 4.0,
        outer_depth: 4.0,
        chroma: 0.20,
        blur: 0.5,
        light_range: (0.10, 1.0),
        dark_range: (0.04, 0.92),
        saturation: 1.25,
        rim: 0.85,
        inner_glow: 0.12,
        inner_shadow: 0.07,
        ..Material::REGULAR
    };

    /// Menus, sheets and windows: thicker, deeper shadow, softer light.
    pub const THICK: Material = Material {
        refraction: 34.0,
        depth: 26.0,
        outer_refraction: 4.0,
        outer_depth: 5.0,
        chroma: 0.10,
        blur: 11.0,
        light_range: (0.68, 1.0),
        dark_range: (0.09, 0.44),
        saturation: 1.6,
        shadow: 0.26,
        shadow_radius: 34.0,
        shadow_y: 14.0,
        bleed: 0.26,
        inner_glow: 0.10,
        ..Material::REGULAR
    };

    /// The droplet that lifts off a slider, switch or segment while touched.
    pub const LENS: Material = Material {
        refraction: 20.0,
        depth: 9.4,
        outer_refraction: 6.0,
        outer_depth: 1.6,
        chroma: 0.17,
        blur: 0.0,
        light_range: (0.15, 1.0),
        dark_range: (0.06, 1.0),
        saturation: 1.06,
        rim: 1.0,
        rim_width: 0.75,
        dark_edge: 0.92,
        bleed: 0.0,
        inner_glow: 0.20,
        inner_shadow: 0.09,
        shadow: 0.17,
        shadow_radius: 9.0,
        shadow_y: 5.0,
        shadow_tint: 0.95,
        frosts: 0.0,
        ..Material::REGULAR
    };

    pub fn tinted(mut self, c: Color) -> Self {
        self.tint = Some(c);
        self
    }

    /// Same look on an element `k` times as large.
    pub fn scaled(mut self, k: f32) -> Material {
        self.refraction *= k;
        self.depth *= k;
        self.outer_refraction *= k;
        self.outer_depth *= k;
        self.rim_width *= k.max(1.0).sqrt();
        self.shadow_radius *= k;
        self.shadow_y *= k;
        self
    }

    /// Blend two materials, used for lifts and morphs.
    pub fn lerp(&self, o: &Material, t: f32) -> Material {
        let l = |a: f32, b: f32| lerp(a, b, t);
        let l2 = |a: (f32, f32), b: (f32, f32)| (l(a.0, b.0), l(a.1, b.1));
        Material {
            refraction: l(self.refraction, o.refraction),
            depth: l(self.depth, o.depth),
            outer_refraction: l(self.outer_refraction, o.outer_refraction),
            outer_depth: l(self.outer_depth, o.outer_depth),
            chroma: l(self.chroma, o.chroma),
            blur: l(self.blur, o.blur),
            magnify: l(self.magnify, o.magnify),
            light_range: l2(self.light_range, o.light_range),
            dark_range: l2(self.dark_range, o.dark_range),
            saturation: l(self.saturation, o.saturation),
            rim: l(self.rim, o.rim),
            rim_width: l(self.rim_width, o.rim_width),
            dark_edge: l(self.dark_edge, o.dark_edge),
            bleed: l(self.bleed, o.bleed),
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
            frosts: l(self.frosts, o.frosts),
        }
    }
}

/// One shape of a glass element. Several shapes in one element fuse together.
#[derive(Clone, Copy, Debug)]
pub struct GlassShape {
    pub rect: Rect,
    pub corner: Corner,
    pub tint: Option<Color>,
}

impl GlassShape {
    pub fn new(rect: Rect, corner: impl Into<Corner>) -> Self {
        Self { rect, corner: corner.into(), tint: None }
    }
    pub fn tint(mut self, c: Option<Color>) -> Self {
        self.tint = c;
        self
    }
}

/// A glass element ready to paint.
#[derive(Clone, Debug)]
pub struct Glass {
    pub shapes: Vec<GlassShape>,
    pub material: Material,
    /// Distance in points at which neighbouring shapes start to merge.
    pub fuse: f32,
    /// 0 is absent, 1 fully formed. Scales the optics, not just the alpha.
    pub presence: f32,
    /// 0 uses the light range, 1 the dark range.
    pub darkness: f32,
    /// Extra shadow, set from backdrop contrast.
    pub shadow_boost: f32,
    /// Touch glow: position and strength.
    pub glow: Option<(Vec2, f32)>,
    /// Extra rotation of the highlights in radians.
    pub light_bend: f32,
    /// Measure the backdrop for this slot.
    pub(crate) stat_key: Option<u64>,
}

impl Glass {
    pub fn new(rect: Rect, corner: impl Into<Corner>, material: Material) -> Self {
        Self::group(vec![GlassShape::new(rect, corner)], material)
    }
    pub fn group(shapes: Vec<GlassShape>, material: Material) -> Self {
        Self {
            shapes,
            material,
            fuse: 12.0,
            presence: 1.0,
            darkness: 0.0,
            shadow_boost: 0.0,
            glow: None,
            light_bend: 0.0,
            stat_key: None,
        }
    }
    pub fn presence(mut self, p: f32) -> Self {
        self.presence = p.clamp(0.0, 1.0);
        self
    }
    pub fn fuse(mut self, f: f32) -> Self {
        self.fuse = f;
        self
    }
    pub fn glow(mut self, pos: Vec2, strength: f32) -> Self {
        self.glow = Some((pos, strength));
        self
    }
    pub fn darkness(mut self, d: f32) -> Self {
        self.darkness = d.clamp(0.0, 1.0);
        self
    }
    pub fn bounds(&self) -> Rect {
        self.shapes.iter().fold(Rect::NOTHING, |a, s| a.union(s.rect))
    }
}
