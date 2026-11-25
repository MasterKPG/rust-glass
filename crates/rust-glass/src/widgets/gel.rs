//! The two motion models behind every control.
//!
//! [`Gel`] is the press response of a surface: it swells and stretches towards
//! the finger. [`Droplet`] is the thumb of sliders, switches and segmented
//! controls: an opaque knob that lifts into a clear lens and changes shape with
//! acceleration.

use crate::anim::{Spring, Spring2, Tuning};
use crate::color::Color;
use crate::context::{Context, Id, Response};
use crate::geom::{Rect, Vec2, lerp, vec2};
use crate::material::{Corner, Glass};

/// Distance that approaches `limit` as `d` grows.
pub fn rubber(d: f32, limit: f32) -> f32 {
    let a = d.abs() * 0.55;
    d.signum() * (1.0 - 1.0 / (a / limit + 1.0)) * limit
}

#[derive(Clone, Copy, Default)]
struct GelState {
    press: Spring,
    pull: Spring2,
    hover: Spring,
}

#[derive(Clone, Copy, Debug)]
pub struct Gel {
    pub rect: Rect,
    pub scale: f32,
    pub press: f32,
    /// For fills and menu rows. Glass itself does not react to hover.
    pub hover: f32,
    pub bend: f32,
}

impl Gel {
    pub fn update(ctx: &mut Context, r: &Response, strength: f32) -> Gel {
        let mut s: GelState = ctx.get(r.id);
        let dt = ctx.dt;
        let held = r.held;
        let rest = r.rect;
        let motion = if ctx.theme.reduce_motion { 0.0 } else { strength };
        let mut pull = Vec2::ZERO;
        if held && let Some(p) = r.pointer {
            let d = p - ctx.input.press_origin;
            let lim = (rest.height() * 0.42).clamp(8.0, 18.0);
            pull = vec2(rubber(d.x, lim), rubber(d.y, lim));
        }
        let inside = held && r.hovered;
        let mut shaped = s.press.step(
            if held { if inside { 1.0 } else { 0.55 } } else { 0.0 },
            ctx.tune(if held { Tuning::new(0.24, 0.78) } else { Tuning::new(0.44, 0.56) }),
            dt,
        );
        shaped |= s.pull.step(pull, ctx.tune(Tuning::new(if held { 0.16 } else { 0.42 }, 0.62)), dt);
        let moving = s.hover.step(r.hovered as u32 as f32, Tuning::SNAP, dt) | shaped;
        if moving {
            ctx.request_repaint();
        }
        ctx.set(r.id, s);

        let size = rest.size();
        // Grow by a fixed amount rather than a factor, so wide controls do not balloon.
        let grow = (size.y * 0.16).min(7.0) * s.press.x * motion;
        let p = s.pull.pos() * motion;
        let stretch = vec2(p.x.abs() * 0.55 - p.y.abs() * 0.22, p.y.abs() * 0.55 - p.x.abs() * 0.22);
        let rect = Rect::from_center_size(
            rest.center() + p * 0.5,
            (size + Vec2::splat(grow) + stretch).max(Vec2::splat(4.0)),
        );
        Gel {
            // Exact at rest, so edges stay on whole pixels.
            rect: if shaped || held { rect } else { rest },
            scale: 1.0 + grow / size.y.max(1.0) * 0.45,
            press: s.press.x,
            hover: s.hover.x,
            bend: p.x * 0.018,
        }
    }

    pub fn apply(&self, g: &mut Glass) {
        let press = self.press.max(0.0);
        g.lit = 0.05 * press;
        g.light_bend = self.bend;
        g.material.refraction *= 1.0 + 0.25 * press;
        g.material.shadow *= 1.0 + 0.35 * press;
    }
}

/// State of a thumb. Positions are relative to the control, so moving the
/// control (dragging its window, scrolling) does not move the thumb inside it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Droplet {
    /// Center along the axis of travel, from the left edge of the control.
    pub pos: Spring,
    pub lift: Spring,
    /// Follows the velocity; its own rate of change is the smoothed
    /// acceleration that shapes the drop.
    chase: Spring,
}

#[derive(Clone, Copy, Debug)]
pub struct DropletFrame {
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
pub struct Flex {
    /// Smooths the acceleration.
    pub chase: Tuning,
    /// Change of the height at the limit.
    pub amount: f32,
    /// Acceleration that reaches three quarters of the limit, in 1000 points
    /// per second squared.
    pub at: f32,
    /// Part of the change the width takes on. 1 keeps the area.
    pub width: f32,
}

impl Flex {
    /// Thumbs, fitted to the reference footage.
    pub const THUMB: Flex = Flex { chase: Tuning::new(0.628, 0.5), amount: 0.159, at: 2.4, width: 1.0 };
    /// The blob of a tab bar: softer, and the width gives less than the height.
    pub const BLOB: Flex = Flex { chase: Tuning::new(1.227, 0.625), amount: 0.219, at: 2.3, width: 0.6 };
}

impl Droplet {
    /// `raised` keeps the droplet lifted. `rest` and `lifted` are its sizes.
    pub fn step(
        &mut self,
        ctx: &mut Context,
        target: f32,
        follow: Tuning,
        raised: bool,
        rest: Vec2,
        lifted: Vec2,
    ) -> DropletFrame {
        // Larger controls travel further for the same feel.
        let k = (rest.y / 24.0).clamp(1.0, 3.0);
        let flex = Flex { at: Flex::THUMB.at * k, ..Flex::THUMB };
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
        flex: Flex,
    ) -> DropletFrame {
        let dt = ctx.dt;
        let follow = ctx.tune(follow);
        let mut moving = self.pos.step(target, follow, dt);
        // Stay up until nearly home.
        let far = (self.pos.x - target).abs() > rest.x * 0.18;
        let up = raised || (far && self.lift.x > 0.3);
        let t = if up { Tuning::new(0.27, 0.72) } else { Tuning::new(0.36, 0.76) };
        moving |= self.lift.step(up as u32 as f32, ctx.tune(t), dt);
        moving |= self.chase.step(self.pos.v, flex.chase, dt);
        if moving {
            ctx.request_repaint();
        }
        let lift = self.lift.x.max(0.0);
        let u = self.chase.v / 1000.0;
        let s = if ctx.theme.reduce_motion { 0.0 } else { flex.amount * (u / flex.at).tanh() };
        let base = rest.lerp(lifted, lift);
        let w = 1.0 + s * (0.2 + 0.8 * lift.min(1.0));
        DropletFrame {
            size: vec2(base.x * w.powf(flex.width), base.y / w),
            base,
            lift,
            bend: (self.pos.v / 1800.0).clamp(-0.35, 0.35),
            at_rest: !moving && !raised,
        }
    }
}

/// Where the thumb is drawn.
pub fn droplet_rect(ctx: &Context, center: Vec2, f: &DropletFrame) -> Rect {
    let r = Rect::from_center_size(center, f.size);
    if f.at_rest { r.snap(ctx.ppp) } else { r }
}

/// Glass of a droplet: solid knob at rest, clear lens when lifted. `k` is the
/// scale of the control.
pub fn droplet_glass(ctx: &Context, rect: Rect, f: &DropletFrame, knob: Color, k: f32) -> Glass {
    let mut lens = ctx.theme.lens.scaled(k);
    // Less clear glass: a light veil and a calmer bend.
    let veil = ctx.theme.tinted() * 0.75;
    lens.light_range.0 = lerp(lens.light_range.0, 0.62, veil);
    lens.dark_range.0 = lerp(lens.dark_range.0, 0.30, veil);
    lens.refraction *= 1.0 - 0.35 * veil;
    lens.chroma *= 1.0 - 0.6 * veil;
    let mut rest = lens;
    rest.refraction = 0.0;
    rest.outer_refraction = 0.0;
    rest.chroma = 0.0;
    rest.blur = 9.0;
    rest.inner_glow = 0.0;
    rest.inner_shadow = 0.0;
    rest.dark_edge = 0.0;
    rest.rim = 1.0;
    rest.rim_base = 0.3;
    rest.rim_width = 0.6 * k.max(1.0).sqrt();
    rest.bleed = 0.0;
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

pub fn droplet_state(ctx: &mut Context, id: Id) -> Droplet {
    ctx.get(id.with("droplet"))
}
pub fn set_droplet(ctx: &mut Context, id: Id, d: Droplet) {
    ctx.set(id.with("droplet"), d);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Event;

    fn run(path: impl Fn(f32) -> f32, n: usize) -> Vec<Vec2> {
        let mut ctx = Context::new();
        let mut d = Droplet::default();
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
    fn blob_gives_more_in_height_than_in_width() {
        let mut ctx = Context::new();
        let mut d = Droplet::default();
        let lifted = vec2(104.0, 72.4);
        let (mut lo, mut hi) = (lifted, lifted);
        for i in 0..360 {
            let t = i as f32 / 60.0;
            ctx.begin_frame(vec2(800.0, 200.0), 1.0, t as f64, &[] as &[Event]);
            let x = 200.0 + 150.0 * (t * 3.5).sin();
            let f =
                d.step_flex(&mut ctx, x, Tuning::new(0.14, 0.88), true, vec2(86.0, 54.0), lifted, Flex::BLOB);
            ctx.end_frame();
            if i > 120 {
                lo = lo.min(f.size);
                hi = hi.max(f.size);
            }
        }
        // The extremes of the reference clip are 96 to 113 wide and 63 to 84 high.
        assert!(lo.x > 90.0 && hi.x < 116.0, "{lo:?} {hi:?}");
        assert!(lo.y > 58.0 && hi.y < 94.0, "{lo:?} {hi:?}");
        assert!(hi.y - lo.y > hi.x - lo.x, "{lo:?} {hi:?}");
    }

    #[test]
    fn rubber_never_exceeds_its_limit() {
        for d in [-1e4, -10.0, 0.0, 3.0, 1e5] {
            assert!(rubber(d, 12.0).abs() <= 12.0);
        }
    }
}
