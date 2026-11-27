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
    glow: Spring,
    /// Where the pointer went down, from the center of the control.
    glow_at: Vec2,
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
    /// Light where the pointer pressed: position and strength from 0 to 1.
    /// It rises at once and fades after the release.
    pub glow: Option<(Vec2, f32)>,
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
        // The release settles with a single soft give rather than a wobble,
        // so labels that grow with the surface do not shake on the way down.
        let mut shaped = s.press.step(
            if held { if inside { 1.0 } else { 0.55 } } else { 0.0 },
            response.tuning(if held { Tuning::new(0.24, 0.78) } else { Tuning::new(0.40, 0.80) }),
            dt,
        );
        shaped |= s.pull.step(pull, response.tuning(Tuning::new(if held { 0.16 } else { 0.42 }, 0.62)), dt);
        if held && let Some(p) = r.pointer {
            if r.pressed || s.glow.value < 0.05 {
                s.glow_at = p - rest.center();
            } else {
                // Follows the finger a little, staying on the control.
                let half = rest.size() * 0.5;
                let to = (p - rest.center()).max(-half).min(half);
                s.glow_at = s.glow_at.lerp(to, 0.25);
            }
        }
        let glowing = s.glow.step(
            held as u32 as f32,
            if held { Tuning::new(0.16, 1.0) } else { Tuning::new(0.55, 1.0) },
            dt,
        );
        let moving = s.hover.step(r.hovered as u32 as f32, Tuning::SNAP, dt) | shaped | glowing;
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
            // Content never shrinks below its size at rest, even while the
            // surface gives a little past it.
            scale: 1.0 + grow.max(0.0) / size.y.max(1.0) * 0.45,
            amount: s.press.value,
            hover: s.hover.value,
            bend: p.x * 0.018,
            glow: (s.glow.value > 0.003 && strength > 0.0).then(|| {
                let k = rect.size().x / size.x.max(1.0);
                (rect.center() + s.glow_at * k, s.glow.value.clamp(0.0, 1.0))
            }),
        }
    }

    /// Applies the effect to the glass of the widget.
    pub fn apply(&self, glass: &mut Glass) {
        let press = self.amount.max(0.0);
        glass.brightness = 0.02 * press;
        glass.glow = self.glow;
        glass.light_bend = self.bend;
        glass.material.refraction *= 1.0 + 0.25 * press;
        glass.material.shadow *= 1.0 + 0.35 * press;
    }
}
