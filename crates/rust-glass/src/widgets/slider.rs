//! Slider, stepper and drag value.

use std::ops::RangeInclusive;

use crate::accessibility::{Action, Role};
use crate::animation::Tuning;
use crate::color::Color;
use crate::context::{Cursor, Response, Sense};
use crate::geometry::{Rect, smoothstep, vec2};
use crate::input::Key;
use crate::material::Corner;
use crate::text::{TextStyle, Weight};
use crate::ui::{Ui, Widget};
use crate::widgets::press::rubber_band;
use crate::widgets::thumb::{load_thumb, store_thumb, thumb_glass, thumb_rect};

/// Slider with a droplet thumb. While held the thumb is a clear lens, so the
/// end of the filled track stays visible under the finger.
pub struct Slider<'a> {
    value: &'a mut f32,
    range: RangeInclusive<f32>,
    step: Option<f32>,
    width: Option<f32>,
    scale: f32,
    tint: Option<Color>,
    knob: Option<Color>,
    track: Option<Color>,
}

impl<'a> Slider<'a> {
    /// Creates a slider for `value` within `range`.
    pub fn new(value: &'a mut f32, range: RangeInclusive<f32>) -> Self {
        Self { value, range, step: None, width: None, scale: 1.0, tint: None, knob: None, track: None }
    }
    /// Snap to multiples of `s`.
    pub fn step(mut self, s: f32) -> Self {
        self.step = Some(s);
        self
    }
    /// Sets the width. The default is the width left, at most 420.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
    /// Enlarge track and thumb.
    pub fn scale(mut self, s: f32) -> Self {
        self.scale = s;
        self
    }
    /// Sets the color of the filled part. The default is the accent color.
    pub fn tint(mut self, color: Color) -> Self {
        self.tint = Some(color);
        self
    }
    /// Sets the color of the thumb at rest.
    pub fn thumb_color(mut self, color: Color) -> Self {
        self.knob = Some(color);
        self
    }
    /// Sets the color of the empty part of the track.
    pub fn track_color(mut self, color: Color) -> Self {
        self.track = Some(color);
        self
    }
}

impl Widget for Slider<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let k = self.scale;
        let rest = vec2(37.0, 24.0) * k;
        let lifted = vec2(50.4, 34.6) * k;
        let track_h = 7.2 * k;
        let w = self.width.unwrap_or_else(|| ui.available_width().clamp(80.0, 420.0));
        let h = (rest.y + 8.0 * k).max(ui.ctx.theme.control_height.min(32.0));
        let mut r = ui.allocate_response(vec2(w, h), Sense::Drag);
        ui.ctx.focusable(r.id);
        let (lo, hi) = (*self.range.start(), *self.range.end());
        let span = (hi - lo).max(1e-6);
        let mut d = load_thumb(ui.ctx, r.id);
        // The thumb travels inside the track. All x values are relative to it.
        let ox = r.rect.min.x;
        let x0 = rest.x * 0.5;
        let x1 = r.rect.width() - rest.x * 0.5;
        let to_x = |v: f32| x0 + (x1 - x0) * ((v - lo) / span).clamp(0.0, 1.0);

        // Grabbing the thumb keeps the offset to the pointer.
        #[derive(Clone, Copy, Default)]
        struct Grab(f32);
        let mut grab: Grab = ui.ctx.get(r.id);
        let mut over = 0.0;
        let before = *self.value;
        if let Some(p) = r.pointer
            && r.held
        {
            if r.pressed {
                let at = d.pos.value + ox;
                grab = Grab(if (p.x - at).abs() < lifted.x * 0.6 { p.x - at } else { 0.0 });
            }
            let x = p.x - ox - grab.0;
            over = rubber_band(x - x.clamp(x0, x1), 10.0 * k);
            let mut v = lo + span * ((x - x0) / (x1 - x0).max(1.0)).clamp(0.0, 1.0);
            if let Some(s) = self.step {
                v = (lo + ((v - lo) / s).round() * s).clamp(lo, hi);
            }
            *self.value = v;
        }
        ui.ctx.set(r.id, grab);
        if r.focused {
            let s = self.step.unwrap_or(span / 40.0);
            if ui.ctx.input.take_key(Key::Right) || ui.ctx.input.take_key(Key::Up) {
                *self.value = (*self.value + s).min(hi);
            }
            if ui.ctx.input.take_key(Key::Left) || ui.ctx.input.take_key(Key::Down) {
                *self.value = (*self.value - s).max(lo);
            }
        }
        if let Some(v) = ui.ctx.take_access_number(r.id) {
            *self.value = (v as f32).clamp(lo, hi);
        }
        if let Some(n) = ui.access(r.id, Role::Slider, r.rect) {
            n.numeric(*self.value as f64, lo as f64, hi as f64, self.step.map(|s| s as f64));
        }
        r.changed = before != *self.value;
        if r.hovered || r.held {
            ui.ctx.cursor = Cursor::Grab;
        }

        let target = to_x(*self.value) + over;
        // Tight under the finger, loose while catching up after a press elsewhere.
        let gap = (d.pos.value - target).abs() / lifted.x;
        let follow = if r.held && self.step.is_none() {
            Tuning::new(0.085 + 0.16 * smoothstep(0.4, 1.6, gap), 0.92)
        } else {
            Tuning::new(0.30, 0.82)
        };
        let f = d.step(ui.ctx, target, follow, r.held, rest, lifted);
        store_thumb(ui.ctx, r.id, d);
        let px = ox + d.pos.value;

        let cy = r.rect.center().y;
        let accent = self.tint.unwrap_or(ui.ctx.theme.accent);
        let track_c = self.track.unwrap_or(ui.surface.strong_fill_color());
        let knob = self.knob.unwrap_or(ui.surface.thumb_color());
        let track = Rect::from_center_size(r.rect.center(), vec2(w, track_h));
        let thumb = thumb_rect(ui.ctx, vec2(px, cy), &f);
        let glass = thumb_glass(ui.ctx, thumb, &f, knob, k);
        let step = self.step;
        let mut p = ui.painter();
        p.fill(track, Corner::Circular(track_h), track_c);
        let fill = Rect { min: track.min, max: vec2(px.max(track.min.x + track_h), track.max.y) };
        p.fill(fill, Corner::Circular(track_h), accent);
        if let Some(s) = step {
            let n = (span / s).round() as i32;
            if (1..=12).contains(&n) {
                for i in 0..=n {
                    let x = ox + to_x(lo + s * i as f32);
                    let c = Color::black(0.16);
                    p.fill(
                        Rect::from_center_size(vec2(x, cy), vec2(2.0, 2.0) * k),
                        Corner::Circular(1.0 * k),
                        c,
                    );
                }
            }
        }
        p.glass(&glass);
        ui.focus_ring(r.id, thumb, Corner::Capsule);
        r
    }
}

/// Drag left and right, or use the arrows, to change a number.
pub struct DragValue<'a> {
    value: &'a mut f32,
    speed: f32,
    range: RangeInclusive<f32>,
    decimals: usize,
    suffix: &'a str,
}

impl<'a> DragValue<'a> {
    /// Creates a field for `value`.
    pub fn new(value: &'a mut f32) -> Self {
        Self { value, speed: 0.1, range: f32::MIN..=f32::MAX, decimals: 1, suffix: "" }
    }
    /// Sets how much the value changes per point of pointer movement.
    pub fn speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }
    /// Keeps the value within `range`.
    pub fn range(mut self, range: RangeInclusive<f32>) -> Self {
        self.range = range;
        self
    }
    /// Sets the number of decimals shown.
    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = decimals;
        self
    }
    /// Shows `suffix` after the number, such as a unit.
    pub fn suffix(mut self, suffix: &'a str) -> Self {
        self.suffix = suffix;
        self
    }
}

impl Widget for DragValue<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let text = format!("{:.*}{}", self.decimals, *self.value, self.suffix);
        let st = TextStyle::new(ui.ctx.theme.body_size).weight(Weight::Medium);
        let l = ui.layout_text(&text, &st);
        let mut r = ui.allocate_response(vec2((l.size.x + 26.0).max(64.0), 30.0), Sense::Drag);
        ui.ctx.focusable(r.id);
        if r.hovered || r.held {
            ui.ctx.cursor = Cursor::ResizeHorizontal;
        }
        let (lo, hi) = (*self.range.start(), *self.range.end());
        let before = *self.value;
        if r.held {
            *self.value = (*self.value + r.delta.x * self.speed).clamp(lo, hi);
        }
        if r.focused {
            let d = ui.ctx.input.key(Key::Right) as i32 - ui.ctx.input.key(Key::Left) as i32;
            *self.value = (*self.value + d as f32 * self.speed).clamp(lo, hi);
        }
        if let Some(v) = ui.ctx.take_access_number(r.id) {
            *self.value = (v as f32).clamp(lo, hi);
        }
        if let Some(n) = ui.access(r.id, Role::SpinButton, r.rect) {
            n.numeric(*self.value as f64, lo as f64, hi as f64, Some(self.speed as f64)).value(text.clone());
        }
        r.changed = before != *self.value;
        let gel = crate::widgets::press::PressEffect::update(ui.ctx, &r, 0.6);
        let (c, t) = (ui.surface.fill_color(), ui.surface.text_color());
        let mut p = ui.painter();
        p.fill(gel.rect, 9.0, c.fade(1.0 + gel.hover * 0.6));
        p.text_centered(gel.rect, &l, t, gel.scale);
        ui.focus_ring(r.id, gel.rect, 9.0);
        r
    }
}

/// Minus and plus buttons on one capsule.
pub struct Stepper<'a> {
    value: &'a mut i32,
    range: RangeInclusive<i32>,
}

impl<'a> Stepper<'a> {
    /// Creates a stepper that changes `value` by one within `range`.
    pub fn new(value: &'a mut i32, range: RangeInclusive<i32>) -> Self {
        Self { value, range }
    }
}

impl Widget for Stepper<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let size = vec2(92.0, 32.0);
        let rect = ui.allocate(size);
        let id = ui.next_id();
        let mut res = ui.interact(id, rect, Sense::Hover);
        let mid = rect.center().x;
        let l = Rect { min: rect.min, max: vec2(mid, rect.max.y) };
        let rr = Rect { min: vec2(mid, rect.min.y), max: rect.max };
        let a = ui.interact(id.with(0), l, Sense::Click);
        let b = ui.interact(id.with(1), rr, Sense::Click);
        let before = *self.value;
        if a.clicked || ui.ctx.access_requested(id, &Action::Decrement) {
            *self.value -= 1;
        }
        if b.clicked || ui.ctx.access_requested(id, &Action::Increment) {
            *self.value += 1;
        }
        if let Some(v) = ui.ctx.take_access_number(id) {
            *self.value = v.round() as i32;
        }
        *self.value = (*self.value).clamp(*self.range.start(), *self.range.end());
        let (lo, hi) = (*self.range.start() as f64, *self.range.end() as f64);
        if let Some(n) = ui.access(id, Role::SpinButton, rect) {
            n.numeric(*self.value as f64, lo, hi, Some(1.0));
        }
        if let Some(n) = ui.access(a.id, Role::Button, l) {
            n.label("Decrease");
        }
        if let Some(n) = ui.access(b.id, Role::Button, rr) {
            n.label("Increase");
        }
        res.changed = before != *self.value;
        let ga = crate::widgets::press::PressEffect::update(ui.ctx, &a, 1.0);
        let gb = crate::widgets::press::PressEffect::update(ui.ctx, &b, 1.0);
        let mut bar = rect;
        for g in [&ga, &gb] {
            if g.amount.abs() > 0.001 {
                bar = bar.union(g.rect);
            }
        }
        let can = (*self.value > *self.range.start(), *self.value < *self.range.end());
        let mut color = ui.surface.text_color();
        if ui.surface.is_glass() {
            let c = ui.surface.fill_color();
            ui.painter().fill(bar, Corner::Capsule, c);
        } else {
            let m = ui.ctx.theme.regular;
            let mut g = crate::material::Glass::new(bar, Corner::Capsule, m);
            color = ui.ctx.prepare_glass(id, &mut g, true).surface().text_color();
            g.brightness = 0.04 * ga.amount.max(gb.amount).max(0.0);
            ui.painter().glass(&g);
        }
        let sep = color.alpha(0.16);
        let mut p = ui.painter();
        p.icon(
            ga.rect.center(),
            16.0 * ga.scale,
            crate::icon::Icon::Minus,
            color.fade(if can.0 { 1.0 } else { 0.3 }),
        );
        p.icon(
            gb.rect.center(),
            16.0 * gb.scale,
            crate::icon::Icon::Plus,
            color.fade(if can.1 { 1.0 } else { 0.3 }),
        );
        p.line(vec2(mid, rect.min.y + 8.0), vec2(mid, rect.max.y - 8.0), 1.0, sep);
        res
    }
}

impl Ui<'_> {
    /// Adds a slider for `value` within `range`.
    pub fn slider(&mut self, value: &mut f32, range: RangeInclusive<f32>) -> Response {
        self.add(Slider::new(value, range))
    }
}
