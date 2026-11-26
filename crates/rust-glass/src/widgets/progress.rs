//! Progress bar, gauge and spinner.

use std::f32::consts::TAU;

use crate::accessibility::Role;
use crate::animation::Tuning;
use crate::color::Color;
use crate::context::{Response, Sense};
use crate::geometry::{Rect, Vec2, vec2};
use crate::material::Corner;
use crate::text::{TextStyle, Weight};
use crate::ui::{Ui, Widget};

/// A bar that fills from left to right.
pub struct ProgressBar {
    value: f32,
    height: f32,
    tint: Option<Color>,
}

impl ProgressBar {
    /// Creates a bar filled to `value`, from 0 to 1.
    pub fn new(value: f32) -> Self {
        Self { value, height: 8.0, tint: None }
    }
    /// Sets the height in points. The default is 8.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }
    /// Sets the fill color. The default is the theme accent.
    pub fn tint(mut self, color: Color) -> Self {
        self.tint = Some(color);
        self
    }
}

impl Widget for ProgressBar {
    fn ui(self, ui: &mut Ui) -> Response {
        let w = ui.available_width().clamp(60.0, 460.0);
        let r = ui.allocate_response(vec2(w, self.height), Sense::Hover);
        if let Some(n) = ui.access(r.id, Role::ProgressIndicator, r.rect) {
            n.numeric(self.value.clamp(0.0, 1.0) as f64, 0.0, 1.0, None);
        }
        let v = ui.ctx.animate(r.id, self.value.clamp(0.0, 1.0), Tuning::new(0.5, 0.9));
        let c = self.tint.unwrap_or(ui.ctx.theme.accent);
        let track = ui.surface.fill_strong();
        let fw = (r.rect.width() * v).max(self.height);
        let fill = Rect::from_min_size(r.rect.min, vec2(fw, self.height));
        let mut p = ui.painter();
        p.fill(r.rect, Corner::Capsule, track);
        p.gradient(fill, Corner::Capsule, c.lerp(Color::WHITE, 0.10), c, vec2(0.0, 1.0));
        let hl =
            Rect::from_min_size(fill.min + vec2(3.0, 1.0), vec2((fw - 6.0).max(0.0), self.height * 0.34));
        p.gradient(hl, Corner::Capsule, Color::white(0.45), Color::white(0.0), vec2(0.0, 1.0));
        r
    }
}

/// A ring that fills clockwise, with a short label in the middle.
pub struct Gauge<'a> {
    value: f32,
    size: f32,
    label: &'a str,
    tint: Color,
}

impl<'a> Gauge<'a> {
    /// Creates a ring filled to `value`, from 0 to 1.
    pub fn new(value: f32, label: &'a str) -> Self {
        Self { value, size: 64.0, label, tint: Color::GREEN }
    }
    /// Sets the diameter in points. The default is 64.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
    /// Sets the fill color. The default is green.
    pub fn tint(mut self, color: Color) -> Self {
        self.tint = color;
        self
    }
}

impl Widget for Gauge<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let r = ui.allocate_response(Vec2::splat(self.size), Sense::Hover);
        if let Some(n) = ui.access(r.id, Role::Meter, r.rect) {
            n.label(self.label).numeric(self.value.clamp(0.0, 1.0) as f64, 0.0, 1.0, None);
        }
        let v = ui.ctx.animate(r.id, self.value.clamp(0.0, 1.0), Tuning::new(0.6, 0.85));
        let w = self.size * 0.12;
        let st = TextStyle::new(self.size * 0.22).weight(Weight::Semibold);
        let l = ui.layout_text(self.label, &st);
        let (track, tc) = (ui.surface.fill(), ui.surface.text());
        let mut p = ui.painter();
        p.arc(r.rect.center(), self.size * 0.5, w, 0.0, TAU, track, track);
        if v > 0.001 {
            p.arc(
                r.rect.center(),
                self.size * 0.5,
                w,
                -TAU / 4.0,
                v * TAU,
                self.tint.lerp(Color::WHITE, 0.25),
                self.tint,
            );
        }
        p.text_centered(r.rect, &l, tc, 1.0);
        r
    }
}

/// An activity indicator for work of unknown length.
pub struct Spinner {
    size: f32,
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

impl Spinner {
    /// Creates a spinner 24 points wide.
    pub fn new() -> Self {
        Self { size: 24.0 }
    }
    /// Sets the diameter in points.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
}

impl Widget for Spinner {
    fn ui(self, ui: &mut Ui) -> Response {
        let r = ui.allocate_response(Vec2::splat(self.size), Sense::Hover);
        let t = ui.ctx.time as f32;
        let c = ui.surface.text();
        let n = 8;
        let head = (t * 1.1).fract() * n as f32;
        let mut p = ui.painter();
        for i in 0..n {
            let a = i as f32 / n as f32 * TAU - TAU / 4.0;
            let dir = vec2(a.cos(), a.sin());
            let age = (head - i as f32).rem_euclid(n as f32) / n as f32;
            let k = self.size * 0.5;
            p.line(
                r.rect.center() + dir * k * 0.46,
                r.rect.center() + dir * k * 0.86,
                self.size * 0.10,
                c.alpha(0.12 + 0.62 * (1.0 - age).powf(1.8)),
            );
        }
        ui.ctx.request_repaint();
        r
    }
}

impl Ui<'_> {
    pub fn progress(&mut self, value: f32) -> Response {
        self.add(ProgressBar::new(value))
    }
    pub fn spinner(&mut self) -> Response {
        self.add(Spinner::new())
    }
}
