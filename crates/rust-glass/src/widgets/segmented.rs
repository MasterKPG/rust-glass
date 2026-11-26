//! Segmented control.

use crate::accessibility::{Action, Role};
use crate::animation::Tuning;
use crate::color::Color;
use crate::context::{Cursor, Response, Sense};
use crate::geometry::{Rect, lerp, vec2};
use crate::input::Key;
use crate::material::Corner;
use crate::paint::Feather;
use crate::text::{TextStyle, Weight};
use crate::ui::{Ui, Widget};
use crate::widgets::thumb::{drag_target, load_thumb, store_thumb, thumb_glass, thumb_rect};

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
    /// Creates a control that sets `selected` to the index of the chosen label.
    pub fn new(selected: &'a mut usize, labels: &'a [&'a str]) -> Self {
        Self { selected, labels, width: None, scale: 1.0, knob: None, track: None, fit: false }
    }
    /// Sets the width. The default fits the labels.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
    /// Scales the whole control.
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }
    /// Sets the color of the thumb at rest.
    pub fn thumb_color(mut self, color: Color) -> Self {
        self.knob = Some(color);
        self
    }
    /// Sets the color of the track.
    pub fn track_color(mut self, color: Color) -> Self {
        self.track = Some(color);
        self
    }
    /// Segments as wide as their label instead of equal.
    pub fn fit(mut self) -> Self {
        self.fit = true;
        self
    }
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
        let mut r = ui.allocate_response(vec2(w, h), Sense::Drag);
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
        let mut d = load_thumb(ui.ctx, r.id);
        let ox = rect.min.x;
        let first = cells[0].center().x - ox;
        let last = cells[n - 1].center().x - ox;
        let dragged = drag_target(ui.ctx, r.id, &r, ox, d.pos.value, first, last, 8.0 * k);
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
            let p = r.pointer.map_or(d.pos.value + ox, |p| p.x);
            let moved = (p - ui.ctx.input.press_origin.x).abs() > 4.0;
            *self.selected = if moved {
                nearest(d.pos.value)
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
        for (i, cell) in cells.iter().enumerate() {
            let sid = r.id.with(("segment", i));
            if ui.ctx.access_requested(sid, &Action::Click) {
                *self.selected = i;
            }
            let sel = *self.selected == i;
            if let Some(node) = ui.access(sid, Role::Tab, *cell) {
                node.label(self.labels.get(i).copied().unwrap_or_default()).selected(sel);
            }
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
        if r.changed && d.lift.value < 0.3 {
            // A tap: hop into the lens for the trip.
            d.lift.value = 0.31;
        }
        let f = d.step(ui.ctx, target, follow, r.held, rest, lifted);
        store_thumb(ui.ctx, r.id, d);

        let thumb = thumb_rect(ui.ctx, vec2(ox + d.pos.value, rect.center().y), &f);
        let lift = f.lift.clamp(0.0, 1.0);
        let knob =
            self.knob.unwrap_or(if ui.surface.darkness() > 0.5 { Color::gray(0.42) } else { Color::WHITE });
        let track_c = self.track.unwrap_or(ui.surface.fill_color());
        let text_c = Color::BLACK.lerp(Color::WHITE, ui.surface.darkness());
        let quiet_c = ui.surface.secondary_color().fade(0.78);
        // The face is painted as a separate platter below the labels, so the
        // labels stay sharp on top of it and both bend in the lens.
        let mut glass = thumb_glass(ui.ctx, thumb, &f, knob, k);
        glass.material.tint = None;
        glass.material.blur = 0.0;
        glass.material.refraction *= 0.72;
        glass.material.depth *= 0.55;
        glass.material.dispersion *= 0.06;
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
        let clear = lerp(0.68, 0.92, ui.ctx.theme.tint_above_default());

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

impl Ui<'_> {
    /// Adds a segmented control that sets `selected` to the index of the
    /// chosen label.
    pub fn segmented(&mut self, selected: &mut usize, labels: &[&str]) -> Response {
        self.add(Segmented::new(selected, labels))
    }
}
