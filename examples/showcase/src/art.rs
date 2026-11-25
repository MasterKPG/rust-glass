//! Generated wallpapers and cover art, so the showcase ships without images.

use rust_glass::prelude::*;

pub struct Wallpaper {
    pub name: &'static str,
    pub dark: bool,
    kind: Kind,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Dunes,
    Aurora,
    Petals,
    Ribbons,
    Grid,
    Plain,
}

pub const WALLPAPERS: &[Wallpaper] = &[
    Wallpaper { name: "Dunes", dark: false, kind: Kind::Dunes },
    Wallpaper { name: "Petals", dark: false, kind: Kind::Petals },
    Wallpaper { name: "Aurora", dark: true, kind: Kind::Aurora },
    Wallpaper { name: "Ribbons", dark: true, kind: Kind::Ribbons },
    Wallpaper { name: "Grid", dark: false, kind: Kind::Grid },
    Wallpaper { name: "Plain", dark: false, kind: Kind::Plain },
];

type V3 = [f32; 3];

fn mix(a: V3, b: V3, t: f32) -> V3 {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}
fn hex(v: u32) -> V3 {
    [((v >> 16) & 255) as f32 / 255.0, ((v >> 8) & 255) as f32 / 255.0, (v & 255) as f32 / 255.0]
}
fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn ramp(stops: &[V3], t: f32) -> V3 {
    let t = t.clamp(0.0, 0.9999) * (stops.len() - 1) as f32;
    let i = t as usize;
    mix(stops[i], stops[i + 1], smooth(0.0, 1.0, t.fract()))
}

fn shade(kind: Kind, x: f32, y: f32) -> V3 {
    match kind {
        Kind::Dunes => {
            let sky = ramp(&[hex(0x7FB8F0), hex(0xBFD9F2), hex(0xF8D9B4), hex(0xF5A66B)], y * 1.5);
            let mut c = sky;
            let sx = x - 1.12;
            let sy = y - 0.30;
            let r = (sx * sx + sy * sy).sqrt();
            c = mix(c, hex(0xFFF3DA), 1.0 - smooth(0.085, 0.092, r));
            c = mix(c, hex(0xFFE0B0), (1.0 - smooth(0.08, 0.45, r)) * 0.5);
            let pal = [
                (hex(0xE98A4E), hex(0xC75B39)),
                (hex(0xD9663D), hex(0x9E3A37)),
                (hex(0xB5433B), hex(0x6E2A40)),
                (hex(0x7C2E47), hex(0x3A1B3F)),
            ];
            for (i, (a, b)) in pal.iter().enumerate() {
                let f = i as f32;
                let h = 0.46
                    + f * 0.135
                    + 0.060 * (x * (2.1 + f * 0.6) + f * 1.9).sin()
                    + 0.022 * (x * (5.3 - f * 0.5) + f * 0.7).cos();
                if y > h {
                    let d = ((y - h) * 3.2).clamp(0.0, 1.0);
                    let ridge = smooth(0.012, 0.0, y - h);
                    let rip = 0.5 + 0.5 * ((y - h) * 120.0 + (x * 9.0 + f).sin() * 3.0).sin();
                    let mut k = mix(*a, *b, d);
                    k = mix(k, *a, rip * 0.10 * (1.0 - d));
                    c = mix(k, hex(0xFFD2A0), ridge * 0.5);
                }
            }
            c
        }
        Kind::Aurora => {
            let mut c = ramp(&[hex(0x040714), hex(0x0A1430), hex(0x13204A), hex(0x0A0F26)], y);
            for i in 0..4 {
                let f = i as f32;
                let wave = 0.34
                    + f * 0.07
                    + 0.10 * (x * (1.8 + f * 0.7) + f * 2.1).sin()
                    + 0.04 * (x * 6.1 + f).sin();
                let d = y - wave;
                let glow = if d > 0.0 { (-d * 5.0).exp() } else { (d * 26.0).exp() };
                let streak = 0.72 + 0.28 * ((x * 38.0 + f * 7.0).sin() * (x * 17.0).cos());
                let col = [hex(0x35F0A6), hex(0x2BC4F5), hex(0x9A5BFF), hex(0xFF5AA8)][i];
                c = mix(c, col, glow * streak * 0.55);
            }
            let (gx, gy) = (x * 46.0, y * 46.0);
            let (sx, sy) = (gx.floor(), gy.floor());
            let h = |k: f32| ((sx * 127.1 + sy * 311.7 + k).sin() * 43758.547).fract().abs();
            if h(0.0) > 0.72 && y < 0.62 {
                let (ox, oy) = (0.2 + 0.6 * h(1.0), 0.2 + 0.6 * h(2.0));
                let d = ((gx - sx - ox).powi(2) + (gy - sy - oy).powi(2)).sqrt();
                let r = 0.03 + 0.05 * h(3.0);
                c = mix(c, [1.0; 3], smooth(r, r * 0.3, d) * (0.35 + 0.6 * h(4.0)));
            }
            let m = 0.80 + 0.06 * (x * 4.0).sin() + 0.03 * (x * 11.0 + 1.0).sin();
            if y > m {
                c = mix(hex(0x070A18), hex(0x02030A), (y - m) * 5.0);
            }
            c
        }
        Kind::Petals => {
            let mut c =
                ramp(&[hex(0xFDF2F6), hex(0xF9D5E5), hex(0xDCCBF5), hex(0xBFE0F7)], (x * 0.5 + y) * 0.62);
            let flowers = [
                (0.28, 0.24, 0.30, 0xFF5C8A, 0xFFC2D6),
                (1.22, 0.20, 0.27, 0x7A5CFF, 0xD3C4FF),
                (0.80, 0.62, 0.34, 0xFF8A3D, 0xFFE0A8),
                (0.16, 0.86, 0.26, 0x18A0D8, 0xB5EBFF),
                (1.42, 0.80, 0.33, 0xF0386B, 0xFFB0B8),
                (0.62, 0.10, 0.15, 0xFFC531, 0xFFF0B8),
                (1.02, 0.94, 0.18, 0x2BB673, 0xC4F5D6),
            ];
            for (i, &(fx, fy, fr, a, b)) in flowers.iter().enumerate() {
                let (dx, dy) = (x - fx, y - fy);
                let r = (dx * dx + dy * dy).sqrt() / fr;
                let ang = dy.atan2(dx) + i as f32;
                let petal = 0.66 + 0.34 * (ang * 2.5).cos().abs().powf(0.55);
                let k = r / petal;
                if k < 1.0 {
                    let vein = 0.5 + 0.5 * (ang * 30.0).sin();
                    let col = mix(hex(a), hex(b), smooth(0.15, 1.0, k) * 0.9 + vein * 0.06);
                    c = mix(c, col, smooth(1.0, 0.97, k));
                }
                if r < 0.17 {
                    c = mix(hex(0xFFD84A), hex(0xD08A12), smooth(0.06, 0.17, r));
                }
            }
            c
        }
        Kind::Ribbons => {
            let mut c = mix(hex(0x8A8278), hex(0x5E5750), y);
            for i in 0..7 {
                let f = i as f32;
                let path = -0.1
                    + f * 0.19
                    + 0.17 * (x * (2.2 + 0.25 * f) + f * 1.7).sin()
                    + 0.05 * (x * 5.0 + f * 2.0).cos();
                let w = 0.052 + 0.016 * (x * 3.0 + f).sin();
                let d = (y - path) / w;
                if d.abs() < 1.0 {
                    let s = (1.0 - d * d).sqrt();
                    let hl = (1.0 - ((d + 0.45) / 0.16).powi(2)).max(0.0) * 0.75
                        + (1.0 - ((d - 0.62) / 0.10).powi(2)).max(0.0) * 0.35;
                    let k = mix(hex(0x060606), hex(0x2A2A2C), s * 0.6);
                    c = mix(c, mix(k, [0.92; 3], hl), smooth(1.0, 0.96, d.abs()));
                } else if d > 1.0 && d < 2.2 {
                    c = mix(c, [0.0; 3], (2.2 - d) / 1.2 * 0.28);
                }
            }
            c
        }
        Kind::Grid => {
            let n = 16.0;
            let (gx, gy) = (x * n, y * n);
            let on = (gx.floor() + gy.floor()) as i32 % 2 == 0;
            let base = if on { hex(0xF7F7F9) } else { hex(0xE3E5EA) };
            let hue = Color::from_hsv((gx.floor() * 0.071 + gy.floor() * 0.113).fract(), 0.75, 0.95);
            let (fx, fy) = (gx.fract() - 0.5, gy.fract() - 0.5);
            let r = (fx * fx + fy * fy).sqrt();
            let mut c = base;
            if on {
                c = mix(c, [hue.r, hue.g, hue.b], smooth(0.30, 0.28, r));
            }
            let line = fx.abs().max(fy.abs());
            mix(c, hex(0x2A2F3A), smooth(0.485, 0.497, line) * 0.7)
        }
        Kind::Plain => hex(0xD9D9D9),
    }
}

impl Wallpaper {
    /// RGBA pixels, 3:2. Uses all cores.
    pub fn render(&self, w: u32, h: u32) -> Vec<u8> {
        let mut out = vec![255u8; (w * h * 4) as usize];
        let n = std::thread::available_parallelism().map_or(4, |n| n.get());
        let rows = (h as usize).div_ceil(n);
        let kind = self.kind;
        std::thread::scope(|s| {
            for (i, chunk) in out.chunks_mut(rows * w as usize * 4).enumerate() {
                s.spawn(move || {
                    for (j, px) in chunk.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                        let (x, y) = (j as u32 % w, (i * rows) as u32 + j as u32 / w);
                        let c = shade(kind, x as f32 / h as f32, y as f32 / h as f32);
                        for k in 0..3 {
                            px[k] = (c[k].clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                        }
                    }
                });
            }
        });
        out
    }
}

/// Square cover art number `i`.
pub fn cover(i: usize, n: u32) -> Vec<u8> {
    let f = i as f32;
    let a = Color::from_hsv(f * 0.137 + 0.92, 0.72, 0.95);
    let b = Color::from_hsv(f * 0.137 + 0.08, 0.80, 0.55);
    let d = Color::from_hsv(f * 0.137 + 0.45, 0.65, 0.98);
    let v = |c: Color| [c.r, c.g, c.b];
    let mut out = Vec::with_capacity((n * n * 4) as usize);
    for py in 0..n {
        for px in 0..n {
            let (x, y) = (px as f32 / n as f32, py as f32 / n as f32);
            let mut c = mix(v(a), v(b), (x * 0.4 + y * 0.8).clamp(0.0, 1.0));
            match i % 4 {
                0 => {
                    let r = ((x - 0.5).powi(2) + (y - 0.46).powi(2)).sqrt();
                    c = mix(c, v(d), smooth(0.285, 0.275, r));
                    let s = 0.5 + 0.5 * (y * 60.0).sin();
                    c = mix(c, v(b), smooth(0.285, 0.275, r) * smooth(0.45, 0.8, y) * smooth(0.4, 0.6, s));
                }
                1 => {
                    let w = (y + 0.07 * (x * 9.0 + f).sin()) * 7.0;
                    c = mix(c, v(d), smooth(0.45, 0.55, (w.fract() - 0.5).abs() * 2.0) * 0.8);
                }
                2 => {
                    let r = ((x - 0.2).powi(2) + (y - 0.95).powi(2)).sqrt();
                    let k = (r * 8.0).fract();
                    c = mix(c, v(d), smooth(0.5, 0.56, k) * smooth(1.0, 0.94, k) * 0.9);
                }
                _ => {
                    let t = (x - y).abs();
                    c = mix(c, v(d), smooth(0.22, 0.20, t));
                    c = mix(c, [1.0; 3], smooth(0.06, 0.05, (x + y - 1.0).abs()) * 0.9);
                }
            }
            for k in c {
                out.push((k.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            }
            out.push(255);
        }
    }
    out
}
