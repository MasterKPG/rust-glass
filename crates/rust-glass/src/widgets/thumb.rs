//! The thumb of sliders, switches, segmented controls and the tab bar: an
//! opaque knob that lifts into a clear lens and changes shape with acceleration.

use crate::animation::{Spring, Tuning};
use crate::color::Color;
use crate::context::{Context, Id, Response};
use crate::geometry::{Rect, Vec2, lerp, vec2};
use crate::material::{Corner, Glass};
use crate::widgets::press::rubber_band;

/// Where the thumb is being dragged, shared by the three controls. Positions
/// are relative to `ox`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn drag_target(
    ctx: &mut Context,
    id: Id,
    r: &Response,
    ox: f32,
    pos: f32,
    lo: f32,
    hi: f32,
    limit: f32,
) -> Option<f32> {
    #[derive(Clone, Copy, Default)]
    struct Grab {
        off: f32,
        on: bool,
    }
    let mut g: Grab = ctx.get(id);
    let mut out = None;
    if r.held
        && let Some(p) = r.pointer
    {
        if r.pressed {
            g = Grab { off: 0.0, on: true };
            if (p.x - ox - pos).abs() < limit * 2.5 {
                g.off = p.x - ox - pos;
            }
        }
        if g.on {
            let x = p.x - ox - g.off;
            out = Some(x.clamp(lo, hi) + rubber_band(x - x.clamp(lo, hi), limit));
        }
    } else {
        g.on = false;
    }
    ctx.set(id, g);
    out
}

/// State of a thumb. Positions are relative to the control, so moving the
/// control (dragging its window, scrolling) does not move the thumb inside it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Thumb {
    /// Center along the axis of travel, from the left edge of the control.
    pub pos: Spring,
    pub lift: Spring,
    /// Follows the velocity; its own rate of change is the smoothed
    /// acceleration that shapes the drop.
    chase: Spring,
}

#[derive(Clone, Copy, Debug)]
pub struct ThumbFrame {
    pub size: Vec2,
    /// Size without the stretch.
    pub base: Vec2,
    pub lift: f32,
    pub bend: f32,
    /// Not moving and not lifted.
    pub at_rest: bool,
}

/// How a droplet gives way to acceleration.
#[derive(Clone, Copy, Debug)]
pub struct ThumbFlex {
    /// Smooths the acceleration.
    pub chase: Tuning,
    /// Change of the height at the limit.
    pub amount: f32,
    /// Acceleration that reaches three quarters of the limit, in 1000 points
    /// per second squared.
    pub at: f32,
    /// Part of the change the width takes on. 1 keeps the area.
    pub width: f32,
    /// How the lens sinks back into the knob once it is home.
    pub settle: Tuning,
}

impl ThumbFlex {
    /// Thumbs, fitted to the reference footage.
    pub const THUMB: ThumbFlex = ThumbFlex {
        chase: Tuning::new(0.628, 0.5),
        amount: 0.159,
        at: 2.4,
        width: 1.0,
        settle: Tuning::new(0.36, 0.76),
    };
    /// The blob of a tab bar, fitted to the iOS 27 clips: it stretches wide
    /// as it lands and settles without a wobble.
    pub const BLOB: ThumbFlex = ThumbFlex {
        chase: Tuning::new(0.42, 0.86),
        amount: 0.20,
        at: 2.3,
        width: 1.0,
        settle: Tuning::new(0.20, 0.95),
    };

    /// Returns the flex for a material of `liquidity` (see
    /// [`Material::liquidity`](crate::Material::liquidity)). 0.5 keeps it as
    /// it is, 0 makes it rigid, 1 stretches more and wobbles longer.
    pub fn liquid(self, liquidity: f32) -> ThumbFlex {
        let l = liquidity.clamp(0.0, 1.0);
        let (amount, response, damping) = if l <= 0.5 {
            let t = l / 0.5;
            (self.amount * t, lerp(0.25, self.chase.response, t), lerp(1.0, self.chase.damping, t))
        } else {
            let t = (l - 0.5) / 0.5;
            (
                self.amount * (1.0 + 0.6 * t),
                self.chase.response * (1.0 + 1.4 * t),
                self.chase.damping * (1.0 - 0.35 * t),
            )
        };
        ThumbFlex { chase: Tuning::new(response, damping), amount, ..self }
    }
}

impl Thumb {
    /// `raised` keeps the droplet lifted. `rest` and `lifted` are its sizes.
    pub fn step(
        &mut self,
        ctx: &mut Context,
        target: f32,
        follow: Tuning,
        raised: bool,
        rest: Vec2,
        lifted: Vec2,
    ) -> ThumbFrame {
        // Larger controls travel further for the same feel.
        let k = (rest.y / 24.0).clamp(1.0, 3.0);
        let flex =
            ThumbFlex { at: ThumbFlex::THUMB.at * k, ..ThumbFlex::THUMB }.liquid(ctx.theme.lens.liquidity);
        self.step_flex(ctx, target, follow, raised, rest, lifted, flex)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn step_flex(
        &mut self,
        ctx: &mut Context,
        target: f32,
        follow: Tuning,
        raised: bool,
        rest: Vec2,
        lifted: Vec2,
        flex: ThumbFlex,
    ) -> ThumbFrame {
        let dt = ctx.dt;
        let follow = ctx.tuning(follow);
        let mut moving = self.pos.step(target, follow, dt);
        // Stay up until nearly home.
        let far = (self.pos.value - target).abs() > rest.x * 0.18;
        let up = raised || (far && self.lift.value > 0.3);
        let t = if up { Tuning::new(0.27, 0.72) } else { flex.settle };
        moving |= self.lift.step(up as u32 as f32, ctx.tuning(t), dt);
        moving |= self.chase.step(self.pos.velocity, flex.chase, dt);
        if moving {
            ctx.request_repaint();
        }
        let lift = self.lift.value.max(0.0);
        let u = self.chase.velocity / 1000.0;
        let s = if ctx.theme.reduce_motion { 0.0 } else { flex.amount * (u / flex.at).tanh() };
        let base = rest.lerp(lifted, lift);
        let w = 1.0 + s * (0.2 + 0.8 * lift.min(1.0));
        ThumbFrame {
            size: vec2(base.x * w.powf(flex.width), base.y / w),
            base,
            lift,
            bend: (self.pos.velocity / 1800.0).clamp(-0.35, 0.35),
            at_rest: !moving && !raised,
        }
    }
}

/// Where the thumb is drawn.
pub fn thumb_rect(ctx: &Context, center: Vec2, f: &ThumbFrame) -> Rect {
    let r = Rect::from_center_size(center, f.size);
    if f.at_rest { r.snap(ctx.pixels_per_point) } else { r }
}

/// Glass of a droplet: solid knob at rest, clear lens when lifted. `k` is the
/// scale of the control.
pub fn thumb_glass(ctx: &Context, rect: Rect, f: &ThumbFrame, knob: Color, k: f32) -> Glass {
    let mut lens = ctx.theme.lens.scaled(k);
    // Less clear glass: a light veil and a calmer bend.
    let veil = ctx.theme.tint_above_default() * 0.75;
    lens.light_range.0 = lerp(lens.light_range.0, 0.62, veil);
    lens.dark_range.0 = lerp(lens.dark_range.0, 0.30, veil);
    lens.refraction *= 1.0 - 0.35 * veil;
    lens.dispersion *= 1.0 - 0.6 * veil;
    let mut rest = lens;
    rest.refraction = 0.0;
    rest.outer_refraction = 0.0;
    rest.dispersion = 0.0;
    rest.blur = 9.0;
    rest.inner_glow = 0.0;
    rest.inner_shadow = 0.0;
    rest.dark_edge = 0.0;
    rest.rim = 1.0;
    rest.rim_base = 0.3;
    rest.rim_width = 0.6 * k.max(1.0).sqrt();
    rest.light_spill = 0.0;
    rest.shadow = 0.15;
    rest.shadow_radius = 9.0 * k;
    rest.shadow_y = 2.5 * k;
    rest.shadow_tint = 0.0;
    rest.tint = Some(knob);
    rest.tint_follow = 0.0;
    let t = f.lift.clamp(0.0, 1.0);
    let mut m = rest.lerp(&lens, t);
    // The knob turns clear early in the lift, the lens keeps growing after.
    m.tint = Some(knob.fade((1.0 - t * 1.35).max(0.0).powf(1.2)));
    m.tint_follow = 0.0;
    m.blur = lerp(9.0, lens.blur, (t * 1.1).min(1.0));
    let mut g = Glass::new(rect, Corner::Capsule, m);
    g.light_bend = f.bend;
    g.darkness = ctx.theme.dark as u32 as f32;
    g
}

pub fn load_thumb(ctx: &mut Context, id: Id) -> Thumb {
    ctx.get(id.with("droplet"))
}
pub fn store_thumb(ctx: &mut Context, id: Id, d: Thumb) {
    ctx.set(id.with("droplet"), d);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Event;

    fn run(path: impl Fn(f32) -> f32, n: usize) -> Vec<Vec2> {
        let mut ctx = Context::new();
        let mut d = Thumb::default();
        let dt = 1.0 / 60.0;
        (0..n)
            .map(|i| {
                let t = i as f32 * dt;
                ctx.begin_frame(vec2(800.0, 200.0), 1.0, t as f64, &[] as &[Event]);
                let f = d.step(
                    &mut ctx,
                    path(t),
                    Tuning::new(0.085, 0.95),
                    true,
                    vec2(37.0, 24.0),
                    vec2(50.0, 34.0),
                );
                ctx.end_frame();
                f.size
            })
            .collect()
    }

    #[test]
    fn area_is_conserved_once_lifted() {
        let sizes = run(|t| 200.0 + 120.0 * (t * 6.0).sin(), 120);
        for s in &sizes[60..] {
            assert!((s.x * s.y / (50.0 * 34.0) - 1.0).abs() < 0.02, "{s:?}");
        }
    }

    #[test]
    fn turning_around_stretches_and_squeezes() {
        let sizes = run(|t| 200.0 + 120.0 * (t * 6.0).sin(), 240);
        let w: Vec<f32> = sizes[120..].iter().map(|s| s.x).collect();
        let max = w.iter().cloned().fold(0.0, f32::max);
        let min = w.iter().cloned().fold(1e9, f32::min);
        assert!(max > 52.0 && min < 48.0, "{min} {max}");
        assert!(max < 50.0 * 1.17 && min > 50.0 * 0.83);
    }

    #[test]
    fn blob_stretches_and_settles_without_a_wobble() {
        // The iOS 27 blob keeps its area as it stretches, and is round again
        // a moment after it stops.
        let mut ctx = Context::new();
        let mut d = Thumb::default();
        let lifted = vec2(116.0, 82.0);
        let (mut lo, mut hi) = (lifted, lifted);
        let mut settled_at = None;
        for i in 0..240 {
            let t = i as f32 / 60.0;
            ctx.begin_frame(vec2(800.0, 200.0), 1.0, t as f64, &[] as &[Event]);
            // Two quick sweeps, then a stop at frame 120.
            let x =
                if i < 120 { 200.0 + 150.0 * (t * 6.0).sin() } else { 200.0 + 150.0 * (2.0f32 * 6.0).sin() };
            let f = d.step_flex(
                &mut ctx,
                x,
                Tuning::new(0.14, 0.88),
                true,
                vec2(86.0, 54.0),
                lifted,
                ThumbFlex::BLOB,
            );
            ctx.end_frame();
            if (30..120).contains(&i) {
                lo = lo.min(f.size);
                hi = hi.max(f.size);
                assert!((f.size.x * f.size.y / (lifted.x * lifted.y) - 1.0).abs() < 0.02, "{:?}", f.size);
            }
            // The last frame it is still out of shape.
            if i >= 120 && (f.size - lifted).length() >= 1.0 {
                settled_at = Some(i - 120);
            }
        }
        assert!(hi.x - lo.x > 10.0, "{lo:?} {hi:?}");
        assert!(hi.x < lifted.x * 1.25 && lo.y > lifted.y * 0.8, "{lo:?} {hi:?}");
        let settled = settled_at.unwrap_or(0);
        assert!(settled < 24, "settled after {settled} frames");
    }

    #[test]
    fn rubber_never_exceeds_its_limit() {
        for d in [-1e4, -10.0, 0.0, 3.0, 1e5] {
            assert!(rubber_band(d, 12.0).abs() <= 12.0);
        }
    }
}
