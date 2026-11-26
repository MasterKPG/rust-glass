//! Springs. Everything that moves in the framework runs on a spring, so a
//! motion can be interrupted or retargeted at any time without a jump.
//!
//! Most code animates through [`Context::animate`](crate::Context::animate),
//! which stores the spring for you. Use [`Spring`] directly when you keep the
//! state yourself.

use crate::geometry::{Rect, Vec2, vec2};

/// How a spring moves, in the SwiftUI convention.
///
/// `response` is the period of the undamped spring in seconds: smaller is
/// faster. `damping` is the damping ratio: 1 arrives without overshoot, lower
/// values bounce.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    /// Period of the undamped spring, in seconds.
    pub response: f32,
    /// Damping ratio. 1 is critically damped.
    pub damping: f32,
}

impl Tuning {
    /// Creates a tuning from a response in seconds and a damping ratio.
    pub const fn new(response: f32, damping: f32) -> Self {
        Self { response, damping }
    }
    /// Fast and without overshoot. For color and opacity.
    pub const SNAP: Tuning = Tuning::new(0.22, 1.0);
    /// The default for position and size.
    pub const SMOOTH: Tuning = Tuning::new(0.38, 0.86);
    /// Lifts, presses and stretches.
    pub const PRESS: Tuning = Tuning::new(0.36, 0.62);
    /// A clear wobble, for menus and drops.
    pub const BOUNCY: Tuning = Tuning::new(0.48, 0.68);
}

/// A value that follows its target like a damped spring.
///
/// # Examples
///
/// ```
/// use rust_glass::prelude::*;
///
/// let mut width = Spring::new(100.0);
/// // Called once per frame with the frame time.
/// let moving = width.step(240.0, Tuning::SMOOTH, 1.0 / 60.0);
/// assert!(moving && width.value > 100.0);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spring {
    /// Current value.
    pub value: f32,
    /// Current velocity, in units per second.
    pub velocity: f32,
    started: bool,
}

impl Spring {
    /// Creates a spring resting at `value`.
    pub fn new(value: f32) -> Self {
        Self { value, velocity: 0.0, started: true }
    }

    /// Moves towards `target` by `dt` seconds and returns `true` while still
    /// moving.
    ///
    /// The first step of a [`Default`] spring jumps straight to the target,
    /// so a widget never animates in from zero by accident.
    ///
    /// The step uses the exact solution of the damped oscillator, so the
    /// result is the same at any frame rate and a long frame cannot make it
    /// unstable.
    pub fn step(&mut self, target: f32, tuning: Tuning, dt: f32) -> bool {
        if !self.started {
            *self = Spring::new(target);
            return false;
        }
        let w = std::f32::consts::TAU / tuning.response.max(0.01);
        let z = tuning.damping.clamp(0.05, 1.0);
        let x0 = self.value - target;
        let v0 = self.velocity;
        let (x, v) = if z > 0.999 {
            let c = v0 + w * x0;
            let e = (-w * dt).exp();
            ((x0 + c * dt) * e, (v0 - w * c * dt) * e)
        } else {
            let wd = w * (1.0 - z * z).sqrt();
            let e = (-z * w * dt).exp();
            let (s, c) = (wd * dt).sin_cos();
            let b = (v0 + z * w * x0) / wd;
            (e * (x0 * c + b * s), e * (v0 * c - (z * w * b + wd * x0) * s))
        };
        self.value = target + x;
        self.velocity = v;
        let scale = 1.0 + target.abs() * 0.02;
        if x.abs() < 0.0008 * scale && v.abs() < 0.01 * scale {
            self.value = target;
            self.velocity = 0.0;
            return false;
        }
        true
    }

    /// Jumps to `value` and stops.
    pub fn set(&mut self, value: f32) {
        *self = Spring::new(value);
    }
}

/// Two springs, one per axis.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spring2d {
    /// Spring of the x axis.
    pub x: Spring,
    /// Spring of the y axis.
    pub y: Spring,
}

impl Spring2d {
    /// Creates a spring resting at `value`.
    pub fn new(value: Vec2) -> Self {
        Self { x: Spring::new(value.x), y: Spring::new(value.y) }
    }
    /// Current value.
    pub fn value(&self) -> Vec2 {
        vec2(self.x.value, self.y.value)
    }
    /// Current velocity, in units per second.
    pub fn velocity(&self) -> Vec2 {
        vec2(self.x.velocity, self.y.velocity)
    }
    /// Moves towards `target` and returns `true` while still moving. See
    /// [`Spring::step`].
    pub fn step(&mut self, target: Vec2, tuning: Tuning, dt: f32) -> bool {
        self.x.step(target.x, tuning, dt) | self.y.step(target.y, tuning, dt)
    }
    /// Jumps to `value` and stops.
    pub fn set(&mut self, value: Vec2) {
        *self = Self::new(value);
    }
}

/// A rectangle whose center and size move on separate springs. Surfaces that
/// change shape use it, which gives them their liquid look.
#[derive(Clone, Copy, Debug, Default)]
pub struct SpringRect {
    /// Spring of the center.
    pub center: Spring2d,
    /// Spring of the size.
    pub size: Spring2d,
}

impl SpringRect {
    /// Creates a spring resting at `rect`.
    pub fn new(rect: Rect) -> Self {
        Self { center: Spring2d::new(rect.center()), size: Spring2d::new(rect.size()) }
    }
    /// Moves towards `target` and returns `true` while still moving.
    pub fn step(&mut self, target: Rect, center: Tuning, size: Tuning, dt: f32) -> bool {
        self.center.step(target.center(), center, dt) | self.size.step(target.size(), size, dt)
    }
    /// Current rectangle.
    pub fn rect(&self) -> Rect {
        Rect::from_center_size(self.center.value(), self.size.value().max(Vec2::splat(1.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_step_snaps() {
        let mut s = Spring::default();
        assert!(!s.step(5.0, Tuning::PRESS, 0.016));
        assert_eq!(s.value, 5.0);
    }

    #[test]
    fn frame_rate_independent() {
        let run = |dt: f32, n: usize| {
            let mut s = Spring::new(0.0);
            for _ in 0..n {
                s.step(1.0, Tuning::PRESS, dt);
            }
            s.value
        };
        assert!((run(1.0 / 60.0, 12) - run(1.0 / 240.0, 48)).abs() < 1e-3);
    }

    #[test]
    fn settles_and_overshoots_only_when_underdamped() {
        let peak = |t: Tuning| {
            let mut s = Spring::new(0.0);
            let mut peak: f32 = 0.0;
            for _ in 0..400 {
                s.step(1.0, t, 1.0 / 120.0);
                peak = peak.max(s.value);
            }
            assert_eq!(s.value, 1.0);
            peak
        };
        assert!(peak(Tuning::PRESS) > 1.02);
        assert!(peak(Tuning::SNAP) <= 1.0001);
    }

    #[test]
    fn survives_huge_frames() {
        let mut s = Spring::new(0.0);
        s.step(100.0, Tuning::BOUNCY, 5.0);
        assert!((s.value - 100.0).abs() < 1.0);
    }
}
