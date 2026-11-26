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

/// A bar that fills from left to right: a flat capsule in one color on a
/// quiet track, as on iOS. Progress bars are not glass.
pub struct ProgressBar {
    value: f32,
    height: f32,
    tint: Option<Color>,
    track: Option<Color>,
}

impl ProgressBar {
    /// Creates a bar filled to `value`, from 0 to 1.
    pub fn new(value: f32) -> Self {
        Self { value, height: 6.0, tint: None, track: None }
    }
    /// Sets the height in points. The default is 6.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }
    /// Sets the fill color. The default is the theme accent.
    pub fn tint(mut self, color: Color) -> Self {
        self.tint = Some(color);
        self
    }
    /// Sets the color of the unfilled track. The default is the strong fill
    /// of the surface below.
    pub fn track(mut self, color: Color) -> Self {
        self.track = Some(color);
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
        let v = ui.ctx.animate(r.id, self.value.clamp(0.0, 1.0), Tuning::new(0.5, 1.0));
        let fill = self.tint.unwrap_or(ui.ctx.theme.accent);
        let track = self.track.unwrap_or(ui.surface.strong_fill_color());
        let mut p = ui.painter();
        p.fill(r.rect, Corner::Capsule, track);
        if v > 0.0005 {
            // Below the height of the bar the fill shrinks into a dot rather
            // than squashing its round ends.
            let fw = r.rect.width() * v;
            let h = self.height.min(fw.max(self.height * 0.5));
            let bar =
                Rect::from_min_size(vec2(r.rect.min.x, r.rect.center().y - h * 0.5), vec2(fw.max(h), h));
            p.fill(bar, Corner::Capsule, fill);
        }
        r
    }
}

/// A ring that fills clockwise, with a short label in the middle.
///
/// The label shrinks until it fits inside the ring with
/// [`Gauge::label_padding`] to spare, so long words never touch it.
pub struct Gauge<'a> {
    value: f32,
    size: f32,
    label: &'a str,
    tint: Color,
    thickness: Option<f32>,
    label_size: Option<f32>,
    padding: Option<f32>,
}

impl<'a> Gauge<'a> {
    /// Creates a ring filled to `value`, from 0 to 1.
    pub fn new(value: f32, label: &'a str) -> Self {
        Self {
            value,
            size: 64.0,
            label,
            tint: Color::GREEN,
            thickness: None,
            label_size: None,
            padding: None,
        }
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
    /// Sets the width of the ring in points. The default is 11 percent of
    /// the diameter.
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = Some(thickness);
        self
    }
    /// Sets the largest font size of the label in points. The default is 22
    /// percent of the diameter. Labels that do not fit are drawn smaller.
    pub fn label_size(mut self, size: f32) -> Self {
        self.label_size = Some(size);
        self
    }
    /// Sets the space kept between the label and the inside of the ring, in
    /// points. The default is 9 percent of the diameter.
    pub fn label_padding(mut self, padding: f32) -> Self {
        self.padding = Some(padding);
        self
    }
}

/// Smallest label size a gauge shrinks to, in points.
const MIN_GAUGE_LABEL: f32 = 7.0;

impl Widget for Gauge<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let r = ui.allocate_response(Vec2::splat(self.size), Sense::Hover);
        if let Some(n) = ui.access(r.id, Role::Meter, r.rect) {
            n.label(self.label).numeric(self.value.clamp(0.0, 1.0) as f64, 0.0, 1.0, None);
        }
        let v = ui.ctx.animate(r.id, self.value.clamp(0.0, 1.0), Tuning::new(0.6, 0.85));
        let radius = self.size * 0.5;
        let w = self.thickness.unwrap_or(self.size * 0.11).clamp(1.0, radius * 0.6);
        let pad = self.padding.unwrap_or(self.size * 0.09).max(0.0);
        let room = (radius - w - pad).max(1.0);
        let l = fit_label(ui, self.label, self.label_size.unwrap_or(self.size * 0.22), room);
        let (track, tc) = (ui.surface.fill_color(), ui.surface.text_color());
        let center = r.rect.center();
        let mut p = ui.painter();
        p.arc(center, radius, w, 0.0, TAU, track, track);
        if v > 0.001 {
            // The round caps stick out by half the width at both ends. Keep
            // a small gap between them when the ring is nearly full.
            let cap = w / (radius - w * 0.5);
            let sweep = (v * TAU).min(TAU - cap * 0.35);
            p.arc(center, radius, w, -TAU / 4.0, sweep, self.tint.lerp(Color::WHITE, 0.2), self.tint);
        }
        p.text_centered(r.rect, &l, tc, 1.0);
        r
    }
}

/// Lays out `text` at `size`, or smaller until its box fits in a circle of
/// `room` points radius.
fn fit_label(ui: &mut Ui, text: &str, size: f32, room: f32) -> std::rc::Rc<crate::text::TextLayout> {
    let style = |s: f32| TextStyle::new(s).weight(Weight::Semibold);
    let l = ui.layout_text(text, &style(size));
    // Glyphs fill about three quarters of the line height.
    let half = vec2(l.size.x, l.size.y * 0.74) * 0.5;
    let k = room / half.length().max(0.01);
    if k >= 1.0 {
        return l;
    }
    // Text scales with its size. Round down to half points so the layout
    // is not redone for every tiny change.
    let fitted = ((size * k * 2.0).floor() * 0.5).max(MIN_GAUGE_LABEL);
    ui.layout_text(text, &style(fitted))
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
        let c = ui.surface.text_color();
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
    /// Adds a progress bar filled to `value`, from 0 to 1.
    pub fn progress(&mut self, value: f32) -> Response {
        self.add(ProgressBar::new(value))
    }
    /// Adds a spinner.
    pub fn spinner(&mut self) -> Response {
        self.add(Spinner::new())
    }
}
