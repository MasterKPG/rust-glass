//! Non premultiplied color in the sRGB (gamma) encoding. Blending happens in
//! the same space, which is what desktop compositors and Core Animation do.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Color = Color::rgba(0.0, 0.0, 0.0, 0.0);
    pub const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);
    pub const BLACK: Color = Color::rgb(0.0, 0.0, 0.0);
    pub const BLUE: Color = Color::hex(0x0A84FF);
    pub const GREEN: Color = Color::hex(0x34C759);
    pub const RED: Color = Color::hex(0xFF3B30);
    pub const ORANGE: Color = Color::hex(0xFF9500);
    pub const YELLOW: Color = Color::hex(0xFFCC00);
    pub const PINK: Color = Color::hex(0xFF2D6F);
    pub const PURPLE: Color = Color::hex(0xAF52DE);
    pub const TEAL: Color = Color::hex(0x30B0C7);
    pub const INDIGO: Color = Color::hex(0x5856D6);

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }
    pub const fn hex(v: u32) -> Self {
        Self::rgb(((v >> 16) & 255) as f32 / 255.0, ((v >> 8) & 255) as f32 / 255.0, (v & 255) as f32 / 255.0)
    }
    pub const fn gray(v: f32) -> Self {
        Self::rgb(v, v, v)
    }
    pub const fn black(a: f32) -> Self {
        Self::rgba(0.0, 0.0, 0.0, a)
    }
    pub const fn white(a: f32) -> Self {
        Self::rgba(1.0, 1.0, 1.0, a)
    }
    /// Replace the alpha.
    pub const fn alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }
    /// Multiply the alpha.
    pub fn fade(self, f: f32) -> Self {
        Self { a: self.a * f, ..self }
    }
    pub fn lerp(self, o: Color, t: f32) -> Color {
        Color {
            r: self.r + (o.r - self.r) * t,
            g: self.g + (o.g - self.g) * t,
            b: self.b + (o.b - self.b) * t,
            a: self.a + (o.a - self.a) * t,
        }
    }
    pub fn luma(self) -> f32 {
        0.299 * self.r + 0.587 * self.g + 0.114 * self.b
    }
    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
    pub fn to_hex(self) -> u32 {
        let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
        (c(self.r) << 16) | (c(self.g) << 8) | c(self.b)
    }

    pub fn from_hsv(h: f32, s: f32, v: f32) -> Color {
        let h = h.rem_euclid(1.0) * 6.0;
        let i = h.floor();
        let f = h - i;
        let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
        match i as i32 {
            0 => Color::rgb(v, t, p),
            1 => Color::rgb(q, v, p),
            2 => Color::rgb(p, v, t),
            3 => Color::rgb(p, q, v),
            4 => Color::rgb(t, p, v),
            _ => Color::rgb(v, p, q),
        }
    }

    pub fn to_hsv(self) -> (f32, f32, f32) {
        let max = self.r.max(self.g).max(self.b);
        let min = self.r.min(self.g).min(self.b);
        let d = max - min;
        let h = if d < 1e-5 {
            0.0
        } else if max == self.r {
            ((self.g - self.b) / d).rem_euclid(6.0) / 6.0
        } else if max == self.g {
            ((self.b - self.r) / d + 2.0) / 6.0
        } else {
            ((self.r - self.g) / d + 4.0) / 6.0
        };
        (h, if max < 1e-5 { 0.0 } else { d / max }, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_round_trip() {
        for c in [Color::BLUE, Color::ORANGE, Color::PINK, Color::GREEN, Color::gray(0.4)] {
            let (h, s, v) = c.to_hsv();
            assert_eq!(Color::from_hsv(h, s, v).to_hex(), c.to_hex());
        }
    }
}
