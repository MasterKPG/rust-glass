//! Press response shared by buttons and other controls: the surface swells and
//! stretches towards the pointer.

use crate::animation::{Spring, Spring2d, Tuning};
use crate::context::{Context, Response};
use crate::geometry::{Rect, Vec2, vec2};
use crate::material::Glass;

/// Returns how far an elastic edge gives when pulled by `distance`. The
/// result approaches `limit` but never reaches it.
pub fn rubber_band(distance: f32, limit: f32) -> f32 {
    let a = distance.abs() * 0.55;
    distance.signum() * (1.0 - 1.0 / (a / limit + 1.0)) * limit
}

#[derive(Clone, Copy, Default)]
struct PressState {
    press: Spring,
    pull: Spring2d,
    hover: Spring,
}

/// The press response of a control in this frame: the surface swells and
/// stretches towards the pointer. Use it for your own controls.
///
/// ```
/// # use rust_glass::prelude::*;
/// # use rust_glass::widgets::press::PressEffect;
/// # fn f(ui: &mut Ui) {
/// let response = ui.allocate_response(vec2(44.0, 44.0), Sense::Click);
/// let press = PressEffect::update(ui.ctx, &response, 1.0);
/// let fill = ui.surface.fill_color();
/// ui.painter().fill(press.rect, Corner::Capsule, fill);
/// # }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct PressEffect {
    /// Where to draw the surface. It equals the widget rectangle at rest.
    pub rect: Rect,
    /// Scale for the content, so it grows with the surface.
    pub scale: f32,
    /// How far the press has gone, from 0 to about 1.
    pub amount: f32,
    /// For fills and menu rows. Glass itself does not react to hover.
    pub hover: f32,
    /// Rotation for the highlights while pulled sideways, in radians.
    pub bend: f32,
}

impl PressEffect {
    /// Updates the press state of the widget behind `response`. `strength`
    /// scales the effect, 0 turns it off.
    pub fn update(response: &mut Context, r: &Response, strength: f32) -> PressEffect {
        let mut s: PressState = response.get(r.id);
        let dt = response.dt;
        let held = r.held;
        let rest = r.rect;
        let motion = if response.theme.reduce_motion { 0.0 } else { strength };
        let mut pull = Vec2::ZERO;
        if held && let Some(p) = r.pointer {
            let d = p - response.input.press_origin;
            let lim = (rest.height() * 0.42).clamp(8.0, 18.0);
            pull = vec2(rubber_band(d.x, lim), rubber_band(d.y, lim));
        }
        let inside = held && r.hovered;
        let mut shaped = s.press.step(
            if held { if inside { 1.0 } else { 0.55 } } else { 0.0 },
            response.tuning(if held { Tuning::new(0.24, 0.78) } else { Tuning::new(0.44, 0.56) }),
            dt,
        );
        shaped |= s.pull.step(pull, response.tuning(Tuning::new(if held { 0.16 } else { 0.42 }, 0.62)), dt);
        let moving = s.hover.step(r.hovered as u32 as f32, Tuning::SNAP, dt) | shaped;
        if moving {
            response.request_repaint();
        }
        response.set(r.id, s);

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
            amount: s.press.value,
            hover: s.hover.value,
            bend: p.x * 0.018,
        }
    }

    /// Applies the effect to the glass of the widget.
    pub fn apply(&self, glass: &mut Glass) {
        let press = self.amount.max(0.0);
        glass.brightness = 0.05 * press;
        glass.light_bend = self.bend;
        glass.material.refraction *= 1.0 + 0.25 * press;
        glass.material.shadow *= 1.0 + 0.35 * press;
    }
}
