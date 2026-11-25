use crate::anim::{Spring, Tuning};
use crate::color::Color;
use crate::context::Sense;
use crate::geom::{Rect, vec2};
use crate::material::Corner;
use crate::paint::Feather;
use crate::ui::Ui;
use crate::widgets::gel::rubber;

/// Vertical scrolling with smooth wheel steps, elastic ends and an overlay
/// scroll bar. `fade` dissolves content at the edges, which keeps floating
/// glass legible without a solid bar behind it.
pub struct ScrollArea {
    height: Option<f32>,
    fade_top: f32,
    fade_bottom: f32,
    inset_top: f32,
    inset_bottom: f32,
    id: &'static str,
}

#[derive(Clone, Copy, Default)]
struct State {
    offset: Spring,
    target: f32,
    content: f32,
    bar: Spring,
    last_move: f64,
    grab: f32,
}

impl Default for ScrollArea {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollArea {
    pub fn new() -> Self {
        Self {
            height: None,
            fade_top: 0.0,
            fade_bottom: 0.0,
            inset_top: 0.0,
            inset_bottom: 0.0,
            id: "scroll",
        }
    }
    pub fn id(mut self, id: &'static str) -> Self {
        self.id = id;
        self
    }
    pub fn height(mut self, h: f32) -> Self {
        self.height = Some(h);
        self
    }
    pub fn fade(mut self, top: f32, bottom: f32) -> Self {
        self.fade_top = top;
        self.fade_bottom = bottom;
        self
    }
    /// Space kept free for floating bars. Content scrolls below them.
    pub fn insets(mut self, top: f32, bottom: f32) -> Self {
        self.inset_top = top;
        self.inset_bottom = bottom;
        self
    }

    pub fn show<R>(self, ui: &mut Ui, f: impl FnOnce(&mut Ui) -> R) -> R {
        let id = ui.make_id(self.id);
        let h = self.height.unwrap_or_else(|| ui.available_height());
        let w = ui.available_width();
        let view = ui.alloc(vec2(w, h));
        let mut s: State = ui.ctx.get(id);
        let dt = ui.ctx.dt;
        let max = (s.content - h).max(0.0);
        let wheel = ui.ctx.scroll_area(id, view.intersect(ui.clip), ui.layer);
        if wheel.y != 0.0 {
            s.target -= wheel.y;
            s.last_move = ui.ctx.time;
            ui.ctx.add_scroll_energy(wheel.y * 0.004);
        }
        // Past the ends the target is pulled back, so it gives and returns.
        let over = s.target - s.target.clamp(0.0, max);
        if over != 0.0 {
            let limit = if ui.ctx.theme.reduce_motion { 0.5 } else { 46.0 };
            s.target = s.target.clamp(0.0, max) + rubber(over, limit);
            let back = (-dt * 11.0).exp();
            let o = s.target - s.target.clamp(0.0, max);
            s.target = s.target.clamp(0.0, max) + if o.abs() < 0.4 { 0.0 } else { o * back };
            ui.ctx.request_repaint();
        }
        if s.offset.step(s.target, Tuning::new(0.17, 1.0), dt) {
            s.last_move = ui.ctx.time;
            ui.ctx.request_repaint();
        }
        let y = s.offset.x;
        let shown_y = (y * ui.ctx.ppp).round() / ui.ctx.ppp;

        let (old_clip, old_mask) = (ui.clip, ui.mask);
        // Room at the sides for shadows and for glass that swells under a press.
        let sides = view.expand2(vec2(30.0, 0.0));
        ui.clip = ui.clip.intersect(sides);
        if self.fade_top > 0.0 || self.fade_bottom > 0.0 {
            let feather = Feather {
                top: self.fade_top * (y / 24.0).clamp(0.0, 1.0),
                bottom: self.fade_bottom * ((max - y) / 24.0).clamp(0.0, 1.0),
                ..Default::default()
            };
            ui.mask = ui.painter().add_mask(sides, 0.0, false, feather);
        }
        let body = Rect {
            min: vec2(view.min.x, view.min.y - shown_y + self.inset_top),
            max: vec2(view.max.x, view.min.y - shown_y + 1.0e5),
        };
        let (out, used) = ui.in_rect_sized(id, body, f);
        ui.clip = old_clip;
        ui.mask = old_mask;
        let content = used.y + self.inset_top + self.inset_bottom;
        if (content - s.content).abs() > 0.5 {
            s.content = content;
            ui.ctx.request_repaint();
        }

        // Scroll bar.
        if max > 1.0 {
            let t0 = view.min.y + self.inset_top.max(6.0);
            let t1 = view.max.y - self.inset_bottom.max(6.0);
            let track = t1 - t0;
            let len = (h / s.content * track).clamp(28.0, track);
            let over = (y - y.clamp(0.0, max)).abs();
            let len = (len - over * 0.6).max(12.0);
            let top = t0 + (track - len) * (y / max).clamp(0.0, 1.0);
            let hit = Rect { min: vec2(view.max.x - 14.0, top), max: vec2(view.max.x, top + len) };
            let r = ui.interact(id.with("bar"), hit, Sense::Drag);
            if r.pressed {
                s.grab = ui.ctx.input.press_origin.y - top;
            }
            if r.held
                && let Some(p) = r.pointer
            {
                s.target = ((p.y - s.grab - t0) / (track - len).max(1.0)).clamp(0.0, 1.0) * max;
                s.offset.set(s.target);
            }
            let recent = ui.ctx.time - s.last_move < 0.9;
            let on = recent || r.hovered || r.held;
            if s.bar.step(on as u32 as f32, Tuning::new(0.3, 1.0), dt) || recent {
                ui.ctx.request_repaint();
            }
            let wide = ui.ctx.animate_bool(id.with("wide"), r.hovered || r.held, Tuning::SNAP);
            let c = Color::black(0.38).lerp(Color::white(0.45), ui.surface.dark());
            let bw = 4.0 + 3.0 * wide;
            let bar = Rect { min: vec2(view.max.x - 4.0 - bw, top), max: vec2(view.max.x - 4.0, top + len) };
            let a = s.bar.x;
            ui.painter().fill(bar, Corner::Capsule, c.fade(a));
        }
        ui.ctx.set(id, s);
        out
    }
}
