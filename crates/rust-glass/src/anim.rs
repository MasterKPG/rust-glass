//! Springs. Every moving thing in the framework is a spring, so motion can be
//! interrupted and retargeted at any moment without a visible jump.

use crate::geom::{Rect, Vec2, vec2};

/// Spring tuning in the SwiftUI convention: `response` is the undamped period
/// in seconds, `damping` the damping ratio (1 is critically damped).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    pub response: f32,
    pub damping: f32,
}

impl Tuning {
    pub const fn new(response: f32, damping: f32) -> Self {
        Self { response, damping }
    }
    /// Fast, no overshoot. Color and opacity changes.
    pub const SNAP: Tuning = Tuning::new(0.22, 1.0);
    /// Default for position and size.
    pub const SMOOTH: Tuning = Tuning::new(0.38, 0.86);
    /// Gel: lifts, presses and stretches.
    pub const GEL: Tuning = Tuning::new(0.36, 0.62);
    /// Pronounced wobble, for menus and drops.
    pub const BOUNCY: Tuning = Tuning::new(0.48, 0.68);
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spring {
    pub x: f32,
    pub v: f32,
    started: bool,
}

impl Spring {
    pub fn at(x: f32) -> Self {
        Self { x, v: 0.0, started: true }
    }

    /// Advance towards `target`. The first call snaps to the target, so a
    /// widget never animates in from zero by accident. Returns `true` while
    /// still moving.
    ///
    /// Uses the closed form solution of the damped oscillator, so the result
    /// does not depend on the frame rate and cannot blow up on a long frame.
    pub fn step(&mut self, target: f32, t: Tuning, dt: f32) -> bool {
        if !self.started {
            *self = Spring::at(target);
            return false;
        }
        let w = std::f32::consts::TAU / t.response.max(0.01);
        let z = t.damping.clamp(0.05, 1.0);
        let x0 = self.x - target;
        let v0 = self.v;
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
        self.x = target + x;
        self.v = v;
        let scale = target.abs().max(1.0);
        if x.abs() < 0.0008 * scale && v.abs() < 0.01 * scale {
            self.x = target;
            self.v = 0.0;
            return false;
        }
        true
    }

    /// Jump without animating.
    pub fn set(&mut self, x: f32) {
        *self = Spring::at(x);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spring2 {
    pub x: Spring,
    pub y: Spring,
}

impl Spring2 {
    pub fn at(p: Vec2) -> Self {
        Self { x: Spring::at(p.x), y: Spring::at(p.y) }
    }
    pub fn pos(&self) -> Vec2 {
        vec2(self.x.x, self.y.x)
    }
    pub fn vel(&self) -> Vec2 {
        vec2(self.x.v, self.y.v)
    }
    pub fn step(&mut self, target: Vec2, t: Tuning, dt: f32) -> bool {
        self.x.step(target.x, t, dt) | self.y.step(target.y, t, dt)
    }
    pub fn set(&mut self, p: Vec2) {
        *self = Self::at(p);
    }
}

/// A rectangle whose center and size run on separate springs, which is what
/// gives morphing surfaces their liquid look.
#[derive(Clone, Copy, Debug, Default)]
pub struct SpringRect {
    pub center: Spring2,
    pub size: Spring2,
}

impl SpringRect {
    pub fn at(r: Rect) -> Self {
        Self { center: Spring2::at(r.center()), size: Spring2::at(r.size()) }
    }
    pub fn step(&mut self, target: Rect, pos: Tuning, size: Tuning, dt: f32) -> bool {
        self.center.step(target.center(), pos, dt) | self.size.step(target.size(), size, dt)
    }
    pub fn rect(&self) -> Rect {
        Rect::from_center_size(self.center.pos(), self.size.pos().max(Vec2::splat(1.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_step_snaps() {
        let mut s = Spring::default();
        assert!(!s.step(5.0, Tuning::GEL, 0.016));
        assert_eq!(s.x, 5.0);
    }

    #[test]
    fn frame_rate_independent() {
        let run = |dt: f32, n: usize| {
            let mut s = Spring::at(0.0);
            for _ in 0..n {
                s.step(1.0, Tuning::GEL, dt);
            }
            s.x
        };
        assert!((run(1.0 / 60.0, 12) - run(1.0 / 240.0, 48)).abs() < 1e-3);
    }

    #[test]
    fn settles_and_overshoots_only_when_underdamped() {
        let peak = |t: Tuning| {
            let mut s = Spring::at(0.0);
            let mut peak: f32 = 0.0;
            for _ in 0..400 {
                s.step(1.0, t, 1.0 / 120.0);
                peak = peak.max(s.x);
            }
            assert_eq!(s.x, 1.0);
            peak
        };
        assert!(peak(Tuning::GEL) > 1.02);
        assert!(peak(Tuning::SNAP) <= 1.0001);
    }

    #[test]
    fn survives_huge_frames() {
        let mut s = Spring::at(0.0);
        s.step(100.0, Tuning::BOUNCY, 5.0);
        assert!((s.x - 100.0).abs() < 1.0);
    }
}
