//! Checkbox, radio, progress, spinner, images, color picker, collapsing
//! section, search field wrapper and list rows.

use std::f32::consts::TAU;

use crate::anim::Tuning;
use crate::color::Color;
use crate::context::{Cursor, Response, Sense};
use crate::geom::{Rect, Vec2, rect, vec2};
use crate::icons::Icon;
use crate::material::{Corner, Glass};
use crate::paint::ImageId;
use crate::text::{TextStyle, Weight};
use crate::ui::{Ui, Widget};
use crate::widgets::gel::Gel;

pub struct Checkbox<'a> {
    pub on: &'a mut bool,
    pub label: &'a str,
}

impl Widget for Checkbox<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let b = 22.0;
        let st = TextStyle::new(ui.ctx.theme.body);
        let l = ui.layout_text(self.label, &st);
        let mut r = ui.alloc_interact(vec2(b + 9.0 + l.size.x, 28.0), Sense::Click);
        ui.ctx.focusable(r.id);
        if r.clicked {
            *self.on = !*self.on;
            r.changed = true;
        }
        if r.hovered {
            ui.ctx.cursor = Cursor::Pointer;
        }
        let gel = Gel::update(ui.ctx, &r, 0.8);
        let t = ui.ctx.animate_bool(r.id.with("on"), *self.on, Tuning::new(0.32, 0.62));
        let c = Rect::from_center_size(
            vec2(r.rect.min.x + b * 0.5, r.rect.center().y),
            Vec2::splat(b * (1.0 + 0.12 * gel.press)),
        );
        check_face(ui, &r, &gel, c, Corner::Radius(7.0), t);
        // Tick drawn stroke by stroke.
        let k = (t * 1.1).clamp(0.0, 1.0);
        let p0 = c.center() + vec2(-5.0, 0.4);
        let p1 = c.center() + vec2(-1.5, 4.0);
        let p2 = c.center() + vec2(5.2, -4.0);
        let tc = ui.surface.text();
        let mut p = ui.painter();
        if k > 0.02 {
            let a = (k / 0.4).min(1.0);
            let b2 = ((k - 0.4) / 0.6).clamp(0.0, 1.0);
            p.line(p0, p0.lerp(p1, a), 2.3, Color::WHITE);
            if b2 > 0.0 {
                p.line(p1, p1.lerp(p2, b2), 2.3, Color::WHITE);
            }
        }
        p.text(vec2(r.rect.min.x + b + 9.0, r.rect.center().y - l.size.y * 0.5), &l, tc);
        ui.focus_ring(r.id, c, 7.0);
        r
    }
}

/// Glass well that fills with the accent color.
fn check_face(ui: &mut Ui, r: &Response, gel: &Gel, c: Rect, corner: Corner, t: f32) {
    let accent = ui.ctx.theme.accent;
    let t = t.clamp(0.0, 1.0);
    if ui.surface.is_glass() {
        let f = ui.surface.fill_strong();
        let mut p = ui.painter();
        p.fill(c, corner, f.lerp(accent, t));
        return;
    }
    let mut m = ui.ctx.theme.regular;
    m.refraction = 8.0;
    m.depth = 7.0;
    m.shadow *= 0.8;
    m.shadow_radius = 6.0;
    m.shadow_y = 2.0;
    m.dark_edge = 0.24;
    m.tint = Some(accent.alpha(0.92 * t));
    let mut g = Glass::new(c, corner, m);
    ui.ctx.prepare_glass(r.id, &mut g, true);
    g.glow = Some((gel.glow_at, gel.glow));
    ui.painter().glass(&g);
}

pub struct Radio<'a> {
    pub selected: bool,
    pub label: &'a str,
}

impl Widget for Radio<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let b = 22.0;
        let st = TextStyle::new(ui.ctx.theme.body);
        let l = ui.layout_text(self.label, &st);
        let r = ui.alloc_interact(vec2(b + 9.0 + l.size.x, 28.0), Sense::Click);
        ui.ctx.focusable(r.id);
        if r.hovered {
            ui.ctx.cursor = Cursor::Pointer;
        }
        let gel = Gel::update(ui.ctx, &r, 0.8);
        let t = ui.ctx.animate_bool(r.id.with("on"), self.selected, Tuning::new(0.34, 0.55));
        let c = Rect::from_center_size(
            vec2(r.rect.min.x + b * 0.5, r.rect.center().y),
            Vec2::splat(b * (1.0 + 0.12 * gel.press)),
        );
        check_face(ui, &r, &gel, c, Corner::Capsule, t);
        let tc = ui.surface.text();
        let mut p = ui.painter();
        let dot = Rect::from_center_size(c.center(), Vec2::splat((8.5 * t).max(0.0)));
        p.fill(dot, Corner::Round(10.0), Color::white(t.clamp(0.0, 1.0)));
        p.text(vec2(r.rect.min.x + b + 9.0, r.rect.center().y - l.size.y * 0.5), &l, tc);
        ui.focus_ring(r.id, c, Corner::Capsule);
        r
    }
}

/// Determinate progress: a tinted glass tube.
pub struct ProgressBar {
    pub value: f32,
    pub height: f32,
    pub tint: Option<Color>,
}

impl Widget for ProgressBar {
    fn ui(self, ui: &mut Ui) -> Response {
        let w = ui.available_width().clamp(60.0, 460.0);
        let r = ui.alloc_interact(vec2(w, self.height), Sense::Hover);
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

/// Ring progress.
pub struct Gauge<'a> {
    pub value: f32,
    pub size: f32,
    pub label: &'a str,
    pub tint: Color,
}

impl Widget for Gauge<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let r = ui.alloc_interact(Vec2::splat(self.size), Sense::Hover);
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

pub struct Spinner {
    pub size: f32,
}

impl Widget for Spinner {
    fn ui(self, ui: &mut Ui) -> Response {
        let r = ui.alloc_interact(Vec2::splat(self.size), Sense::Hover);
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

pub struct Image {
    pub id: ImageId,
    pub size: Vec2,
    pub corner: Corner,
}

impl Widget for Image {
    fn ui(self, ui: &mut Ui) -> Response {
        let r = ui.alloc_interact(self.size, Sense::Hover);
        ui.painter().image(r.rect, self.corner, self.id, rect(0.0, 0.0, 1.0, 1.0), Color::WHITE);
        r
    }
}

/// Section whose content unfolds.
pub struct Collapsing<'a> {
    pub title: &'a str,
    pub open: bool,
}

impl Collapsing<'_> {
    pub fn show(self, ui: &mut Ui, f: impl FnOnce(&mut Ui)) {
        let id = ui.make_id(("collapsing", self.title));
        #[derive(Clone, Copy)]
        struct State {
            open: bool,
            h: f32,
        }
        let mut s: State = ui.ctx.try_get(id).unwrap_or(State { open: self.open, h: 0.0 });
        let w = ui.available_width();
        let rect = ui.alloc(vec2(w, 30.0));
        let r = ui.interact(id, rect, Sense::Click);
        ui.ctx.focusable(id);
        if r.clicked {
            s.open = !s.open;
        }
        if r.hovered {
            ui.ctx.cursor = Cursor::Pointer;
        }
        let t = ui.ctx.animate_bool(id.with("t"), s.open, Tuning::new(0.40, 0.84));
        let st = TextStyle::new(ui.ctx.theme.body).weight(Weight::Semibold);
        let l = ui.layout_text(self.title, &st);
        let c = ui.surface.text();
        let cc = ui.surface.secondary();
        let m = vec2(rect.min.x + 8.0, rect.center().y);
        let a = t * TAU / 4.0;
        let rot = |v: Vec2| vec2(v.x * a.cos() - v.y * a.sin(), v.x * a.sin() + v.y * a.cos());
        let mut p = ui.painter();
        p.line(m + rot(vec2(-2.4, -5.0)), m + rot(vec2(2.8, 0.0)), 2.1, cc);
        p.line(m + rot(vec2(2.8, 0.0)), m + rot(vec2(-2.4, 5.0)), 2.1, cc);
        p.text(vec2(rect.min.x + 22.0, rect.center().y - l.size.y * 0.5), &l, c);
        ui.focus_ring(id, rect, 8.0);

        if t > 0.002 || s.h == 0.0 {
            let shown = s.h * t.max(0.0);
            let body =
                Rect::from_min_size(vec2(rect.min.x + 22.0, rect.max.y + 4.0), vec2(w - 22.0, 10_000.0));
            let (old_clip, old_op) = (ui.clip, ui.opacity);
            ui.clip = ui.clip.intersect(Rect {
                min: vec2(-1e6, rect.max.y),
                max: vec2(1e6, rect.max.y + 4.0 + shown + if t > 0.98 { 40.0 } else { 0.0 }),
            });
            ui.opacity *= ((t - 0.15) / 0.85).clamp(0.0, 1.0);
            let (_, size) = ui.in_rect_sized("body", body, f);
            ui.clip = old_clip;
            ui.opacity = old_op;
            if s.h == 0.0 {
                ui.ctx.request_relayout();
            }
            s.h = size.y + 4.0;
            ui.alloc(vec2(w, (shown - ui.spacing).max(0.0)));
        }
        ui.ctx.set(id, s);
    }
}

/// Swatch that opens a picker.
pub struct ColorPicker<'a> {
    pub color: &'a mut Color,
}

impl Widget for ColorPicker<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let mut r = ui.alloc_interact(vec2(46.0, 30.0), Sense::Click);
        ui.ctx.focusable(r.id);
        if r.hovered {
            ui.ctx.cursor = Cursor::Pointer;
        }
        let gel = Gel::update(ui.ctx, &r, 1.0);
        if r.clicked {
            ui.ctx.toggle_popup(r.id);
        }
        let mut m = ui.ctx.theme.regular;
        m.tint = Some(self.color.alpha(0.95));
        m.tint_follow = 0.4;
        let mut g = Glass::new(gel.rect, Corner::Capsule, m);
        g.glow = Some((gel.glow_at, gel.glow));
        ui.painter().glass(&g);

        #[derive(Clone, Copy, Default)]
        struct Hue {
            h: f32,
            hex: u32,
        }
        let mut hs: Hue = ui.ctx.get(r.id);
        if hs.hex != self.color.to_hex() {
            let (h, s, _) = self.color.to_hsv();
            // Keep the hue when the color has none.
            if s > 0.01 {
                hs.h = h;
            }
        }
        let before = self.color.to_hex();
        let color = &mut *self.color;
        crate::widgets::overlay::popup(ui.ctx, r.id, gel.rect, 244.0, |ui| {
            let (_, mut s, mut v) = color.to_hsv();
            let pad = ui.alloc_interact(vec2(212.0, 140.0), Sense::Drag);
            if pad.held
                && let Some(p) = pad.pointer
            {
                s = ((p.x - pad.rect.min.x) / pad.rect.width()).clamp(0.0, 1.0);
                v = 1.0 - ((p.y - pad.rect.min.y) / pad.rect.height()).clamp(0.0, 1.0);
            }
            let base = Color::from_hsv(hs.h, 1.0, 1.0);
            let dot = pad.rect.min + vec2(s * pad.rect.width(), (1.0 - v) * pad.rect.height());
            let lift = ui.ctx.animate_bool(pad.id, pad.held, Tuning::GEL);
            let lens = ui.ctx.theme.lens;
            let mut p = ui.painter();
            p.fill(pad.rect, 14.0, base);
            p.gradient(pad.rect, 14.0, Color::WHITE, Color::white(0.0), vec2(1.0, 0.0));
            p.gradient(pad.rect, 14.0, Color::black(0.0), Color::BLACK, vec2(0.0, 1.0));
            let d = Rect::from_center_size(dot, Vec2::splat(20.0 + 14.0 * lift));
            p.stroke(d, Corner::Capsule, 2.0, Color::white(0.95));
            p.glass(&Glass::new(d, Corner::Capsule, lens));

            let bar = ui.alloc_interact(vec2(212.0, 22.0), Sense::Drag);
            if bar.held
                && let Some(p) = bar.pointer
            {
                hs.h = ((p.x - bar.rect.min.x - 8.0) / (bar.rect.width() - 16.0)).clamp(0.0, 0.999);
            }
            let lift = ui.ctx.animate_bool(bar.id, bar.held, Tuning::GEL);
            let mut p = ui.painter();
            let track = bar.rect.shrink2(vec2(0.0, 6.0));
            let n = 6;
            let tm = p.add_mask(track, Corner::Capsule, false, Default::default());
            p.with_mask(tm, |p| {
                for i in 0..n {
                    let w = track.width() / n as f32;
                    let seg = Rect::from_min_size(
                        track.min + vec2(w * i as f32, 0.0),
                        vec2(w + 0.5, track.height()),
                    );
                    let a = Color::from_hsv(i as f32 / n as f32, 0.95, 1.0);
                    let b = Color::from_hsv((i + 1) as f32 / n as f32, 0.95, 1.0);
                    p.gradient(seg, 0.0, a, b, vec2(1.0, 0.0));
                }
            });
            let x = bar.rect.min.x + 8.0 + hs.h * (bar.rect.width() - 16.0);
            let d = Rect::from_center_size(
                vec2(x, bar.rect.center().y),
                vec2(26.0 + 8.0 * lift, 20.0 + 8.0 * lift),
            );
            p.glass(&Glass::new(d, Corner::Capsule, lens));
            if pad.held || bar.held {
                *color = Color::from_hsv(hs.h, s, v);
            }
            let hex = format!("#{:06X}", color.to_hex());
            ui.caption(&hex);
        });
        hs.hex = self.color.to_hex();
        ui.ctx.set(r.id, hs);
        r.changed = before != self.color.to_hex();
        r
    }
}

/// Row of a settings style list: icon, title, trailing content.
pub struct Row<'a> {
    pub icon: Option<(Icon, Color)>,
    pub title: &'a str,
}

impl Row<'_> {
    pub fn show(self, ui: &mut Ui, trailing: impl FnOnce(&mut Ui)) {
        let w = ui.available_width();
        let r = ui.alloc(vec2(w, 40.0));
        let st = TextStyle::new(ui.ctx.theme.body);
        let l = ui.layout_text(self.title, &st);
        let c = ui.surface.text();
        let mut x = r.min.x;
        let mut p = ui.painter();
        if let Some((i, bg)) = self.icon {
            let b = Rect::from_center_size(vec2(x + 14.0, r.center().y), Vec2::splat(28.0));
            p.gradient(b, 8.0, bg.lerp(Color::WHITE, 0.14), bg, vec2(0.0, 1.0));
            p.icon(b.center(), 17.0, i, Color::WHITE);
            x += 40.0;
        }
        p.text(vec2(x, r.center().y - l.size.y * 0.5), &l, c);
        let left = x + l.size.x + 16.0;
        let id = ui.next_id();
        // Right aligned by remembering last frame's width.
        #[derive(Clone, Copy, Default)]
        struct W(f32);
        let prev: Option<W> = ui.ctx.try_get(id);
        if prev.is_none() {
            ui.ctx.request_relayout();
        }
        let tw = prev.map_or(0.0, |w| w.0);
        let tr = Rect { min: vec2((r.max.x - tw).max(left), r.min.y), max: r.max };
        let (_, used) = ui.row_sized(id, tr, trailing);
        ui.ctx.set(id, W(used.x));
    }
}

impl Ui<'_> {
    pub fn checkbox(&mut self, on: &mut bool, label: &str) -> Response {
        self.add(Checkbox { on, label })
    }
    pub fn radio<T: PartialEq>(&mut self, value: &mut T, this: T, label: &str) -> Response {
        let mut r = self.add(Radio { selected: *value == this, label });
        if r.clicked {
            r.changed = *value != this;
            *value = this;
        }
        r
    }
    pub fn progress(&mut self, value: f32) -> Response {
        self.add(ProgressBar { value, height: 8.0, tint: None })
    }
    pub fn spinner(&mut self) -> Response {
        self.add(Spinner { size: 24.0 })
    }
    pub fn image(&mut self, id: ImageId, size: Vec2, corner: impl Into<Corner>) -> Response {
        self.add(Image { id, size, corner: corner.into() })
    }
    pub fn collapsing(&mut self, title: &str, f: impl FnOnce(&mut Ui)) {
        Collapsing { title, open: false }.show(self, f);
    }
    pub fn color_picker(&mut self, color: &mut Color) -> Response {
        self.add(ColorPicker { color })
    }
}
