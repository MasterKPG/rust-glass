use std::ops::RangeInclusive;

use crate::anim::Tuning;
use crate::color::Color;
use crate::context::{Cursor, Response, Sense};
use crate::geom::{Rect, smoothstep, vec2};
use crate::input::Key;
use crate::material::Corner;
use crate::text::{TextStyle, Weight};
use crate::ui::{Ui, Widget};
use crate::widgets::gel::{droplet_glass, droplet_rect, droplet_state, rubber, set_droplet};

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
    pub fn new(value: &'a mut f32, range: RangeInclusive<f32>) -> Self {
        Self { value, range, step: None, width: None, scale: 1.0, tint: None, knob: None, track: None }
    }
    /// Snap to multiples of `s`.
    pub fn step(mut self, s: f32) -> Self {
        self.step = Some(s);
        self
    }
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    /// Enlarge track and thumb.
    pub fn scale(mut self, s: f32) -> Self {
        self.scale = s;
        self
    }
    pub fn tint(mut self, c: Color) -> Self {
        self.tint = Some(c);
        self
    }
    pub fn knob(mut self, c: Color) -> Self {
        self.knob = Some(c);
        self
    }
    pub fn track(mut self, c: Color) -> Self {
        self.track = Some(c);
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
        let mut r = ui.alloc_interact(vec2(w, h), Sense::Drag);
        ui.ctx.focusable(r.id);
        let (lo, hi) = (*self.range.start(), *self.range.end());
        let span = (hi - lo).max(1e-6);
        let mut d = droplet_state(ui.ctx, r.id);
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
                let at = d.pos.x + ox;
                grab = Grab(if (p.x - at).abs() < lifted.x * 0.6 { p.x - at } else { 0.0 });
            }
            let x = p.x - ox - grab.0;
            over = rubber(x - x.clamp(x0, x1), 10.0 * k);
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
        r.changed = before != *self.value;
        if r.hovered || r.held {
            ui.ctx.cursor = Cursor::Grab;
        }

        let target = to_x(*self.value) + over;
        // Tight under the finger, loose while catching up after a press elsewhere.
        let gap = (d.pos.x - target).abs() / lifted.x;
        let follow = if r.held && self.step.is_none() {
            Tuning::new(0.085 + 0.16 * smoothstep(0.4, 1.6, gap), 0.92)
        } else {
            Tuning::new(0.30, 0.82)
        };
        let f = d.step(ui.ctx, target, follow, r.held, rest, lifted);
        set_droplet(ui.ctx, r.id, d);
        let px = ox + d.pos.x;

        let cy = r.rect.center().y;
        let accent = self.tint.unwrap_or(ui.ctx.theme.accent);
        let track_c = self.track.unwrap_or(ui.surface.fill_strong());
        let knob = self.knob.unwrap_or(ui.surface.knob());
        let track = Rect::from_center_size(r.rect.center(), vec2(w, track_h));
        let thumb = droplet_rect(ui.ctx, vec2(px, cy), &f);
        let glass = droplet_glass(ui.ctx, thumb, &f, knob, k);
        let step = self.step;
        let mut p = ui.painter();
        p.fill(track, Corner::Round(track_h), track_c);
        let fill = Rect { min: track.min, max: vec2(px.max(track.min.x + track_h), track.max.y) };
        p.fill(fill, Corner::Round(track_h), accent);
        if let Some(s) = step {
            let n = (span / s).round() as i32;
            if (1..=12).contains(&n) {
                for i in 0..=n {
                    let x = ox + to_x(lo + s * i as f32);
                    let c = Color::black(0.16);
                    p.fill(
                        Rect::from_center_size(vec2(x, cy), vec2(2.0, 2.0) * k),
                        Corner::Round(1.0 * k),
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
    pub fn new(value: &'a mut f32) -> Self {
        Self { value, speed: 0.1, range: f32::MIN..=f32::MAX, decimals: 1, suffix: "" }
    }
    pub fn speed(mut self, s: f32) -> Self {
        self.speed = s;
        self
    }
    pub fn range(mut self, r: RangeInclusive<f32>) -> Self {
        self.range = r;
        self
    }
    pub fn decimals(mut self, d: usize) -> Self {
        self.decimals = d;
        self
    }
    pub fn suffix(mut self, s: &'a str) -> Self {
        self.suffix = s;
        self
    }
}

impl Widget for DragValue<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let text = format!("{:.*}{}", self.decimals, *self.value, self.suffix);
        let st = TextStyle::new(ui.ctx.theme.body).weight(Weight::Medium);
        let l = ui.layout_text(&text, &st);
        let mut r = ui.alloc_interact(vec2((l.size.x + 26.0).max(64.0), 30.0), Sense::Drag);
        ui.ctx.focusable(r.id);
        if r.hovered || r.held {
            ui.ctx.cursor = Cursor::ResizeH;
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
        r.changed = before != *self.value;
        let gel = crate::widgets::gel::Gel::update(ui.ctx, &r, 0.6);
        let (c, t) = (ui.surface.fill(), ui.surface.text());
        let mut p = ui.painter();
        p.fill(gel.rect, 9.0, c.fade(1.0 + gel.hover * 0.6));
        p.text_centered(gel.rect, &l, t, gel.scale);
        ui.focus_ring(r.id, gel.rect, 9.0);
        r
    }
}

/// Minus and plus on one capsule.
pub struct Stepper<'a> {
    pub value: &'a mut i32,
    pub range: RangeInclusive<i32>,
}

impl Widget for Stepper<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let size = vec2(92.0, 32.0);
        let rect = ui.alloc(size);
        let id = ui.next_id();
        let mut res = ui.interact(id, rect, Sense::Hover);
        let mid = rect.center().x;
        let l = Rect { min: rect.min, max: vec2(mid, rect.max.y) };
        let rr = Rect { min: vec2(mid, rect.min.y), max: rect.max };
        let a = ui.interact(id.with(0), l, Sense::Click);
        let b = ui.interact(id.with(1), rr, Sense::Click);
        let before = *self.value;
        if a.clicked {
            *self.value -= 1;
        }
        if b.clicked {
            *self.value += 1;
        }
        *self.value = (*self.value).clamp(*self.range.start(), *self.range.end());
        res.changed = before != *self.value;
        let ga = crate::widgets::gel::Gel::update(ui.ctx, &a, 1.0);
        let gb = crate::widgets::gel::Gel::update(ui.ctx, &b, 1.0);
        let mut bar = rect;
        for g in [&ga, &gb] {
            if g.press.abs() > 0.001 {
                bar = bar.union(g.rect);
            }
        }
        let can = (*self.value > *self.range.start(), *self.value < *self.range.end());
        let mut color = ui.surface.text();
        if ui.surface.is_glass() {
            let c = ui.surface.fill();
            ui.painter().fill(bar, Corner::Capsule, c);
        } else {
            let m = ui.ctx.theme.regular;
            let mut g = crate::material::Glass::new(bar, Corner::Capsule, m);
            color = ui.ctx.prepare_glass(id, &mut g, true).surface().text();
            g.lit = 0.04 * ga.press.max(gb.press).max(0.0);
            ui.painter().glass(&g);
        }
        let sep = color.alpha(0.16);
        let mut p = ui.painter();
        p.icon(
            ga.rect.center(),
            16.0 * ga.scale,
            crate::icons::Icon::Minus,
            color.fade(if can.0 { 1.0 } else { 0.3 }),
        );
        p.icon(
            gb.rect.center(),
            16.0 * gb.scale,
            crate::icons::Icon::Plus,
            color.fade(if can.1 { 1.0 } else { 0.3 }),
        );
        p.line(vec2(mid, rect.min.y + 8.0), vec2(mid, rect.max.y - 8.0), 1.0, sep);
        res
    }
}

impl Ui<'_> {
    pub fn slider(&mut self, value: &mut f32, range: RangeInclusive<f32>) -> Response {
        self.add(Slider::new(value, range))
    }
}
