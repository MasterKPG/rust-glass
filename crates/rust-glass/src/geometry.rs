//! Points, sizes and rectangles.
//!
//! All UI code works in logical points. The painter converts to physical
//! pixels with [`Context::pixels_per_point`](crate::Context::pixels_per_point).

use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// A point or a size in points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    /// Horizontal component, growing to the right.
    pub x: f32,
    /// Vertical component, growing downwards.
    pub y: f32,
}

/// Creates a [`Vec2`].
pub const fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

impl Vec2 {
    /// `(0, 0)`.
    pub const ZERO: Vec2 = vec2(0.0, 0.0);

    /// Creates a vector with both components set to `value`.
    pub const fn splat(value: f32) -> Self {
        vec2(value, value)
    }
    /// Returns the length.
    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }
    /// Returns a vector of length 1 in the same direction, or zero for a
    /// zero vector.
    pub fn normalized(self) -> Self {
        let l = self.length();
        if l > 1e-6 { self / l } else { Vec2::ZERO }
    }
    /// Returns the dot product.
    pub fn dot(self, other: Vec2) -> f32 {
        self.x * other.x + self.y * other.y
    }
    /// Returns the smaller of each component.
    pub fn min(self, other: Vec2) -> Self {
        vec2(self.x.min(other.x), self.y.min(other.y))
    }
    /// Returns the larger of each component.
    pub fn max(self, other: Vec2) -> Self {
        vec2(self.x.max(other.x), self.y.max(other.y))
    }
    /// Mixes towards `other`: `t` = 0 gives `self`, 1 gives `other`.
    pub fn lerp(self, other: Vec2, t: f32) -> Self {
        self + (other - self) * t
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, other: Vec2) -> Vec2 {
        vec2(self.x + other.x, self.y + other.y)
    }
}
impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, other: Vec2) -> Vec2 {
        vec2(self.x - other.x, self.y - other.y)
    }
}
impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, factor: f32) -> Vec2 {
        vec2(self.x * factor, self.y * factor)
    }
}
impl Div<f32> for Vec2 {
    type Output = Vec2;
    fn div(self, divisor: f32) -> Vec2 {
        vec2(self.x / divisor, self.y / divisor)
    }
}
impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        vec2(-self.x, -self.y)
    }
}
impl AddAssign for Vec2 {
    fn add_assign(&mut self, other: Vec2) {
        *self = *self + other;
    }
}
impl SubAssign for Vec2 {
    fn sub_assign(&mut self, other: Vec2) {
        *self = *self - other;
    }
}

/// Mixes two numbers: `t` = 0 gives `a`, 1 gives `b`.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Returns 0 below `edge0`, 1 above `edge1`, and a smooth S curve between.
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// An axis aligned rectangle in points, from its top left corner `min` to
/// its bottom right corner `max`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// Top left corner.
    pub min: Vec2,
    /// Bottom right corner.
    pub max: Vec2,
}

/// Creates a [`Rect`] from its top left corner and its size.
pub const fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect { min: vec2(x, y), max: vec2(x + width, y + height) }
}

impl Rect {
    /// A rectangle larger than any window. Use it as a clip that clips
    /// nothing.
    pub const EVERYTHING: Rect = Rect { min: vec2(-1e6, -1e6), max: vec2(1e6, 1e6) };
    /// An inverted rectangle, the starting point for [`Rect::union`].
    pub const NOTHING: Rect = Rect { min: vec2(1e6, 1e6), max: vec2(-1e6, -1e6) };

    /// Creates a rectangle from its top left corner and its size.
    pub fn from_min_size(min: Vec2, size: Vec2) -> Self {
        Rect { min, max: min + size }
    }
    /// Creates a rectangle from its center and its size.
    pub fn from_center_size(center: Vec2, size: Vec2) -> Self {
        Rect { min: center - size * 0.5, max: center + size * 0.5 }
    }
    /// Returns the width.
    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }
    /// Returns the height.
    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }
    /// Returns the width and height.
    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }
    /// Returns the center point.
    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }
    /// Returns the x of the left edge.
    pub fn left(&self) -> f32 {
        self.min.x
    }
    /// Returns the x of the right edge.
    pub fn right(&self) -> f32 {
        self.max.x
    }
    /// Returns the y of the top edge.
    pub fn top(&self) -> f32 {
        self.min.y
    }
    /// Returns the y of the bottom edge.
    pub fn bottom(&self) -> f32 {
        self.max.y
    }
    /// Returns `true` if `point` is inside. The right and bottom edges are
    /// outside.
    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.min.x && point.x < self.max.x && point.y >= self.min.y && point.y < self.max.y
    }
    /// Returns `true` if the two rectangles overlap.
    pub fn intersects(&self, other: &Rect) -> bool {
        self.min.x < other.max.x
            && other.min.x < self.max.x
            && self.min.y < other.max.y
            && other.min.y < self.max.y
    }
    /// Returns the overlap of the two rectangles. It is empty, see
    /// [`Rect::is_positive`], when they do not overlap.
    pub fn intersect(&self, other: Rect) -> Rect {
        Rect { min: self.min.max(other.min), max: self.max.min(other.max) }
    }
    /// Returns the smallest rectangle that contains both.
    pub fn union(&self, other: Rect) -> Rect {
        Rect { min: self.min.min(other.min), max: self.max.max(other.max) }
    }
    /// Returns `true` if the width and height are larger than zero.
    pub fn is_positive(&self) -> bool {
        self.max.x > self.min.x && self.max.y > self.min.y
    }
    /// Grows every side by `amount`.
    pub fn expand(&self, amount: f32) -> Rect {
        self.expand2(Vec2::splat(amount))
    }
    /// Grows the left and right sides by `amount.x`, the top and bottom by
    /// `amount.y`.
    pub fn expand2(&self, amount: Vec2) -> Rect {
        Rect { min: self.min - amount, max: self.max + amount }
    }
    /// Shrinks every side by `amount`.
    pub fn shrink(&self, amount: f32) -> Rect {
        self.expand(-amount)
    }
    /// Shrinks the left and right sides by `amount.x`, the top and bottom by
    /// `amount.y`.
    pub fn shrink2(&self, amount: Vec2) -> Rect {
        self.expand2(-amount)
    }
    /// Moves the rectangle by `offset`.
    pub fn translate(&self, offset: Vec2) -> Rect {
        Rect { min: self.min + offset, max: self.max + offset }
    }
    /// Scales width and height around the center.
    pub fn scale_from_center(&self, scale_x: f32, scale_y: f32) -> Rect {
        Rect::from_center_size(self.center(), vec2(self.width() * scale_x, self.height() * scale_y))
    }
    /// Mixes towards `other`: `t` = 0 gives `self`, 1 gives `other`.
    pub fn lerp(&self, other: &Rect, t: f32) -> Rect {
        Rect { min: self.min.lerp(other.min, t), max: self.max.lerp(other.max, t) }
    }
    /// Rounds the edges to whole physical pixels.
    pub fn snap(&self, pixels_per_point: f32) -> Rect {
        let round = |v: f32| (v * pixels_per_point).round() / pixels_per_point;
        Rect {
            min: vec2(round(self.min.x), round(self.min.y)),
            max: vec2(round(self.max.x), round(self.max.y)),
        }
    }
    /// Moves the rectangle, without resizing it, so it lies inside `outer`.
    /// When it is larger than `outer`, its top left corner stays inside.
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
