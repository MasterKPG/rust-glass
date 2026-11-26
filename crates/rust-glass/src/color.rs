//! Colors.
//!
//! Components are not premultiplied and are stored in the sRGB encoding.
//! Blending happens in that same space, like desktop compositors and Core
//! Animation do.

/// An RGBA color with components from 0 to 1, in the sRGB encoding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    /// Red, 0 to 1.
    pub r: f32,
    /// Green, 0 to 1.
    pub g: f32,
    /// Blue, 0 to 1.
    pub b: f32,
    /// Opacity, 0 to 1.
    pub a: f32,
}

impl Color {
    /// Fully transparent black.
    pub const TRANSPARENT: Color = Color::rgba(0.0, 0.0, 0.0, 0.0);
    /// Opaque white.
    pub const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);
    /// Opaque black.
    pub const BLACK: Color = Color::rgb(0.0, 0.0, 0.0);
    /// The system blue. The default accent color.
    pub const BLUE: Color = Color::hex(0x0A84FF);
    /// The system green.
    pub const GREEN: Color = Color::hex(0x34C759);
    /// The system red.
    pub const RED: Color = Color::hex(0xFF3B30);
    /// The system orange.
    pub const ORANGE: Color = Color::hex(0xFF9500);
    /// The system yellow.
    pub const YELLOW: Color = Color::hex(0xFFCC00);
    /// The system pink.
    pub const PINK: Color = Color::hex(0xFF2D6F);
    /// The system purple.
    pub const PURPLE: Color = Color::hex(0xAF52DE);
    /// The system teal.
    pub const TEAL: Color = Color::hex(0x30B0C7);
    /// The system indigo.
    pub const INDIGO: Color = Color::hex(0x5856D6);

    /// Creates a color from red, green, blue and opacity.
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
    /// Creates an opaque color from red, green and blue.
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }
    /// Creates an opaque color from a `0xRRGGBB` value.
    ///
    /// ```
    /// # use rust_glass::Color;
    /// assert_eq!(Color::hex(0xFF0000), Color::rgb(1.0, 0.0, 0.0));
    /// ```
    pub const fn hex(rgb: u32) -> Self {
        Self::rgb(
            ((rgb >> 16) & 255) as f32 / 255.0,
            ((rgb >> 8) & 255) as f32 / 255.0,
            (rgb & 255) as f32 / 255.0,
        )
    }
    /// Creates an opaque gray, from 0 (black) to 1 (white).
    pub const fn gray(level: f32) -> Self {
        Self::rgb(level, level, level)
    }
    /// Creates black with the given opacity.
    pub const fn black(alpha: f32) -> Self {
        Self::rgba(0.0, 0.0, 0.0, alpha)
    }
    /// Creates white with the given opacity.
    pub const fn white(alpha: f32) -> Self {
        Self::rgba(1.0, 1.0, 1.0, alpha)
    }
    /// Returns the color with its opacity replaced.
    pub const fn alpha(self, alpha: f32) -> Self {
        Self { a: alpha, ..self }
    }
    /// Returns the color with its opacity multiplied by `factor`.
    pub fn fade(self, factor: f32) -> Self {
        Self { a: self.a * factor, ..self }
    }
    /// Mixes towards `other`: `t` = 0 gives `self`, 1 gives `other`.
    pub fn lerp(self, other: Color, t: f32) -> Color {
        Color {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }
    /// Returns the perceived brightness, from 0 to 1.
    pub fn luma(self) -> f32 {
        0.299 * self.r + 0.587 * self.g + 0.114 * self.b
    }
    /// Returns `[r, g, b, a]`.
    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
    /// Returns the color as `0xRRGGBB`, without opacity.
    pub fn to_hex(self) -> u32 {
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
        (byte(self.r) << 16) | (byte(self.g) << 8) | byte(self.b)
    }

    /// Creates an opaque color from hue, saturation and value, each 0 to 1.
    pub fn from_hsv(hue: f32, saturation: f32, value: f32) -> Color {
        let (s, v) = (saturation, value);
        let h = hue.rem_euclid(1.0) * 6.0;
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

    /// Returns hue, saturation and value, each 0 to 1.
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
