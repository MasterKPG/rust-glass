//! Drag and drop inside a window: pick up content, carry it on a lifted
//! glass plate and drop it on a target that accepts its payload type.

use std::any::Any;
use std::hash::Hash;

use crate::animation::{Spring, SpringRect, Tuning};
use crate::context::{Cursor, Id, Order, Response, Sense};
use crate::geometry::{Rect, Vec2, smoothstep, vec2};
use crate::material::{Corner, Glass};
use crate::ui::Ui;

/// Movement in points before a press turns into a drag.
const SLOP: f32 = 6.0;

/// The lift of a drag source: how far it has risen off the page, and where
/// its lifted copy is.
#[derive(Clone, Copy, Default)]
struct Grab {
    lift: Spring,
    shape: SpringRect,
    /// Smoothed speed of the copy while it follows the pointer, in points
    /// per second. A drop carries it on, so the copy flies on from where it
    /// was let go.
    velocity: Vec2,
    /// The copy follows the pointer.
    carried: bool,
    /// The copy is on its way to its place after a drop or a cancel. The
    /// item in the list stays hidden until it is there.
    landing: bool,
    /// The copy is over its place and fading into it.
    arrived: bool,
}

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

    /// [`Ui::drag_source`] with an id used as it is, not combined with the
    /// id of this `Ui`. Give an item the same id in every list it can be
    /// dropped in, and it lands where it was let go when it moves from one
    /// list to another.
    pub fn drag_source_id<P: Any + Clone>(&mut self, id: Id, payload: P, f: impl FnMut(&mut Ui)) -> Response {
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
        let lift_id = id.with("lift");
        let mut grab: Grab = self.ctx.get(lift_id);
        let old = self.opacity;
        let lift = grab.lift.value.clamp(0.0, 1.0);
        // While the copy lands, it alone shows the item. It hands over to the
        // slot in one frame once it has set down exactly on it, so the item
        // is never seen twice.
        let landing = grab.landing;
        self.opacity *= if landing {
            0.0
        } else {
            // The slot stays as a faint outline of what was picked up.
            1.0 - 0.72 * lift
        };
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
        let dragging = self.ctx.dragged_id() == Some(id);

        // The item lifts as soon as it is pressed, follows the pointer into
        // the drag without a jump, and settles back into its slot when let
        // go. Where the press started, the lifted copy is exactly on the
        // slot, so the drag picks it up where it already is.
        let held = dragging || r.held;
        let place = match (dragging, self.ctx.drag_rect(), r.pointer) {
            (true, Some(at), _) => Some(at),
            (false, _, Some(p)) if held => Some(rect.translate(p - self.ctx.input.press_origin)),
            _ => None,
        };
        let dt = self.ctx.dt;
        let mut moving = false;
        match place {
            Some(at) => {
                if grab.carried && dt > 0.0 {
                    let v = (at.center() - grab.shape.rect().center()) / dt;
                    grab.velocity = grab.velocity.lerp(v, 0.35);
                } else {
                    grab.velocity = Vec2::ZERO;
                }
                grab.shape = SpringRect::new(at);
                grab = Grab { carried: true, landing: false, arrived: false, ..grab };
            }
            None => {
                if grab.carried {
                    // Let go: the copy keeps the speed it had and curves into
                    // its place from there.
                    let v = grab.velocity.length();
                    let v = if v > 2600.0 { grab.velocity * (2600.0 / v) } else { grab.velocity };
                    grab.shape.center.x.velocity = v.x;
                    grab.shape.center.y.velocity = v.y;
                    grab = Grab { carried: false, landing: true, arrived: false, ..grab };
                }
                let fly = self.ctx.tuning(Tuning::new(0.36, 0.84));
                let size = self.ctx.tuning(Tuning::new(0.3, 0.9));
                moving |= grab.shape.step(rect, fly, size, dt);
            }
        }
        // It stays lifted on the way and sets down once it is over its place.
        let away = (grab.shape.rect().center() - rect.center()).length();
        grab.arrived = grab.landing && away < 1.0;
        let up = held || (grab.landing && !grab.arrived);
        let tuning = self.ctx.tuning(if up { Tuning::new(0.22, 0.74) } else { Tuning::new(0.24, 0.8) });
        moving |= grab.lift.step(up as u32 as f32, tuning, dt);
        if grab.landing && !up && grab.lift.value.abs() < 0.004 && grab.lift.velocity.abs() < 0.05 {
            grab.landing = false;
            grab.arrived = false;
            grab.lift = Spring::new(0.0);
            self.ctx.request_repaint();
        }
        if moving {
            self.ctx.request_repaint();
        }
        let lift = grab.lift.value;
        if held || landing || grab.landing || lift > 0.01 {
            let at = grab.shape.rect();
            // Set down on whole pixels, as the slot is.
            let at = if grab.arrived && lift < 0.3 {
                rect
            } else {
                Rect::from_center_size(at.center(), rect.size())
            };
            let layer = self.ctx.layer(Id::new("drag and drop"), Order::Tooltip);
            let mut ui = Ui::new(self.ctx, layer, id.with("lifted"), at);
            ui.spacing = self.spacing;
            // A plate of glass carries the content and swells as it lifts.
            let up = lift.clamp(0.0, 1.2);
            let plate = at.expand(10.0 * up);
            let mut m = ui.ctx.theme.regular;
            m.shadow *= 1.0 + 0.6 * lift.clamp(0.0, 1.0);
            let mut g = Glass::new(plate, Corner::Radius(12.0 + 6.0 * lift.clamp(0.0, 1.0)), m)
                .presence(lift.clamp(0.0, 1.0));
            let plate_surface = ui.ctx.prepare_glass(id.with("plate"), &mut g, true).surface();
            // Low on the page the content takes the colors it has in its slot,
            // so handing over to the slot shows no change.
            ui.surface = if lift > 0.35 { plate_surface } else { self.surface };
            ui.painter().glass(&g);
            ui.opacity = if landing { 1.0 } else { smoothstep(0.0, 0.5, lift) };
            f(&mut ui);
            r.changed = dragging;
        }
        self.ctx.set(lift_id, grab);
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
