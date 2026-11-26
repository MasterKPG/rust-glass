//! Collapsing section.

use std::f32::consts::TAU;

use crate::accessibility::Role;
use crate::animation::Tuning;
use crate::context::{Cursor, Sense};
use crate::geometry::{Rect, Vec2, vec2};
use crate::text::{TextStyle, Weight};
use crate::ui::Ui;

/// A title that shows or hides the content below it when clicked.
pub struct Collapsing<'a> {
    title: &'a str,
    open: bool,
}

impl<'a> Collapsing<'a> {
    /// Creates a closed section.
    pub fn new(title: &'a str) -> Self {
        Self { title, open: false }
    }
    /// Sets whether the section starts open. Only the first frame counts.
    pub fn default_open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Shows the title, and the content drawn by `f` while open.
    pub fn show(self, ui: &mut Ui, f: impl FnOnce(&mut Ui)) {
        let id = ui.make_id(("collapsing", self.title));
        #[derive(Clone, Copy)]
        struct State {
            open: bool,
            h: f32,
        }
        let mut s: State = ui.ctx.try_get(id).unwrap_or(State { open: self.open, h: 0.0 });
        let w = ui.available_width();
        let rect = ui.allocate(vec2(w, 30.0));
        let r = ui.interact(id, rect, Sense::Click);
        ui.ctx.focusable(id);
        if r.clicked {
            s.open = !s.open;
        }
        if let Some(n) = ui.access(id, Role::Button, rect) {
            n.label(self.title).expanded(s.open);
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
            ui.allocate(vec2(w, (shown - ui.spacing).max(0.0)));
        }
        ui.ctx.set(id, s);
    }
}

impl Ui<'_> {
    pub fn collapsing(&mut self, title: &str, f: impl FnOnce(&mut Ui)) {
        Collapsing::new(title).show(self, f);
    }
}
