//! Segmented control, tab bar and switch. All three share the droplet thumb.

use crate::anim::Tuning;
use crate::color::Color;
use crate::context::{Context, Cursor, Id, Order, Response, Sense};
use crate::geom::{Rect, Vec2, lerp, vec2};
use crate::icons::Icon;
use crate::input::Key;
use crate::material::{Corner, Glass};
use crate::paint::Feather;
use crate::text::{TextStyle, Weight};
use crate::ui::{Ui, Widget};
use crate::widgets::gel::{droplet_glass, droplet_rect, droplet_state, rubber, set_droplet};

/// Pick one of a few options. The selected label is drawn inside the thumb and
/// seen through it, so it bends and splits at the rim while the thumb moves.
pub struct Segmented<'a> {
    selected: &'a mut usize,
    labels: &'a [&'a str],
    width: Option<f32>,
    scale: f32,
    knob: Option<Color>,
    track: Option<Color>,
    fit: bool,
}

impl<'a> Segmented<'a> {
    pub fn new(selected: &'a mut usize, labels: &'a [&'a str]) -> Self {
        Self { selected, labels, width: None, scale: 1.0, knob: None, track: None, fit: false }
    }
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    pub fn scale(mut self, s: f32) -> Self {
        self.scale = s;
        self
    }
    pub fn knob(mut self, c: Color) -> Self {
        self.knob = Some(c);
        self
    }
    pub fn track(mut self, c: Color) -> Self {
        self.track = Some(c);
        self
    }
    /// Segments as wide as their label instead of equal.
    pub fn fit(mut self) -> Self {
        self.fit = true;
        self
    }
}

/// Where the thumb is being dragged, shared by the three controls. Positions
/// are relative to `ox`.
#[allow(clippy::too_many_arguments)]
fn drag_target(
    ctx: &mut Context,
    id: Id,
    r: &Response,
    ox: f32,
    pos: f32,
    lo: f32,
    hi: f32,
    limit: f32,
) -> Option<f32> {
    #[derive(Clone, Copy, Default)]
    struct Grab {
        off: f32,
        on: bool,
    }
    let mut g: Grab = ctx.get(id);
    let mut out = None;
    if r.held
        && let Some(p) = r.pointer
    {
        if r.pressed {
            g = Grab { off: 0.0, on: true };
            if (p.x - ox - pos).abs() < limit * 2.5 {
                g.off = p.x - ox - pos;
            }
        }
        if g.on {
            let x = p.x - ox - g.off;
            out = Some(x.clamp(lo, hi) + rubber(x - x.clamp(lo, hi), limit));
        }
    } else {
        g.on = false;
    }
    ctx.set(id, g);
    out
}

impl Widget for Segmented<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let k = self.scale;
        let h = 34.0 * k;
        let font = 12.8 * k;
        let n = self.labels.len().max(1);
        let quiet = TextStyle::new(font).weight(Weight::Medium);
        let loud = TextStyle::new(font).weight(Weight::Semibold);
        let texts: Vec<_> =
            self.labels.iter().map(|l| (ui.layout_text(l, &quiet), ui.layout_text(l, &loud))).collect();
        let natural: Vec<f32> = texts.iter().map(|t| t.1.size.x + 36.0 * k).collect();
        let total: f32 = if self.fit {
            natural.iter().sum()
        } else {
            natural.iter().cloned().fold(0.0, f32::max) * n as f32
        };
        let w = self.width.unwrap_or(total).min(ui.available_width().max(60.0));
        let mut widths: Vec<f32> =
            if self.fit { natural.iter().map(|x| x * w / total).collect() } else { vec![w / n as f32; n] };
        if widths.is_empty() {
            widths.push(w);
        }
        let mut r = ui.alloc_interact(vec2(w, h), Sense::Drag);
        ui.ctx.focusable(r.id);
        let rect = r.rect;
        let mut x = rect.min.x;
        let cells: Vec<Rect> = widths
            .iter()
            .map(|cw| {
                let c = Rect::from_min_size(vec2(x, rect.min.y), vec2(*cw, h));
                x += cw;
                c
            })
            .collect();

        let before = *self.selected;
        *self.selected = before.min(n - 1);
        let mut d = droplet_state(ui.ctx, r.id);
        let ox = rect.min.x;
        let first = cells[0].center().x - ox;
        let last = cells[n - 1].center().x - ox;
        let dragged = drag_target(ui.ctx, r.id, &r, ox, d.pos.x, first, last, 8.0 * k);
        let nearest = |x: f32| {
            (0..n)
                .min_by(|a, b| {
                    let da = (cells[*a].center().x - ox - x).abs();
                    let db = (cells[*b].center().x - ox - x).abs();
                    da.total_cmp(&db)
                })
                .unwrap_or(0)
        };
        if r.released {
            let p = r.pointer.map_or(d.pos.x + ox, |p| p.x);
            let moved = (p - ui.ctx.input.press_origin.x).abs() > 4.0;
            *self.selected = if moved {
                nearest(d.pos.x)
            } else {
                cells.iter().position(|c| p >= c.min.x && p < c.max.x).unwrap_or(nearest(p - ox))
            };
        }
        if r.focused {
            if ui.ctx.input.take_key(Key::Right) {
                *self.selected = (*self.selected + 1).min(n - 1);
            }
            if ui.ctx.input.take_key(Key::Left) {
                *self.selected = self.selected.saturating_sub(1);
            }
        }
        if r.hovered {
            ui.ctx.cursor = Cursor::Pointer;
        }
        r.changed = before != *self.selected;

        let sel = *self.selected;
        let target = dragged.unwrap_or(cells[sel].center().x - ox);
        // Interpolate the resting width while travelling between cells.
        let hover_cell = nearest(target);
        let cell_w = ui.ctx.animate(r.id.with("w"), cells[hover_cell].width(), Tuning::SMOOTH);
        let rest = vec2(cell_w, h - 1.0 * k);
        let lifted = vec2(cell_w * 1.07 + 4.0 * k, h * 1.34);
        let follow =
            if dragged.is_some() && !r.pressed { Tuning::new(0.13, 0.9) } else { Tuning::new(0.36, 0.80) };
        if r.changed && d.lift.x < 0.3 {
            // A tap: hop into the lens for the trip.
            d.lift.x = 0.31;
        }
        let f = d.step(ui.ctx, target, follow, r.held, rest, lifted);
        set_droplet(ui.ctx, r.id, d);

        let thumb = droplet_rect(ui.ctx, vec2(ox + d.pos.x, rect.center().y), &f);
        let lift = f.lift.clamp(0.0, 1.0);
        let knob =
            self.knob.unwrap_or(if ui.surface.dark() > 0.5 { Color::gray(0.42) } else { Color::WHITE });
        let track_c = self.track.unwrap_or(ui.surface.fill());
        let text_c = Color::BLACK.lerp(Color::WHITE, ui.surface.dark());
        let quiet_c = ui.surface.secondary().fade(0.78);
        // The face is painted as a separate platter below the labels, so the
        // labels stay sharp on top of it and both bend in the lens.
        let mut glass = droplet_glass(ui.ctx, thumb, &f, knob, k);
        glass.material.tint = None;
        glass.material.blur = 0.0;
        glass.material.refraction *= 0.72;
        glass.material.depth *= 0.55;
        glass.material.chroma *= 0.06;
        glass.material.dark_edge *= 0.10;
        glass.material.inner_glow *= 1.9;
        glass.material.shadow *= 0.48;
        glass.material.shadow_radius *= 1.0 + 0.7 * lift;
        glass.material.shadow_y *= 1.0 + 0.9 * lift;
        glass.material.shadow_tint *= 0.3;
        glass.material.saturation = 1.0;
        glass.material.light_range.0 = 0.0;
        glass.material.dark_range.0 = 0.0;
        let grow = 1.0 + 0.15 * lift;
        // How much of the platter stays under the lifted lens.
        let clear = lerp(0.68, 0.92, ui.ctx.theme.tinted());

        let mut p = ui.painter();
        p.fill(rect, Corner::Capsule, track_c);
        let outside = p.add_mask(thumb, Corner::Capsule, true, Feather::default());
        p.with_mask(outside, |p| {
            for (i, c) in cells.iter().enumerate() {
                p.text_centered(*c, &texts[i].0, quiet_c, 1.0);
            }
        });
        p.fill(thumb, Corner::Capsule, knob.fade(lerp(1.0, clear, lift)));
        let inside = p.add_mask(thumb, Corner::Capsule, false, Feather::default());
        p.with_mask(inside, |p| {
            for (i, c) in cells.iter().enumerate() {
                p.text_centered(*c, &texts[i].1, text_c, grow);
            }
        });
        p.glass(&glass);
        ui.focus_ring(r.id, rect, Corner::Capsule);
        r
    }
}

/// On and off.
pub struct Toggle<'a> {
    on: &'a mut bool,
    label: &'a str,
    tint: Color,
    scale: f32,
}

impl<'a> Toggle<'a> {
    pub fn new(on: &'a mut bool) -> Self {
        Self { on, label: "", tint: Color::GREEN, scale: 1.0 }
    }
    pub fn label(mut self, l: &'a str) -> Self {
        self.label = l;
        self
    }
    pub fn tint(mut self, c: Color) -> Self {
        self.tint = c;
        self
    }
    pub fn scale(mut self, s: f32) -> Self {
        self.scale = s;
        self
    }
}

impl Widget for Toggle<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let k = self.scale;
        let size = vec2(60.0, 27.0) * k;
        let rest = vec2(36.0, 23.0) * k;
        let lifted = vec2(52.0, 35.0) * k;
        let st = TextStyle::new(ui.ctx.theme.body);
        let text = (!self.label.is_empty()).then(|| ui.layout_text(self.label, &st));
        let fill_row = text.is_some() && !ui.is_horizontal();
        let tw = text.as_ref().map_or(0.0, |t| t.size.x + 12.0);
        let total = if fill_row { ui.available_width() } else { size.x + tw };
        let rect = ui.alloc(vec2(total, size.y + 4.0));
        let id = ui.next_id();
        let track = Rect::from_min_size(vec2(rect.max.x - size.x, rect.center().y - size.y * 0.5), size)
            .snap(ui.ctx.ppp);
        let mut r = ui.interact(id, rect, Sense::Drag);
        ui.ctx.focusable(id);
        if r.hovered {
            ui.ctx.cursor = Cursor::Pointer;
        }
        let pad = 2.0 * k;
        let ox = track.min.x;
        let x0 = pad + rest.x * 0.5;
        let x1 = track.width() - pad - rest.x * 0.5;
        let mut d = droplet_state(ui.ctx, id);
        let before = *self.on;
        let dragged = drag_target(ui.ctx, id, &r, ox, d.pos.x, x0, x1, 5.0 * k);
        let mid = (x0 + x1) * 0.5;
        if r.released {
            let moved = r.pointer.is_some_and(|p| (p.x - ui.ctx.input.press_origin.x).abs() > 4.0);
            *self.on = if moved { d.pos.x > mid } else { !before };
        }
        if r.focused && (ui.ctx.input.take_key(Key::Space) || ui.ctx.input.take_key(Key::Enter)) {
            *self.on = !*self.on;
        }
        r.changed = before != *self.on;
        let home = if *self.on { x1 } else { x0 };
        // Without a drag a press only lifts the thumb where it is.
        let moved = r.pointer.is_some_and(|p| (p.x - ui.ctx.input.press_origin.x).abs() > 3.0);
        let target = dragged.filter(|_| moved).unwrap_or(home);
        if r.changed && d.lift.x < 0.3 {
            d.lift.x = 0.31;
        }
        let follow =
            if dragged.is_some() && moved { Tuning::new(0.14, 0.9) } else { Tuning::new(0.32, 0.80) };
        let f = d.step(ui.ctx, target, follow, r.held, rest, lifted);
        set_droplet(ui.ctx, id, d);

        let t = ((d.pos.x - x0) / (x1 - x0)).clamp(0.0, 1.0);
        let off = ui.surface.fill_strong();
        let knob = ui.surface.knob();
        let thumb = droplet_rect(ui.ctx, vec2(ox + d.pos.x, track.center().y), &f);
        let glass = droplet_glass(ui.ctx, thumb, &f, knob, k);
        let tc = ui.surface.text();
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

/// Floating tab bar. Returns true when the selection changed.
pub struct TabBar<'a> {
    pub selected: &'a mut usize,
    pub tabs: &'a [(Icon, &'a str)],
}

impl TabBar<'_> {
    /// Place the bar centered on `center`.
    pub fn show_at(self, ctx: &mut Context, id: Id, center: Vec2) -> bool {
        let n = self.tabs.len().max(1);
        // Neighbouring selections overlap a little, as the pill is wider than
        // the distance between tabs.
        let pitch = 77.0;
        let pill = vec2(86.0, 54.0);
        let pad = 4.0;
        let blob = vec2(98.0, 80.0);
        let size = vec2(pitch * (n - 1) as f32, 0.0) + pill + Vec2::splat(pad * 2.0);
        let bar = Rect::from_center_size(center, size).snap(ctx.ppp);
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
        let mut d = droplet_state(ctx, id);
        let dragged = drag_target(ctx, id, &r, ox, d.pos.x, at(0), at(n - 1), 14.0);
        let nearest = |x: f32| (((x - at(0)) / pitch).round().max(0.0) as usize).min(n - 1);
        if r.released {
            let p = r.pointer.map_or(d.pos.x, |p| p.x - ox);
            let moved = (p + ox - ctx.input.press_origin.x).abs() > 4.0;
            *self.selected = if moved { nearest(d.pos.x) } else { nearest(p) };
        }
        if r.focused {
            if ctx.input.take_key(Key::Right) {
                *self.selected = (*self.selected + 1).min(n - 1);
            }
            if ctx.input.take_key(Key::Left) {
                *self.selected = self.selected.saturating_sub(1);
            }
        }
        let changed = before != *self.selected;
        if changed && d.lift.x < 0.3 {
            d.lift.x = 0.31;
        }
        let target = dragged.unwrap_or(at(*self.selected));
        let follow =
            if dragged.is_some() && !r.pressed { Tuning::new(0.14, 0.88) } else { Tuning::new(0.38, 0.80) };
        let f = d.step(ctx, target, follow, r.held, pill, blob);
        set_droplet(ctx, id, d);
        let lift = f.lift.clamp(0.0, 1.0);

        // The whole bar swells under the finger, contents included.
        let press = ctx.animate_bool(id.with("press"), r.held, Tuning::new(0.30, 0.74)).max(0.0);
        let (sx, sy) =
            if ctx.theme.reduce_motion { (1.0, 1.0) } else { (1.0 + 0.040 * press, 1.0 + 0.030 * press) };
        let body = if press > 0.0 { bar.scale_from_center(sx, sy) } else { bar };
        let to_x = |x: f32| center.x + (x - size.x * 0.5) * sx;
        let drop = droplet_rect(ctx, vec2(to_x(d.pos.x), center.y), &f);

        let mut g = Glass::new(body, Corner::Capsule, ctx.theme.regular);
        let a = ctx.prepare_glass(id, &mut g, true);
        g.lit = 0.09 * press;
        let s = a.surface();
        let accent = ctx.theme.accent;
        let accent = if a.dark > 0.5 { accent.lerp(Color::WHITE, 0.15) } else { accent };
        // At the clearest setting the blob is pure glass. Towards tinted it gets
        // a milky body under the labels and bends less.
        let veil = ctx.theme.veil();
        let mut m = ctx.theme.blob;
        m.refraction *= 1.0 - 0.55 * veil;
        m.pinch *= 1.0 - 0.6 * veil;
        m.chroma *= 1.0 - 0.7 * veil;
        m.inner_glow += 0.10 * veil;
        let mut lens = Glass::new(drop, Corner::Capsule, m).presence(lift);
        lens.light_bend = f.bend;
        let milk = Color::WHITE.lerp(Color::gray(0.30), a.dark).alpha(0.85 * veil * lift);
        let small = TextStyle::new(10.5).weight(Weight::Semibold);
        let labels: Vec<_> = self.tabs.iter().map(|t| ctx.text.layout(t.1, &small)).collect();

        let mut p = ctx.painter(layer, Rect::EVERYTHING);
        p.glass(&g);
        p.fill(drop, Corner::Capsule, s.fill_strong().fade((1.0 - lift * 1.5).max(0.0) * 0.8));
        p.fill(drop, Corner::Capsule, milk);
        let draw = |p: &mut crate::paint::Painter, c: Color, k: f32| {
            for (i, label) in labels.iter().enumerate() {
                let m = vec2(to_x(at(i)), center.y);
                p.icon(m + vec2(0.0, -7.5) * k, 23.0 * k, self.tabs[i].0, c);
                let lr = Rect::from_center_size(m + vec2(0.0, 14.5) * k, label.size);
                p.text_centered(lr, label, c, k);
            }
        };
        let out = p.add_mask(drop, Corner::Capsule, true, Feather::default());
        p.with_mask(out, |p| draw(p, s.text(), 1.0));
        let inm = p.add_mask(drop, Corner::Capsule, false, Feather::default());
        p.with_mask(inm, |p| draw(p, accent, 1.0 + 0.18 * lift));
        p.glass(&lens);
        if ctx.shows_focus(id) {
            let mut p = ctx.painter(layer, Rect::EVERYTHING);
            p.stroke(body.expand(3.5), Corner::Capsule, 2.5, accent);
        }
        changed
    }
}

impl Ui<'_> {
    pub fn segmented(&mut self, selected: &mut usize, labels: &[&str]) -> Response {
        self.add(Segmented::new(selected, labels))
    }
    pub fn toggle(&mut self, on: &mut bool, label: &str) -> Response {
        self.add(Toggle::new(on).label(label))
    }
}
