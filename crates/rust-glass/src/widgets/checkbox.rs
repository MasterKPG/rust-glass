//! Checkbox and radio button.

use crate::accessibility::Role;
use crate::animation::Tuning;
use crate::color::Color;
use crate::context::{Cursor, Response, Sense};
use crate::geometry::{Rect, Vec2, vec2};
use crate::material::{Corner, Glass};
use crate::text::TextStyle;
use crate::ui::{Ui, Widget};
use crate::widgets::press::PressEffect;

/// A box that is checked or not, with a label.
pub struct Checkbox<'a> {
    on: &'a mut bool,
    label: &'a str,
}

impl<'a> Checkbox<'a> {
    /// Creates a checkbox for `on`.
    pub fn new(on: &'a mut bool, label: &'a str) -> Self {
        Self { on, label }
    }
}

impl Widget for Checkbox<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let b = 22.0;
        let st = TextStyle::new(ui.ctx.theme.body_size);
        let l = ui.layout_text(self.label, &st);
        let mut r = ui.allocate_response(vec2(b + 9.0 + l.size.x, 28.0), Sense::Click);
        ui.ctx.focusable(r.id);
        if r.clicked {
            *self.on = !*self.on;
            r.changed = true;
        }
        if let Some(n) = ui.access(r.id, Role::CheckBox, r.rect) {
            n.label(self.label).toggled(*self.on);
        }
        if r.hovered {
            ui.ctx.cursor = Cursor::Pointer;
        }
        let gel = PressEffect::update(ui.ctx, &r, 0.8);
        let t = ui.ctx.animate_bool(r.id.with("on"), *self.on, Tuning::new(0.32, 0.62));
        let c = Rect::from_center_size(
            vec2(r.rect.min.x + b * 0.5, r.rect.center().y),
            Vec2::splat(b * (1.0 + 0.12 * gel.amount)),
        );
        check_face(ui, &r, &gel, c, Corner::Radius(7.0), t);
        // Tick drawn stroke by stroke.
        let k = (t * 1.1).clamp(0.0, 1.0);
        let p0 = c.center() + vec2(-5.0, 0.4);
        let p1 = c.center() + vec2(-1.5, 4.0);
        let p2 = c.center() + vec2(5.2, -4.0);
        let tc = ui.surface.text_color();
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
fn check_face(ui: &mut Ui, r: &Response, gel: &PressEffect, c: Rect, corner: Corner, t: f32) {
    let accent = ui.ctx.theme.accent;
    let t = t.clamp(0.0, 1.0);
    if ui.surface.is_glass() {
        let f = ui.surface.strong_fill_color();
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
    g.brightness = 0.05 * gel.amount.max(0.0);
    ui.painter().glass(&g);
}

/// One choice of a group. Usually added with [`Ui::radio`], which handles
/// the group value.
pub struct Radio<'a> {
    selected: bool,
    label: &'a str,
}

impl<'a> Radio<'a> {
    /// Creates a radio button that shows as selected or not.
    pub fn new(selected: bool, label: &'a str) -> Self {
        Self { selected, label }
    }
}

impl Widget for Radio<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let b = 22.0;
        let st = TextStyle::new(ui.ctx.theme.body_size);
        let l = ui.layout_text(self.label, &st);
        let r = ui.allocate_response(vec2(b + 9.0 + l.size.x, 28.0), Sense::Click);
        ui.ctx.focusable(r.id);
        if let Some(n) = ui.access(r.id, Role::RadioButton, r.rect) {
            n.label(self.label).toggled(self.selected);
        }
        if r.hovered {
            ui.ctx.cursor = Cursor::Pointer;
        }
        let gel = PressEffect::update(ui.ctx, &r, 0.8);
        let t = ui.ctx.animate_bool(r.id.with("on"), self.selected, Tuning::new(0.34, 0.55));
        let c = Rect::from_center_size(
            vec2(r.rect.min.x + b * 0.5, r.rect.center().y),
            Vec2::splat(b * (1.0 + 0.12 * gel.amount)),
        );
        check_face(ui, &r, &gel, c, Corner::Capsule, t);
        let tc = ui.surface.text_color();
        let mut p = ui.painter();
        let dot = Rect::from_center_size(c.center(), Vec2::splat((8.5 * t).max(0.0)));
        p.fill(dot, Corner::Circular(10.0), Color::white(t.clamp(0.0, 1.0)));
        p.text(vec2(r.rect.min.x + b + 9.0, r.rect.center().y - l.size.y * 0.5), &l, tc);
        ui.focus_ring(r.id, c, Corner::Capsule);
        r
    }
}

impl Ui<'_> {
    /// Adds a checkbox for `on`.
    pub fn checkbox(&mut self, on: &mut bool, label: &str) -> Response {
        self.add(Checkbox::new(on, label))
    }
    /// Adds a radio button that sets `value` to `this` when clicked.
    pub fn radio<T: PartialEq>(&mut self, value: &mut T, this: T, label: &str) -> Response {
        let mut r = self.add(Radio::new(*value == this, label));
        if r.clicked {
            r.changed = *value != this;
            *value = this;
        }
        r
    }
}
