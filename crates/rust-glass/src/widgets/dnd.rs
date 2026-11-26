//! Drag and drop inside a window: pick up content, carry it on a lifted
//! glass plate and drop it on a target that accepts its payload type.

use std::any::Any;
use std::hash::Hash;

use crate::anim::Tuning;
use crate::context::{Cursor, Id, Order, Response, Sense};
use crate::geom::{Rect, Vec2, vec2};
use crate::material::Corner;
use crate::ui::Ui;

/// Movement in points before a press turns into a drag.
const SLOP: f32 = 6.0;

#[derive(Clone, Copy)]
struct Reorder {
    list: Id,
    from: usize,
}

impl Ui<'_> {
    /// Content that can be picked up and dropped on a [`Ui::drop_zone`] that
    /// accepts `P`. `f` draws the content; while it is dragged, `f` runs a
    /// second time to draw the lifted copy under the pointer.
    ///
    /// ```
    /// # use rust_glass::prelude::*;
    /// # fn f(ui: &mut Ui, fruit: &mut Vec<String>, basket: &mut Vec<String>) {
    /// for (i, name) in fruit.iter().enumerate() {
    ///     ui.drag_source(("fruit", i), i, |ui| {
    ///         ui.label(name);
    ///     });
    /// }
    /// let (_, dropped) = ui.drop_zone::<usize, _>("basket", vec2(200.0, 80.0), |ui| {
    ///     for name in basket.iter() {
    ///         ui.label(name);
    ///     }
    /// });
    /// if let Some(i) = dropped {
    ///     basket.push(fruit.remove(i));
    /// }
    /// # }
    /// ```
    pub fn drag_source<P: Any + Clone>(
        &mut self,
        id: impl Hash,
        payload: P,
        mut f: impl FnMut(&mut Ui),
    ) -> Response {
        let id = self.make_id(id);
        let dragging = self.ctx.dragged_id() == Some(id);
        let old = self.opacity;
        if dragging {
            // The slot stays as a faint outline of what was picked up.
            self.opacity *= 0.28;
        }
        let ((), rect) = self.scope_id(id.with("slot"), &mut f);
        self.opacity = old;
        let mut r = self.interact(id, rect, Sense::Drag);
        if r.hovered && !dragging {
            self.ctx.cursor = Cursor::Grab;
        }
        if r.held
            && !dragging
            && self.ctx.dragged_id().is_none()
            && let Some(p) = r.pointer
            && (p - self.ctx.input.press_origin).length() > SLOP
        {
            let grab = self.ctx.input.press_origin - rect.min;
            self.ctx.start_drag(id, Box::new(payload), grab, rect.size());
        }
        let lift = self.ctx.animate_bool(id.with("lift"), dragging, Tuning::new(0.28, 0.72));
        if self.ctx.dragged_id() == Some(id)
            && let Some(at) = self.ctx.drag_rect()
        {
            let layer = self.ctx.layer(Id::new("drag and drop"), Order::Tooltip);
            let mut ui = Ui::new(self.ctx, layer, id.with("lifted"), at);
            ui.spacing = self.spacing;
            // A plate of glass carries the content and swells as it lifts.
            let plate = at.expand(6.0 + 4.0 * lift.clamp(0.0, 1.2));
            let m = ui.ctx.theme.regular;
            ui.surface = ui.glass(id.with("plate"), plate, Corner::Radius(18.0), m, true);
            f(&mut ui);
            r.changed = true;
        }
        r
    }

    /// Region that accepts dropped payloads of type `P`. It is at least
    /// `min_size` large and lights up while a matching payload is over it.
    /// Returns the payload in the frame it is dropped.
    pub fn drop_zone<P: Any, R>(
        &mut self,
        id: impl Hash,
        min_size: Vec2,
        f: impl FnOnce(&mut Ui) -> R,
    ) -> (R, Option<P>) {
        let id = self.make_id(id);
        let (out, rect) = self.scope_id(id, |ui| {
            let out = f(ui);
            ui.reserve(min_size);
            out
        });
        let over = self.ctx.drag_over::<P>(rect);
        let t = self.ctx.animate_bool(id.with("over"), over, Tuning::SNAP);
        if t > 0.005 {
            let c = self.ctx.theme.accent;
            let mut p = self.painter();
            p.fill(rect.expand(4.0), 14.0, c.alpha(0.10 * t));
            p.stroke(rect.expand(4.0), 14.0, 2.0, c.alpha(0.65 * t));
        }
        (out, self.ctx.take_drop::<P>(rect))
    }

    /// A vertical list whose items can be dragged into a new order. `T` is
    /// hashed for the id of each item, so equal items need something that
    /// tells them apart. Returns true in the frame the order changed.
    ///
    /// ```
    /// # use rust_glass::prelude::*;
    /// # fn f(ui: &mut Ui, tasks: &mut Vec<String>) {
    /// ui.reorder("tasks", tasks, |ui, task| {
    ///     ui.label(task);
    /// });
    /// # }
    /// ```
    pub fn reorder<T: Hash>(
        &mut self,
        id: impl Hash,
        items: &mut Vec<T>,
        mut f: impl FnMut(&mut Ui, &T),
    ) -> bool {
        let list = self.make_id(id);
        let mut rects = Vec::with_capacity(items.len());
        for (i, it) in items.iter().enumerate() {
            let r = self.drag_source((list, it), Reorder { list, from: i }, |ui| f(ui, it));
            rects.push(r.rect);
        }
        let Some(d) = self.ctx.dragged::<Reorder>().filter(|d| d.list == list) else { return false };
        let Some(p) = self.ctx.input.pointer else { return false };
        let area = rects.iter().fold(Rect::NOTHING, |a, r| a.union(*r));
        if rects.is_empty() || !area.expand2(vec2(48.0, 28.0)).contains(p) {
            return false;
        }
        let to = rects.iter().filter(|r| p.y > r.center().y).count();
        let gap = self.spacing * 0.5;
        let y = match to {
            0 => rects[0].min.y - gap,
            n if n == rects.len() => rects[n - 1].max.y + gap,
            n => (rects[n - 1].max.y + rects[n].min.y) * 0.5,
        };
        let c = self.ctx.theme.accent;
        let line = Rect { min: vec2(area.min.x, y - 1.5), max: vec2(area.max.x, y + 1.5) };
        self.painter().fill(line, Corner::Capsule, c);
        if self.ctx.take_drop::<Reorder>(area.expand2(vec2(48.0, 28.0))).is_some() {
            let item = items.remove(d.from);
            let to = if to > d.from { to - 1 } else { to };
            items.insert(to.min(items.len()), item);
            return true;
        }
        false
    }
}
