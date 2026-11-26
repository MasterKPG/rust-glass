use std::ops::Range;

use crate::anim::{Spring, Tuning};
use crate::color::Color;
use crate::context::{Id, Sense};
use crate::geom::{Rect, Vec2, vec2};
use crate::material::Corner;
use crate::paint::Feather;
use crate::ui::Ui;
use crate::widgets::gel::rubber;

/// Scrolling with smooth wheel steps, elastic ends and overlay scroll bars.
/// Vertical by default; [`Self::horizontal`] lays the content out in a row
/// and [`Self::both`] scrolls on two axes. `fade` dissolves content at the
/// edges, which keeps floating glass legible without a solid bar behind it.
///
/// Long lists draw only what is visible with [`Self::show_rows`].
pub struct ScrollArea {
    height: Option<f32>,
    width: Option<f32>,
    x: bool,
    y: bool,
    content_width: Option<f32>,
    fade: Feather,
    inset_top: f32,
    inset_bottom: f32,
    id: &'static str,
}

#[derive(Clone, Copy, Default)]
struct Axis {
    offset: Spring,
    target: f32,
    content: f32,
    grab: f32,
}

#[derive(Clone, Copy, Default)]
struct State {
    axes: [Axis; 2],
    bar: Spring,
    last_move: f64,
}

impl Default for ScrollArea {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollArea {
    /// Vertical scrolling.
    pub fn new() -> Self {
        Self {
            height: None,
            width: None,
            x: false,
            y: true,
            content_width: None,
            fade: Feather::default(),
            inset_top: 0.0,
            inset_bottom: 0.0,
            id: "scroll",
        }
    }
    /// Horizontal scrolling. Children are placed side by side and centered
    /// on the height of the area. Vertical wheel movement passes to the area
    /// around it; shift with the wheel or a sideways swipe scrolls this one.
    pub fn horizontal() -> Self {
        Self { x: true, y: false, ..Self::new() }
    }
    /// Scrolling on both axes, for content larger than the view both ways.
    /// Children are stacked vertically in a column [`Self::content_width`]
    /// wide, or as wide as they need when that is not set.
    pub fn both() -> Self {
        Self { x: true, ..Self::new() }
    }
    pub fn id(mut self, id: &'static str) -> Self {
        self.id = id;
        self
    }
    pub fn height(mut self, h: f32) -> Self {
        self.height = Some(h);
        self
    }
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    /// Width of the column the content is laid out in, for [`Self::both`].
    pub fn content_width(mut self, w: f32) -> Self {
        self.content_width = Some(w);
        self
    }
    /// Dissolve content over this distance at the top and bottom.
    pub fn fade(mut self, top: f32, bottom: f32) -> Self {
        self.fade.top = top;
        self.fade.bottom = bottom;
        self
    }
    /// Dissolve content over this distance at the left and right.
    pub fn fade_sides(mut self, left: f32, right: f32) -> Self {
        self.fade.left = left;
        self.fade.right = right;
        self
    }
    /// Space kept free for floating bars. Content scrolls below them.
    pub fn insets(mut self, top: f32, bottom: f32) -> Self {
        self.inset_top = top;
        self.inset_bottom = bottom;
        self
    }

    pub fn show<R>(self, ui: &mut Ui, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.show_inner(ui, |ui, _| f(ui))
    }

    /// A vertical list of `count` rows, each `row_height` tall, of which only
    /// the visible ones are laid out. `f` gets the range of rows to draw and
    /// must give each row exactly `row_height`.
    ///
    /// ```
    /// # use rust_glass::prelude::*;
    /// # fn f(ui: &mut Ui, names: &[String]) {
    /// ScrollArea::new().height(300.0).show_rows(ui, 28.0, names.len(), |ui, rows| {
    ///     for i in rows {
    ///         ui.label(&names[i]);
    ///     }
    /// });
    /// # }
    /// ```
    pub fn show_rows<R>(
        self,
        ui: &mut Ui,
        row_height: f32,
        count: usize,
        f: impl FnOnce(&mut Ui, Range<usize>) -> R,
    ) -> R {
        let inset = self.inset_top;
        self.show_inner(ui, |ui, visible| {
            let stride = row_height + ui.spacing;
            let top = (visible.min.y - inset).max(0.0);
            let first = ((top / stride).floor() as usize).min(count);
            let n = (visible.height() / stride).ceil() as usize + 2;
            let last = (first + n).min(count);
            ui.space(first as f32 * stride);
            let out = f(ui, first..last);
            let total = (count as f32 * stride - ui.spacing).max(0.0);
            ui.reserve(vec2(0.0, total));
            out
        })
    }

    /// `f` gets the visible part of the content, relative to its top left.
    fn show_inner<R>(self, ui: &mut Ui, f: impl FnOnce(&mut Ui, Rect) -> R) -> R {
        let id = ui.make_id(self.id);
        let h = self.height.unwrap_or_else(|| ui.available_height());
        let w = self.width.unwrap_or_else(|| ui.available_width());
        let view = ui.alloc(vec2(w, h));
        let mut s: State = ui.ctx.get(id);
        let dt = ui.ctx.dt;
        let on = [self.x, self.y];
        let size = [w, h];
        let max = [0, 1].map(|a| (s.axes[a].content - size[a]).max(0.0));
        let wheel = ui.ctx.scroll_area_axes(id, view.intersect(ui.clip), ui.layer, self.x, self.y);
        let wheel = [wheel.x, wheel.y];
        let limit = if ui.ctx.theme.reduce_motion { 0.5 } else { 46.0 };
        let mut shown = [0.0; 2];
        for a in 0..2 {
            let ax = &mut s.axes[a];
            if !on[a] {
                *ax = Axis::default();
                continue;
            }
            if wheel[a] != 0.0 {
                ax.target -= wheel[a];
                s.last_move = ui.ctx.time;
                ui.ctx.add_scroll_energy(wheel[a] * 0.004);
            }
            // Past the ends the target is pulled back, so it gives and returns.
            let over = ax.target - ax.target.clamp(0.0, max[a]);
            if over != 0.0 {
                ax.target = ax.target.clamp(0.0, max[a]) + rubber(over, limit);
                let back = (-dt * 11.0).exp();
                let o = ax.target - ax.target.clamp(0.0, max[a]);
                ax.target = ax.target.clamp(0.0, max[a]) + if o.abs() < 0.4 { 0.0 } else { o * back };
                ui.ctx.request_repaint();
            }
            if ax.offset.step(ax.target, Tuning::new(0.17, 1.0), dt) {
                s.last_move = ui.ctx.time;
                ui.ctx.request_repaint();
            }
            shown[a] = (ax.offset.x * ui.ctx.ppp).round() / ui.ctx.ppp;
        }
        let off = [s.axes[0].offset.x, s.axes[1].offset.x];

        let (old_clip, old_mask) = (ui.clip, ui.mask);
        // Room for shadows and for glass that swells under a press, on the
        // sides that do not scroll.
        let room = vec2(if self.x { 0.0 } else { 30.0 }, if self.y { 0.0 } else { 30.0 });
        let sides = view.expand2(room);
        ui.clip = ui.clip.intersect(sides);
        let edge = |o: f32, m: f32| (o / 24.0).clamp(0.0, 1.0) * (m > 0.5) as u32 as f32;
        let feather = Feather {
            left: self.fade.left * edge(off[0], max[0]),
            right: self.fade.right * edge(max[0] - off[0], max[0]),
            top: self.fade.top * edge(off[1], max[1]),
            bottom: self.fade.bottom * edge(max[1] - off[1], max[1]),
        };
        if self.fade.left + self.fade.right + self.fade.top + self.fade.bottom > 0.0 {
            ui.mask = ui.painter().add_mask(sides, 0.0, false, feather);
        }
        let min = vec2(view.min.x - shown[0], view.min.y - shown[1] + self.inset_top);
        let visible = Rect::from_min_size(vec2(off[0], off[1]), view.size());
        let (out, used) = if self.x && !self.y {
            let body = Rect { min, max: vec2(min.x + 1.0e5, view.max.y - self.inset_bottom) };
            ui.row_sized(id, body, |ui| f(ui, visible))
        } else {
            let width = if self.x { self.content_width.unwrap_or(1.0e5) } else { view.width() };
            let body = Rect { min, max: vec2(min.x + width, min.y + 1.0e5) };
            ui.in_rect_sized(id, body, |ui| f(ui, visible))
        };
        ui.clip = old_clip;
        ui.mask = old_mask;
        let content = [used.x, used.y + self.inset_top + self.inset_bottom];
        for a in 0..2 {
            if on[a] && (content[a] - s.axes[a].content).abs() > 0.5 {
                s.axes[a].content = content[a];
                ui.ctx.request_repaint();
            }
        }

        // Scroll bars.
        let recent = ui.ctx.time - s.last_move < 0.9;
        let mut engaged = false;
        let mut bars = Vec::new();
        for a in 0..2 {
            if !on[a] || max[a] <= 1.0 {
                continue;
            }
            let (t0, t1) = if a == 1 {
                (view.min.y + self.inset_top.max(6.0), view.max.y - self.inset_bottom.max(6.0))
            } else {
                (view.min.x + 6.0, view.max.x - if on[1] && max[1] > 1.0 { 14.0 } else { 6.0 })
            };
            let ax = &mut s.axes[a];
            let track = t1 - t0;
            let len = (size[a] / ax.content * track).clamp(28.0, track);
            let over = (off[a] - off[a].clamp(0.0, max[a])).abs();
            let len = (len - over * 0.6).max(12.0);
            let start = t0 + (track - len) * (off[a] / max[a]).clamp(0.0, 1.0);
            let hit = if a == 1 {
                Rect { min: vec2(view.max.x - 14.0, start), max: vec2(view.max.x, start + len) }
            } else {
                Rect { min: vec2(start, view.max.y - 14.0), max: vec2(start + len, view.max.y) }
            };
            let r = ui.interact(id.with(("bar", a)), hit, Sense::Drag);
            let p0 = if a == 1 { ui.ctx.input.press_origin.y } else { ui.ctx.input.press_origin.x };
            if r.pressed {
                ax.grab = p0 - start;
            }
            if r.held
                && let Some(p) = r.pointer
            {
                let p = if a == 1 { p.y } else { p.x };
                ax.target = ((p - ax.grab - t0) / (track - len).max(1.0)).clamp(0.0, 1.0) * max[a];
                ax.offset.set(ax.target);
            }
            engaged |= r.hovered || r.held;
            bars.push((a, r.id, r.hovered || r.held, start, len));
        }
        if s.bar.step((recent || engaged) as u32 as f32, Tuning::new(0.3, 1.0), dt) || recent {
            ui.ctx.request_repaint();
        }
        let c = Color::black(0.38).lerp(Color::white(0.45), ui.surface.dark());
        let alpha = s.bar.x;
        for (a, bar_id, hot, start, len) in bars {
            let wide = ui.ctx.animate_bool(bar_id.with("wide"), hot, Tuning::SNAP);
            let bw = 4.0 + 3.0 * wide;
            let r = if a == 1 {
                Rect { min: vec2(view.max.x - 4.0 - bw, start), max: vec2(view.max.x - 4.0, start + len) }
            } else {
                Rect { min: vec2(start, view.max.y - 4.0 - bw), max: vec2(start + len, view.max.y - 4.0) }
            };
            ui.painter().fill(r, Corner::Capsule, c.fade(alpha));
        }
        ui.ctx.set(id, s);
        out
    }
}

/// Scroll position of the area `id` inside `ui`, for saving and restoring.
pub fn scroll_offset(ui: &mut Ui, id: &'static str) -> Vec2 {
    let id: Id = ui.make_id(id);
    let s: State = ui.ctx.get(id);
    vec2(s.axes[0].target, s.axes[1].target)
}

/// Jump the area `id` inside `ui` to `offset`. Call before the area is shown.
pub fn set_scroll_offset(ui: &mut Ui, id: &'static str, offset: Vec2) {
    let id: Id = ui.make_id(id);
    let mut s: State = ui.ctx.get(id);
    for (a, v) in [offset.x, offset.y].into_iter().enumerate() {
        s.axes[a].target = v.max(0.0);
        s.axes[a].offset.set(v.max(0.0));
    }
    ui.ctx.set(id, s);
}
