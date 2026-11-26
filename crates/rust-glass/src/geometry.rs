//! Points, sizes and rectangles. All UI code works in logical points; the
//! painter converts to physical pixels.

use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

pub const fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

impl Vec2 {
    pub const ZERO: Vec2 = vec2(0.0, 0.0);

    pub const fn splat(v: f32) -> Self {
        vec2(v, v)
    }
    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }
    pub fn normalized(self) -> Self {
        let l = self.length();
        if l > 1e-6 { self / l } else { Vec2::ZERO }
    }
    pub fn dot(self, o: Vec2) -> f32 {
        self.x * o.x + self.y * o.y
    }
    pub fn min(self, o: Vec2) -> Self {
        vec2(self.x.min(o.x), self.y.min(o.y))
    }
    pub fn max(self, o: Vec2) -> Self {
        vec2(self.x.max(o.x), self.y.max(o.y))
    }
    pub fn lerp(self, o: Vec2, t: f32) -> Self {
        self + (o - self) * t
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        vec2(self.x + o.x, self.y + o.y)
    }
}
impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        vec2(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, s: f32) -> Vec2 {
        vec2(self.x * s, self.y * s)
    }
}
impl Div<f32> for Vec2 {
    type Output = Vec2;
    fn div(self, s: f32) -> Vec2 {
        vec2(self.x / s, self.y / s)
    }
}
impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        vec2(-self.x, -self.y)
    }
}
impl AddAssign for Vec2 {
    fn add_assign(&mut self, o: Vec2) {
        *self = *self + o;
    }
}
impl SubAssign for Vec2 {
    fn sub_assign(&mut self, o: Vec2) {
        *self = *self - o;
    }
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

pub const fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect { min: vec2(x, y), max: vec2(x + w, y + h) }
}

impl Rect {
    pub const EVERYTHING: Rect = Rect { min: vec2(-1e6, -1e6), max: vec2(1e6, 1e6) };
    /// Inverted rectangle, the identity for [`Rect::union`].
    pub const NOTHING: Rect = Rect { min: vec2(1e6, 1e6), max: vec2(-1e6, -1e6) };

    pub fn from_min_size(min: Vec2, size: Vec2) -> Self {
        Rect { min, max: min + size }
    }
    pub fn from_center_size(c: Vec2, size: Vec2) -> Self {
        Rect { min: c - size * 0.5, max: c + size * 0.5 }
    }
    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }
    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }
    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }
    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }
    pub fn left(&self) -> f32 {
        self.min.x
    }
    pub fn right(&self) -> f32 {
        self.max.x
    }
    pub fn top(&self) -> f32 {
        self.min.y
    }
    pub fn bottom(&self) -> f32 {
        self.max.y
    }
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x < self.max.x && p.y >= self.min.y && p.y < self.max.y
    }
    pub fn intersects(&self, o: &Rect) -> bool {
        self.min.x < o.max.x && o.min.x < self.max.x && self.min.y < o.max.y && o.min.y < self.max.y
    }
    pub fn intersect(&self, o: Rect) -> Rect {
        Rect { min: self.min.max(o.min), max: self.max.min(o.max) }
    }
    pub fn union(&self, o: Rect) -> Rect {
        Rect { min: self.min.min(o.min), max: self.max.max(o.max) }
    }
    pub fn is_positive(&self) -> bool {
        self.max.x > self.min.x && self.max.y > self.min.y
    }
    pub fn expand(&self, v: f32) -> Rect {
        self.expand2(Vec2::splat(v))
    }
    pub fn expand2(&self, v: Vec2) -> Rect {
        Rect { min: self.min - v, max: self.max + v }
    }
    pub fn shrink(&self, v: f32) -> Rect {
        self.expand(-v)
    }
    pub fn shrink2(&self, v: Vec2) -> Rect {
        self.expand2(-v)
    }
    pub fn translate(&self, v: Vec2) -> Rect {
        Rect { min: self.min + v, max: self.max + v }
    }
    pub fn scale_from_center(&self, sx: f32, sy: f32) -> Rect {
        Rect::from_center_size(self.center(), vec2(self.width() * sx, self.height() * sy))
    }
    pub fn lerp(&self, o: &Rect, t: f32) -> Rect {
        Rect { min: self.min.lerp(o.min, t), max: self.max.lerp(o.max, t) }
    }
    /// Edges rounded to whole pixels.
    pub fn snap(&self, ppp: f32) -> Rect {
        let f = |v: f32| (v * ppp).round() / ppp;
        Rect { min: vec2(f(self.min.x), f(self.min.y)), max: vec2(f(self.max.x), f(self.max.y)) }
    }
    /// Move, without resizing, so the rectangle lies inside `outer`.
    pub fn clamp_inside(&self, outer: Rect) -> Rect {
        let mut d = Vec2::ZERO;
        if self.max.x > outer.max.x {
            d.x = outer.max.x - self.max.x;
        }
        if self.min.x + d.x < outer.min.x {
            d.x = outer.min.x - self.min.x;
        }
        if self.max.y > outer.max.y {
            d.y = outer.max.y - self.max.y;
        }
        if self.min.y + d.y < outer.min.y {
            d.y = outer.min.y - self.min.y;
        }
        self.translate(d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_with_nothing_is_identity() {
        let r = rect(1.0, 2.0, 3.0, 4.0);
        assert_eq!(Rect::NOTHING.union(r), r);
    }

    #[test]
    fn clamp_inside_moves_without_resizing() {
        let r = rect(90.0, -5.0, 20.0, 10.0).clamp_inside(rect(0.0, 0.0, 100.0, 100.0));
        assert_eq!(r, rect(80.0, 0.0, 20.0, 10.0));
    }
}
