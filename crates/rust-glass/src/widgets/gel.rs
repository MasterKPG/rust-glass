//! The two motion models behind every control.
//!
//! [`Gel`] is the press response of a surface: it swells, stretches towards the
//! finger and lights up from the point of contact. [`Droplet`] is the thumb of
//! sliders, switches and segmented controls: an opaque knob that lifts into a
//! clear lens, stretches with speed and squashes when it stops.

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
    glow: Spring,
    at: Vec2,
}

#[derive(Clone, Copy, Debug)]
pub struct Gel {
    pub rect: Rect,
    pub scale: f32,
    pub press: f32,
    pub hover: f32,
    pub glow_at: Vec2,
    pub glow: f32,
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
        let mut moving = s.press.step(
            if held { if inside { 1.0 } else { 0.55 } } else { 0.0 },
            ctx.tune(if held { Tuning::new(0.22, 0.75) } else { Tuning::new(0.42, 0.52) }),
            dt,
        );
        moving |= s.pull.step(pull, ctx.tune(Tuning::new(if held { 0.16 } else { 0.4 }, 0.6)), dt);
        moving |= s.hover.step(r.hovered as u32 as f32, Tuning::SNAP, dt);
        moving |= s.glow.step(inside as u32 as f32, Tuning::new(if inside { 0.18 } else { 0.55 }, 1.0), dt);
        if r.hovered
            && let Some(p) = r.pointer
        {
            s.at = p;
        }
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
            rect,
            scale: 1.0 + grow / size.y.max(1.0) * 0.45,
            press: s.press.x,
            hover: s.hover.x,
            glow_at: s.at,
            glow: s.glow.x.max(0.0) * 0.34 + s.hover.x * 0.07,
            bend: p.x * 0.018,
        }
    }

    pub fn apply(&self, g: &mut Glass) {
        g.glow = Some((self.glow_at, self.glow));
        g.light_bend = self.bend;
        g.material.refraction *= 1.0 + 0.25 * self.press.max(0.0);
        g.material.shadow *= 1.0 + 0.35 * self.press.max(0.0);
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Droplet {
    /// Center along the axis of travel.
    pub pos: Spring,
    pub lift: Spring,
    stretch: Spring,
}

#[derive(Clone, Copy, Debug)]
pub struct DropletFrame {
    pub size: Vec2,
    pub lift: f32,
    pub bend: f32,
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
        let dt = ctx.dt;
        let follow = ctx.tune(follow);
        let mut moving = self.pos.step(target, follow, dt);
        // Stay up until nearly home.
        let far = (self.pos.x - target).abs() > rest.x * 0.18;
        let up = raised || (far && self.lift.x > 0.3);
        let t = if up { Tuning::new(0.26, 0.74) } else { Tuning::new(0.36, 0.72) };
        moving |= self.lift.step(up as u32 as f32, ctx.tune(t), dt);
        let speed = (self.pos.v.abs() / 950.0).min(1.0);
        moving |= self.stretch.step(speed, ctx.tune(Tuning::new(0.30, 0.36)), dt);
        if moving {
            ctx.request_repaint();
        }
        let lift = self.lift.x.max(0.0);
        let s = if ctx.theme.reduce_motion { 0.0 } else { self.stretch.x };
        let base = rest.lerp(lifted, lift);
        let amount = 0.35 + 0.65 * lift.min(1.0);
        let size = vec2(base.x * (1.0 + 0.24 * s * amount), base.y * (1.0 - 0.18 * s * amount));
        DropletFrame { size, lift, bend: (self.pos.v / 1500.0).clamp(-0.5, 0.5) }
    }
}

/// Glass of a droplet: solid knob at rest, clear lens when lifted. `k` is the
/// scale of the control.
pub fn droplet_glass(ctx: &Context, rect: Rect, f: &DropletFrame, knob: Color, k: f32) -> Glass {
    let lens = ctx.theme.lens.scaled(k);
    let mut rest = lens;
    rest.refraction = 0.0;
    rest.outer_refraction = 0.0;
    rest.chroma = 0.0;
    rest.blur = 9.0;
    rest.inner_glow = 0.0;
    rest.inner_shadow = 0.0;
    rest.dark_edge = 0.0;
    rest.rim = 1.0;
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
    g
}

pub fn droplet_state(ctx: &mut Context, id: Id) -> Droplet {
    ctx.get(id.with("droplet"))
}
pub fn set_droplet(ctx: &mut Context, id: Id, d: Droplet) {
    ctx.set(id.with("droplet"), d);
}
