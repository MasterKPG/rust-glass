//! Drag and drop inside a window: pick up content, carry it on a lifted
//! glass plate and drop it on a target that accepts its payload type.

use std::any::Any;
use std::hash::Hash;

use crate::animation::{Spring, Tuning};
use crate::context::{Cursor, Id, Order, Response, Sense};
use crate::geometry::{Rect, Vec2, vec2};
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
        f: impl FnMut(&mut Ui),
    ) -> Response {
        let id = self.make_id(id);
        self.drag_source_with_id(id, payload, f)
    }

    /// [`Ui::drag_source`] with an id that is already final.
    fn drag_source_with_id<P: Any + Clone>(
        &mut self,
        id: Id,
        payload: P,
        mut f: impl FnMut(&mut Ui),
    ) -> Response {
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

    /// A vertical list whose items can be dragged into a new order. While an
    /// item is dragged, the others move apart to show where it will land.
    /// `T` is hashed for the id of each item, so equal items need something
    /// that tells them apart. Returns `true` in the frame the order changed.
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
        let ids: Vec<Id> = items.iter().map(|item| self.make_id((list, item))).collect();
        let dragged =
            self.ctx.dragged::<Reorder>().filter(|d| d.list == list && d.from < items.len()).map(|d| d.from);
        let mut state: ReorderState = self.ctx.get(list);
        let gap_id = |slot: usize| list.with(("gap", slot));

        let Some(from) = dragged else {
            if state.active {
                // The drag ended without a drop: the item is back in its place,
                // which its gap was holding.
                for slot in 0..=items.len() {
                    self.ctx.set(gap_id(slot), Spring::new(0.0));
                }
                self.ctx.set(list, ReorderState::default());
            }
            for (i, item) in items.iter().enumerate() {
                self.drag_source_with_id(ids[i], Reorder { list, from: i }, |ui| f(ui, item));
            }
            return false;
        };

        if !state.active {
            // The drag just started. Open a gap where the item was, at once,
            // so nothing moves yet.
            let height = self.ctx.drag_rect().map_or(0.0, |r| r.height()) + self.spacing;
            state = ReorderState { active: true, slot: from, height };
            self.ctx.set(gap_id(from), Spring::new(height));
        }

        // Lay out the other items with an animated gap before each one, and
        // remember where they were before the drag started.
        let mut centers = Vec::with_capacity(items.len());
        let mut area = Rect::NOTHING;
        let mut gaps = 0.0;
        let mut slot = 0;
        for (i, item) in items.iter().enumerate() {
            if i == from {
                // The dragged item takes no space. It is still drawn, hidden,
                // because its drag source draws the lifted copy.
                let rest = Rect::from_min_size(self.next_pos(), vec2(self.available_width(), 1.0e5));
                self.in_rect(("dragged", i), rest, |ui| {
                    ui.opacity = 0.0;
                    ui.drag_source_with_id(ids[i], Reorder { list, from: i }, |ui| f(ui, item));
                });
                continue;
            }
            gaps += self.reorder_gap(gap_id(slot), state.slot == slot, state.height);
            let rect = self.drag_source_with_id(ids[i], Reorder { list, from: i }, |ui| f(ui, item)).rect;
            let below_dragged = if i > from { state.height } else { 0.0 };
            centers.push(rect.center().y - gaps + below_dragged);
            area = area.union(rect);
            slot += 1;
        }
        self.reorder_gap(gap_id(slot), state.slot == slot, state.height);

        // The item lands before the first item whose center is below its own.
        // Outside the list it goes back to where it came from.
        let reach = area.expand2(vec2(48.0, 28.0 + state.height));
        let target = match (self.ctx.drag_rect(), self.ctx.input.pointer) {
            (Some(lifted), Some(pointer)) if reach.contains(pointer) => {
                centers.iter().filter(|&&c| c < lifted.center().y).count()
            }
            _ => from,
        };
        state.slot = target;
        self.ctx.set(list, state);

        if self.ctx.take_drop::<Reorder>(reach).is_some() {
            let item = items.remove(from);
            items.insert(target.min(items.len()), item);
            // The item now fills the gap that was open for it.
            for slot in 0..=items.len() {
                self.ctx.set(gap_id(slot), Spring::new(0.0));
            }
            self.ctx.set(list, ReorderState::default());
            return true;
        }
        false
    }

    /// Adds the animated space before one slot of a reordered list and
    /// returns its height.
    fn reorder_gap(&mut self, id: Id, open: bool, height: f32) -> f32 {
        let gap = self.ctx.animate(id, if open { height } else { 0.0 }, Tuning::SMOOTH).max(0.0);
        if gap > 0.25 {
            self.space(gap);
        }
        gap
    }
}

/// Where a reordered list makes room for the dragged item.
#[derive(Clone, Copy, Default)]
struct ReorderState {
    /// A drag over this list is running.
    active: bool,
    /// The slot among the other items that has the gap.
    slot: usize,
    /// Height of the gap: the dragged item and one spacing.
    height: f32,
}
