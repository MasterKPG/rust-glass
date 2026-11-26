//! Press response shared by buttons and other controls: the surface swells and
//! stretches towards the pointer.

use crate::animation::{Spring, Spring2d, Tuning};
use crate::context::{Context, Response};
use crate::geometry::{Rect, Vec2, vec2};
use crate::material::Glass;

/// Distance that approaches `limit` as `d` grows.
pub fn rubber_band(d: f32, limit: f32) -> f32 {
    let a = d.abs() * 0.55;
    d.signum() * (1.0 - 1.0 / (a / limit + 1.0)) * limit
}

#[derive(Clone, Copy, Default)]
struct PressState {
    press: Spring,
    pull: Spring2d,
    hover: Spring,
}

#[derive(Clone, Copy, Debug)]
pub struct PressEffect {
    pub rect: Rect,
    pub scale: f32,
    pub press: f32,
    /// For fills and menu rows. Glass itself does not react to hover.
    pub hover: f32,
    pub bend: f32,
}

impl PressEffect {
    pub fn update(ctx: &mut Context, r: &Response, strength: f32) -> PressEffect {
        let mut s: PressState = ctx.get(r.id);
        let dt = ctx.dt;
        let held = r.held;
        let rest = r.rect;
        let motion = if ctx.theme.reduce_motion { 0.0 } else { strength };
        let mut pull = Vec2::ZERO;
        if held && let Some(p) = r.pointer {
            let d = p - ctx.input.press_origin;
            let lim = (rest.height() * 0.42).clamp(8.0, 18.0);
            pull = vec2(rubber_band(d.x, lim), rubber_band(d.y, lim));
        }
        let inside = held && r.hovered;
        let mut shaped = s.press.step(
            if held { if inside { 1.0 } else { 0.55 } } else { 0.0 },
            ctx.tuning(if held { Tuning::new(0.24, 0.78) } else { Tuning::new(0.44, 0.56) }),
            dt,
        );
        shaped |= s.pull.step(pull, ctx.tuning(Tuning::new(if held { 0.16 } else { 0.42 }, 0.62)), dt);
        let moving = s.hover.step(r.hovered as u32 as f32, Tuning::SNAP, dt) | shaped;
        if moving {
            ctx.request_repaint();
        }
        ctx.set(r.id, s);

        let size = rest.size();
        // Grow by a fixed amount rather than a factor, so wide controls do not balloon.
        let grow = (size.y * 0.16).min(7.0) * s.press.value * motion;
        let p = s.pull.value() * motion;
        let stretch = vec2(p.x.abs() * 0.55 - p.y.abs() * 0.22, p.y.abs() * 0.55 - p.x.abs() * 0.22);
        let rect = Rect::from_center_size(
            rest.center() + p * 0.5,
            (size + Vec2::splat(grow) + stretch).max(Vec2::splat(4.0)),
        );
        PressEffect {
            // Exact at rest, so edges stay on whole pixels.
            rect: if shaped || held { rect } else { rest },
            scale: 1.0 + grow / size.y.max(1.0) * 0.45,
            press: s.press.value,
            hover: s.hover.value,
            bend: p.x * 0.018,
        }
    }

    pub fn apply(&self, g: &mut Glass) {
        let press = self.press.max(0.0);
        g.brightness = 0.05 * press;
        g.light_bend = self.bend;
        g.material.refraction *= 1.0 + 0.25 * press;
        g.material.shadow *= 1.0 + 0.35 * press;
    }
}
