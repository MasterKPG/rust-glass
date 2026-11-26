//! Lists: navigation list, settings rows, sections and tables.

use crate::accessibility::Role;
use crate::animation::Tuning;
use crate::color::Color;
use crate::context::{Cursor, Sense};
use crate::geometry::{Rect, Vec2, vec2};
use crate::icon::Icon;
use crate::material::Corner;
use crate::text::{TextStyle, Weight};
use crate::ui::Ui;

/// Vertical navigation list with a sliding highlight. Returns true on change.
pub fn nav_list(ui: &mut Ui, selected: &mut usize, items: &[(Icon, &str)]) -> bool {
    let before = *selected;
    let w = ui.available_width();
    let h = 38.0;
    let id = ui.make_id("nav");
    let old = ui.spacing;
    ui.spacing = 2.0;
    let top = ui.next_pos().y;
    let y = ui.ctx.animate(id, *selected as f32, Tuning::new(0.34, 0.78));
    let v = ui.ctx.get::<crate::animation::Spring>(id).velocity;
    let accent = ui.ctx.theme.accent.lerp(Color::WHITE, 0.35 * ui.surface.darkness());
    // The highlight stretches along its way.
    let s = (v.abs() * 0.9).min(9.0);
    let hl = Rect::from_min_size(vec2(ui.cursor.x, top + y * (h + 2.0)), vec2(w, h))
        .expand2(vec2(-s * 0.2, s * 0.35));
    let c = ui.surface.strong_fill_color();
    ui.painter().fill(hl, Corner::Capsule, c);
    for (i, (icon, label)) in items.iter().enumerate() {
        let r = ui.allocate_response(vec2(w, h), Sense::Click);
        ui.ctx.focusable(r.id);
        if r.clicked {
            *selected = i;
        }
        if let Some(n) = ui.access(r.id, crate::accessibility::Role::Tab, r.rect) {
            n.label(*label).selected(*selected == i);
        }
        if r.hovered {
            ui.ctx.cursor = Cursor::Pointer;
        }
        let hov = ui.ctx.animate_bool(r.id.with("h"), r.hovered && i != *selected, Tuning::SNAP);
        let on = ui.ctx.animate_bool(r.id.with("on"), i == *selected, Tuning::SNAP);
        let weight = if i == *selected { Weight::Semibold } else { Weight::Medium };
        let st = TextStyle::new(ui.ctx.theme.body_size).weight(weight);
        let l = ui.layout_text(label, &st);
        let (f, t) = (ui.surface.fill_color(), ui.surface.text_color());
        let mut p = ui.painter();
        p.fill(r.rect, Corner::Capsule, f.fade(hov * 0.7));
        p.icon(vec2(r.rect.min.x + 22.0, r.rect.center().y), 18.0, *icon, t.lerp(accent, on));
        p.text(vec2(r.rect.min.x + 42.0, r.rect.center().y - l.size.y * 0.5), &l, t);
        ui.focus_ring(r.id, r.rect, Corner::Capsule);
    }
    ui.spacing = old;
    before != *selected
}

/// Groups the widgets added in `f` on a card that starts with `title`.
///
/// The title is inside the card, so it stays readable over any wallpaper.
pub fn section(ui: &mut Ui, title: &str, f: impl FnOnce(&mut Ui)) {
    ui.card(|ui| {
        let style = TextStyle::new(ui.ctx.theme.body_size + 1.0).weight(Weight::Semibold);
        let color = ui.surface.text_color();
        ui.styled_text(title, style, color);
        f(ui);
    });
}

/// A table with a header row. Columns share the width equally.
pub fn table(ui: &mut Ui, header: &[&str], rows: &[Vec<String>]) {
    let n = header.len().max(1);
    let w = ui.available_width() / n as f32;
    let (tc, sc, sep) = (ui.surface.text_color(), ui.surface.secondary_color(), ui.surface.separator_color());
    let hs = TextStyle::new(12.0).weight(Weight::Semibold);
    let bs = TextStyle::new(ui.ctx.theme.body_size);
    let total = ui.available_width();
    let old = ui.spacing;
    ui.spacing = 0.0;
    let line = |ui: &mut Ui, cells: &[&str], st: &TextStyle, c: Color, h: f32, i: usize| {
        let r = ui.allocate_response(vec2(total, h), Sense::Hover);
        let hov = ui.ctx.animate_bool(r.id, r.hovered && i > 0, Tuning::SNAP);
        let ls: Vec<_> = cells.iter().map(|t| ui.layout_text(t, st)).collect();
        let f = ui.surface.fill_color();
        let mut p = ui.painter();
        p.fill(r.rect.expand2(vec2(8.0, 0.0)), 8.0, f.fade(hov * 0.8));
        for (k, l) in ls.iter().enumerate() {
            p.text(vec2(r.rect.min.x + w * k as f32, r.rect.center().y - l.size.y * 0.5), l, c);
        }
        p.fill(
            Rect { min: vec2(r.rect.min.x, r.rect.max.y - 0.5), max: vec2(r.rect.max.x, r.rect.max.y + 0.5) },
            0.0,
            sep,
        );
    };
    line(ui, header, &hs, sc, 28.0, 0);
    for (i, r) in rows.iter().enumerate() {
        let cells: Vec<&str> = r.iter().map(String::as_str).collect();
        line(ui, &cells, &bs, tc, 34.0, i + 1);
    }
    ui.spacing = old;
}

/// A row of a settings list: an optional colored icon, a title, and controls
/// at the trailing end. Screen readers name the trailing controls after the
/// title.
pub struct SettingsRow<'a> {
    icon: Option<(Icon, Color)>,
    title: &'a str,
}

impl<'a> SettingsRow<'a> {
    /// Creates a row with a title.
    pub fn new(title: &'a str) -> Self {
        Self { icon: None, title }
    }
    /// Shows `icon` in white on a rounded square of `color` before the title.
    pub fn icon(mut self, icon: Icon, color: Color) -> Self {
        self.icon = Some((icon, color));
        self
    }

    /// Shows the row. `trailing` draws the controls at its end.
    pub fn show(self, ui: &mut Ui, trailing: impl FnOnce(&mut Ui)) {
        let w = ui.available_width();
        let r = ui.allocate(vec2(w, 40.0));
        let st = TextStyle::new(ui.ctx.theme.body_size);
        let l = ui.layout_text(self.title, &st);
        let c = ui.surface.text_color();
        let mut x = r.min.x;
        let mut p = ui.painter();
        if let Some((i, bg)) = self.icon {
            let b = Rect::from_center_size(vec2(x + 14.0, r.center().y), Vec2::splat(28.0));
            p.gradient(b, 8.0, bg.lerp(Color::WHITE, 0.14), bg, vec2(0.0, 1.0));
            p.icon(b.center(), 17.0, i, Color::WHITE);
            x += 40.0;
        }
        p.text(vec2(x, r.center().y - l.size.y * 0.5), &l, c);
        let left = x + l.size.x + 16.0;
        let id = ui.next_id();
        if let Some(n) = ui.access(id.with("title"), Role::Label, r) {
            n.label(self.title);
        }
        // Right aligned by remembering last frame's width.
        #[derive(Clone, Copy, Default)]
        struct W(f32);
        let prev: Option<W> = ui.ctx.try_get(id);
        if prev.is_none() {
            ui.ctx.request_relayout();
        }
        let tw = prev.map_or(0.0, |w| w.0);
        let tr = Rect { min: vec2((r.max.x - tw).max(left), r.min.y), max: r.max };
        let from = ui.ctx.access_nodes().len();
        let (_, used) = ui.row_sized(id, tr, trailing);
        ui.ctx.access_label_since(from, self.title);
        ui.ctx.set(id, W(used.x));
    }
}
