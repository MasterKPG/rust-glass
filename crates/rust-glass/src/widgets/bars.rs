//! Floating bars and panels.

use crate::anim::Tuning;
use crate::color::Color;
use crate::context::{Context, Cursor, Id, Order, Sense};
use crate::geom::{Rect, vec2};
use crate::icons::Icon;
use crate::material::{Corner, Glass, Material};
use crate::text::{TextStyle, Weight};
use crate::ui::Ui;

/// Glass panel at a fixed place above the content.
pub struct Panel {
    id: Id,
    rect: Rect,
    corner: Corner,
    material: Option<Material>,
    pad: f32,
    flips: bool,
    order: Order,
}

impl Panel {
    pub fn new(id: impl std::hash::Hash, rect: Rect) -> Self {
        Self {
            id: Id::new(("panel", Id::new(id).0)),
            rect,
            corner: Corner::Radius(28.0),
            material: None,
            pad: 14.0,
            flips: false,
            order: Order::Bar,
        }
    }
    pub fn corner(mut self, c: impl Into<Corner>) -> Self {
        self.corner = c.into();
        self
    }
    pub fn material(mut self, m: Material) -> Self {
        self.material = Some(m);
        self
    }
    pub fn padding(mut self, p: f32) -> Self {
        self.pad = p;
        self
    }
    /// Small panels may switch between light and dark.
    pub fn flips(mut self) -> Self {
        self.flips = true;
        self
    }
    pub fn order(mut self, o: Order) -> Self {
        self.order = o;
        self
    }

    pub fn show<R>(self, ctx: &mut Context, f: impl FnOnce(&mut Ui) -> R) -> R {
        let layer = ctx.layer(self.id, self.order);
        ctx.interact(self.id.with("block"), self.rect, Rect::EVERYTHING, layer, Sense::Hover);
        let m = self.material.unwrap_or(ctx.theme.thick);
        let mut g = Glass::new(self.rect, self.corner, m);
        let a = ctx.prepare_glass(self.id, &mut g, self.flips);
        ctx.painter(layer, Rect::EVERYTHING).glass(&g);
        let mut ui = Ui::new(ctx, layer, self.id, self.rect.shrink(self.pad));
        ui.surface = a.surface();
        f(&mut ui)
    }
}

/// Plain region of the window, without a surface of its own.
pub fn region<R>(ctx: &mut Context, id: impl std::hash::Hash, rect: Rect, f: impl FnOnce(&mut Ui) -> R) -> R {
    let id = Id::new(("region", Id::new(id).0));
    let layer = ctx.layer(id, Order::Content);
    let mut ui = Ui::new(ctx, layer, id, rect);
    f(&mut ui)
}

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
    let v = ui.ctx.get::<crate::anim::Spring>(id).v;
    let accent = ui.ctx.theme.accent;
    // The highlight stretches along its way.
    let s = (v.abs() * 0.9).min(9.0);
    let hl = Rect::from_min_size(vec2(ui.cursor.x, top + y * (h + 2.0)), vec2(w, h))
        .expand2(vec2(-s * 0.2, s * 0.35));
    let c = ui.surface.fill_strong();
    ui.painter().fill(hl, Corner::Capsule, c);
    for (i, (icon, label)) in items.iter().enumerate() {
        let r = ui.alloc_interact(vec2(w, h), Sense::Click);
        ui.ctx.focusable(r.id);
        if r.clicked {
            *selected = i;
        }
        if r.hovered {
            ui.ctx.cursor = Cursor::Pointer;
        }
        let hov = ui.ctx.animate_bool(r.id.with("h"), r.hovered && i != *selected, Tuning::SNAP);
        let on = ui.ctx.animate_bool(r.id.with("on"), i == *selected, Tuning::SNAP);
        let weight = if i == *selected { Weight::Semibold } else { Weight::Medium };
        let st = TextStyle::new(ui.ctx.theme.body).weight(weight);
        let l = ui.layout_text(label, &st);
        let (f, t) = (ui.surface.fill(), ui.surface.text());
        let mut p = ui.painter();
        p.fill(r.rect, Corner::Capsule, f.fade(hov * 0.7));
        p.icon(vec2(r.rect.min.x + 22.0, r.rect.center().y), 18.0, *icon, t.lerp(accent, on));
        p.text(vec2(r.rect.min.x + 42.0, r.rect.center().y - l.size.y * 0.5), &l, t);
        ui.focus_ring(r.id, r.rect, Corner::Capsule);
    }
    ui.spacing = old;
    before != *selected
}

/// Content card with a title, for grouping controls.
pub fn section(ui: &mut Ui, title: &str, f: impl FnOnce(&mut Ui)) {
    let c = ui.surface.secondary();
    let st = TextStyle::new(13.0).weight(Weight::Semibold);
    ui.text_with(title, st, c);
    ui.card(f);
}

/// Two column table with a header row.
pub fn table(ui: &mut Ui, header: &[&str], rows: &[Vec<String>]) {
    let n = header.len().max(1);
    let w = ui.available_width() / n as f32;
    let (tc, sc, sep) = (ui.surface.text(), ui.surface.secondary(), ui.surface.separator());
    let hs = TextStyle::new(12.0).weight(Weight::Semibold);
    let bs = TextStyle::new(ui.ctx.theme.body);
    let total = ui.available_width();
    let old = ui.spacing;
    ui.spacing = 0.0;
    let line = |ui: &mut Ui, cells: &[&str], st: &TextStyle, c: Color, h: f32, i: usize| {
        let r = ui.alloc_interact(vec2(total, h), Sense::Hover);
        let hov = ui.ctx.animate_bool(r.id, r.hovered && i > 0, Tuning::SNAP);
        let ls: Vec<_> = cells.iter().map(|t| ui.layout_text(t, st)).collect();
        let f = ui.surface.fill();
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
