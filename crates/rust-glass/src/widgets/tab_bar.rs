//! Floating tab bar.

use crate::accessibility::{Action, Role};
use crate::animation::Tuning;
use crate::color::Color;
use crate::context::{Context, Cursor, Id, Order, Sense};
use crate::geometry::{Rect, Vec2, lerp, vec2};
use crate::icon::Icon;
use crate::input::Key;
use crate::material::{Corner, Glass};
use crate::paint::Feather;
use crate::text::{TextStyle, Weight};
use crate::widgets::thumb::{ThumbFlex, drag_target, load_thumb, store_thumb, thumb_rect};

/// A floating glass tab bar. While the selection is dragged it lifts into a
/// clear blob that stands out from the bar.
pub struct TabBar<'a> {
    selected: &'a mut usize,
    tabs: &'a [(Icon, &'a str)],
}

impl<'a> TabBar<'a> {
    /// Creates a tab bar with an icon and a label per tab.
    pub fn new(selected: &'a mut usize, tabs: &'a [(Icon, &'a str)]) -> Self {
        Self { selected, tabs }
    }

    /// Shows the bar centered on `center`, above the content. Returns `true`
    /// when the selection changed.
    pub fn show_at(self, ctx: &mut Context, id: Id, center: Vec2) -> bool {
        let n = self.tabs.len().max(1);
        // Neighbouring selections overlap a little, as the pill is wider than
        // the distance between tabs.
        let pitch = 77.0;
        let pill = vec2(86.0, 54.0);
        let pad = 4.0;
        let blob = vec2(112.0, 77.0);
        let size = vec2(pitch * (n - 1) as f32, 0.0) + pill + Vec2::splat(pad * 2.0);
        let bar = Rect::from_center_size(center, size).snap(ctx.pixels_per_point);
        let center = bar.center();
        let layer = ctx.layer(id, Order::Bar);
        let r = ctx.interact(id, bar, Rect::EVERYTHING, layer, Sense::Drag);
        ctx.focusable(id);
        if r.hovered {
            ctx.cursor = Cursor::Pointer;
        }
        let ox = bar.min.x;
        let at = |i: usize| pad + pill.x * 0.5 + pitch * i as f32;
        let before = *self.selected;
        *self.selected = before.min(n - 1);
        let mut d = load_thumb(ctx, id);
        let dragged = drag_target(ctx, id, &r, ox, d.pos.value, at(0), at(n - 1), 10.0);
        let nearest = |x: f32| (((x - at(0)) / pitch).round().max(0.0) as usize).min(n - 1);
        if r.released {
            let p = r.pointer.map_or(d.pos.value, |p| p.x - ox);
            let moved = (p + ox - ctx.input.press_origin.x).abs() > 4.0;
            *self.selected = if moved { nearest(d.pos.value) } else { nearest(p) };
        }
        if r.focused {
            if ctx.input.take_key(Key::Right) {
                *self.selected = (*self.selected + 1).min(n - 1);
            }
            if ctx.input.take_key(Key::Left) {
                *self.selected = self.selected.saturating_sub(1);
            }
        }
        for (i, tab) in self.tabs.iter().enumerate() {
            let tid = id.with(("tab", i));
            if ctx.access_requested(tid, &Action::Click) {
                *self.selected = i;
            }
            let cell = Rect::from_center_size(vec2(ox + at(i), center.y), pill);
            if let Some(node) = ctx.access_node(tid, Role::Tab, cell, layer) {
                node.label(tab.1).selected(*self.selected == i);
            }
        }
        let changed = before != *self.selected;
        if changed && d.lift.value < 0.3 {
            d.lift.value = 0.31;
        }
        let target = dragged.unwrap_or(at(*self.selected));
        let follow =
            if dragged.is_some() && !r.pressed { Tuning::new(0.12, 0.94) } else { Tuning::new(0.30, 0.90) };
        let flex = ThumbFlex::BLOB.liquid(ctx.theme.blob.liquidity);
        let f = d.step_flex(ctx, target, follow, r.held, pill, blob, flex);
        store_thumb(ctx, id, d);
        let lift = f.lift.clamp(0.0, 1.0);

        // The whole bar swells under the finger, contents included.
        let press = ctx.animate_bool(id.with("press"), r.held, Tuning::new(0.30, 0.74)).max(0.0);
        let (sx, sy) =
            if ctx.theme.reduce_motion { (1.0, 1.0) } else { (1.0 + 0.040 * press, 1.0 + 0.030 * press) };
        let body = if press > 0.0 { bar.scale_from_center(sx, sy) } else { bar };
        let to_x = |x: f32| center.x + (x - size.x * 0.5) * sx;
        let drop = thumb_rect(ctx, vec2(to_x(d.pos.value), center.y), &f);
        // Squeezed as a whole, so a tall blob keeps its flat top and bottom.
        let round = Corner::Stretched { base: f.base, power: lerp(2.18, 2.3, lift) };

        // Light under the finger, which stays where it was when let go and
        // fades there.
        let glow = ctx.animate(
            id.with("glow"),
            r.held as u32 as f32,
            if r.held { Tuning::new(0.16, 1.0) } else { Tuning::new(0.5, 1.0) },
        );
        let mut glow_at: Vec2 = ctx.get(id.with("glow at"));
        if r.held
            && let Some(p) = r.pointer
        {
            glow_at = vec2(p.x.clamp(body.min.x, body.max.x), p.y.clamp(body.min.y, body.max.y)) - center;
            ctx.set(id.with("glow at"), glow_at);
        }
        let glow_at = center + glow_at;

        let mut g = Glass::new(body, Corner::Capsule, ctx.theme.regular);
        let a = ctx.prepare_glass(id, &mut g, true);
        g = g.glow(glow_at, glow * lerp(0.5, 0.2, a.darkness));
        g.brightness = 0.03 * press;
        let s = a.surface();
        let accent = ctx.theme.accent;
        let accent = if a.darkness > 0.5 { accent.lerp(Color::WHITE, 0.15) } else { accent };
        // At the clearest setting the blob is pure glass. Towards tinted it gets
        // a milky body under the labels and bends less.
        let veil = ctx.theme.tab_blob_veil();
        let mut m = ctx.theme.blob;
        m.refraction *= 1.0 - 0.55 * veil;
        m.pinch *= 1.0 - 0.6 * veil;
        m.dispersion *= 1.0 - 0.7 * veil;
        m.inner_glow += 0.10 * veil;
        // The lens stays clear on dark glass, where light would turn it milky.
        let mut lens =
            Glass::new(drop, round, m).presence(lift).glow(glow_at, glow * lerp(0.45, 0.12, a.darkness));
        lens.light_bend = f.bend;
        // The labels bend on their own, and only over the bar: the caps that
        // stand proud of it belong to the lens.
        let mut m = ctx.theme.blob_labels;
        m.refraction *= 1.0 - 0.55 * veil;
        m.dispersion *= 1.0 - 0.7 * veil;
        let mut warp = Glass::new(drop, round, m).presence(lift);
        warp.light_bend = f.bend;
        let inside = body.expand(-3.5);
        let milk = Color::WHITE.lerp(Color::gray(0.30), a.darkness).alpha(0.85 * veil * lift);
        let small = TextStyle::new(10.5).weight(Weight::Semibold);
        let labels: Vec<_> = self.tabs.iter().map(|t| ctx.text.layout(t.1, &small)).collect();

        let mut p = ctx.painter(layer, Rect::EVERYTHING);
        p.glass(&g);
        // The selection at rest is a flat shade of the bar, as on iOS 27:
        // slightly darker on light glass, much darker on dark glass.
        let pill = Color::black(lerp(0.09, 0.50, a.darkness));
        p.fill(drop, round, pill.fade((1.0 - lift * 1.5).max(0.0)));
        p.fill(drop, round, milk);
        let draw = |p: &mut crate::paint::Painter, c: Color, k: f32| {
            for (i, label) in labels.iter().enumerate() {
                let m = vec2(to_x(at(i)), center.y);
                p.icon(m + vec2(0.0, -7.5) * k, 23.0 * k, self.tabs[i].0, c);
                let lr = Rect::from_center_size(m + vec2(0.0, 14.5) * k, label.size);
                p.text_centered(lr, label, c, k);
            }
        };
        let out = p.add_mask(drop, round, true, Feather::default());
        p.with_mask(out, |p| draw(p, s.text_color(), 1.0));
        p.glass(&lens);
        let inm = p.add_mask(drop, round, false, Feather::default());
        p.with_mask(inm, |p| draw(p, accent, 1.0 + 0.18 * lift));
        let over = p.add_mask(
            inside,
            Corner::Capsule,
            false,
            Feather { left: 6.0, top: 6.0, right: 6.0, bottom: 6.0 },
        );
        p.with_mask(over, |p| p.glass(&warp));
        if ctx.shows_focus(id) {
            let mut p = ctx.painter(layer, Rect::EVERYTHING);
            p.stroke(body.expand(3.5), Corner::Capsule, 2.5, accent);
        }
        changed
    }
}
