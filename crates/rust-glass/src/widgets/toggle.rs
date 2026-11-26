//! Switch.

use crate::accessibility::{Action, Role};
use crate::animation::Tuning;
use crate::color::Color;
use crate::context::{Cursor, Response, Sense};
use crate::geometry::{Rect, vec2};
use crate::input::Key;
use crate::material::Corner;
use crate::text::TextStyle;
use crate::ui::{Ui, Widget};
use crate::widgets::thumb::{drag_target, load_thumb, store_thumb, thumb_glass, thumb_rect};

/// On and off.
pub struct Toggle<'a> {
    on: &'a mut bool,
    label: &'a str,
    tint: Color,
    scale: f32,
}

impl<'a> Toggle<'a> {
    /// Creates a switch for `on`.
    pub fn new(on: &'a mut bool) -> Self {
        Self { on, label: "", tint: Color::GREEN, scale: 1.0 }
    }
    /// Shows `label` before the switch. In a vertical layout the row then
    /// takes the full width, with the switch at the end.
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = label;
        self
    }
    /// Sets the color when on. The default is green.
    pub fn tint(mut self, color: Color) -> Self {
        self.tint = color;
        self
    }
    /// Scales the switch.
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }
}

impl Widget for Toggle<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let k = self.scale;
        let size = vec2(60.0, 27.0) * k;
        let rest = vec2(36.0, 23.0) * k;
        let lifted = vec2(52.0, 35.0) * k;
        let st = TextStyle::new(ui.ctx.theme.body_size);
        let text = (!self.label.is_empty()).then(|| ui.layout_text(self.label, &st));
        let fill_row = text.is_some() && !ui.is_horizontal();
        let tw = text.as_ref().map_or(0.0, |t| t.size.x + 12.0);
        let total = if fill_row { ui.available_width() } else { size.x + tw };
        let rect = ui.allocate(vec2(total, size.y + 4.0));
        let id = ui.next_id();
        let track = Rect::from_min_size(vec2(rect.max.x - size.x, rect.center().y - size.y * 0.5), size)
            .snap(ui.ctx.pixels_per_point);
        let mut r = ui.interact(id, rect, Sense::Drag);
        ui.ctx.focusable(id);
        if r.hovered {
            ui.ctx.cursor = Cursor::Pointer;
        }
        let pad = 2.0 * k;
        let ox = track.min.x;
        let x0 = pad + rest.x * 0.5;
        let x1 = track.width() - pad - rest.x * 0.5;
        let mut d = load_thumb(ui.ctx, id);
        let before = *self.on;
        let dragged = drag_target(ui.ctx, id, &r, ox, d.pos.value, x0, x1, 5.0 * k);
        let mid = (x0 + x1) * 0.5;
        if r.released {
            let moved = r.pointer.is_some_and(|p| (p.x - ui.ctx.input.press_origin.x).abs() > 4.0);
            *self.on = if moved { d.pos.value > mid } else { !before };
        }
        if (r.focused && (ui.ctx.input.take_key(Key::Space) || ui.ctx.input.take_key(Key::Enter)))
            || ui.ctx.access_requested(id, &Action::Click)
        {
            *self.on = !*self.on;
        }
        if let Some(n) = ui.access(id, Role::Switch, rect) {
            n.label(self.label).toggled(*self.on);
        }
        r.changed = before != *self.on;
        let home = if *self.on { x1 } else { x0 };
        // Without a drag a press only lifts the thumb where it is.
        let moved = r.pointer.is_some_and(|p| (p.x - ui.ctx.input.press_origin.x).abs() > 3.0);
        let target = dragged.filter(|_| moved).unwrap_or(home);
        if r.changed && d.lift.value < 0.3 {
            d.lift.value = 0.31;
        }
        let follow =
            if dragged.is_some() && moved { Tuning::new(0.14, 0.9) } else { Tuning::new(0.32, 0.80) };
        let f = d.step(ui.ctx, target, follow, r.held, rest, lifted);
        store_thumb(ui.ctx, id, d);

        let t = ((d.pos.value - x0) / (x1 - x0)).clamp(0.0, 1.0);
        let off = ui.surface.strong_fill_color();
        let knob = ui.surface.thumb_color();
        let thumb = thumb_rect(ui.ctx, vec2(ox + d.pos.value, track.center().y), &f);
        let glass = thumb_glass(ui.ctx, thumb, &f, knob, k);
        let tc = ui.surface.text_color();
        let on_c = self.tint;
        let mut p = ui.painter();
        if let Some(l) = &text {
            p.text(vec2(rect.min.x, rect.center().y - l.size.y * 0.5), l, tc);
        }
        p.fill(track, Corner::Capsule, off);
        p.fill(track, Corner::Capsule, on_c.fade(t));
        p.glass(&glass);
        ui.focus_ring(id, track, Corner::Capsule);
        r
    }
}

impl Ui<'_> {
    /// Adds a switch with a label.
    pub fn toggle(&mut self, on: &mut bool, label: &str) -> Response {
        self.add(Toggle::new(on).label(label))
    }
}
