use crate::anim::Tuning;
use crate::color::Color;
use crate::context::{Context, Cursor, Id, Response, Sense};
use crate::geom::{Rect, Vec2, vec2};
use crate::icons::Icon;
use crate::material::{Corner, Glass, GlassShape, Material};
use crate::text::{TextStyle, Weight};
use crate::ui::{Ui, Widget};
use crate::widgets::gel::Gel;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ButtonKind {
    Glass,
    /// Accent colored glass for the primary action.
    Prominent,
    Clear,
    Tinted(Color),
    /// No surface until hovered.
    Plain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonSize {
    Small,
    Regular,
    Large,
}

#[derive(Clone, Debug)]
pub struct Button<'a> {
    label: &'a str,
    icon: Option<Icon>,
    kind: ButtonKind,
    size: ButtonSize,
    corner: Corner,
    min_width: f32,
    width: Option<f32>,
    selected: bool,
    id: Option<Id>,
}

impl<'a> Button<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            icon: None,
            kind: ButtonKind::Glass,
            size: ButtonSize::Regular,
            corner: Corner::Capsule,
            min_width: 0.0,
            width: None,
            selected: false,
            id: None,
        }
    }
    /// Round button with only an icon.
    pub fn icon_only(icon: Icon) -> Self {
        Self { icon: Some(icon), ..Self::new("") }
    }
    pub fn icon(mut self, i: Icon) -> Self {
        self.icon = Some(i);
        self
    }
    pub fn kind(mut self, k: ButtonKind) -> Self {
        self.kind = k;
        self
    }
    pub fn prominent(self) -> Self {
        self.kind(ButtonKind::Prominent)
    }
    pub fn clear(self) -> Self {
        self.kind(ButtonKind::Clear)
    }
    pub fn plain(self) -> Self {
        self.kind(ButtonKind::Plain)
    }
    pub fn tinted(self, c: Color) -> Self {
        self.kind(ButtonKind::Tinted(c))
    }
    pub fn size(mut self, s: ButtonSize) -> Self {
        self.size = s;
        self
    }
    pub fn small(self) -> Self {
        self.size(ButtonSize::Small)
    }
    pub fn large(self) -> Self {
        self.size(ButtonSize::Large)
    }
    pub fn corner(mut self, c: impl Into<Corner>) -> Self {
        self.corner = c.into();
        self
    }
    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = w;
        self
    }
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    /// Draw as switched on.
    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }
    pub fn id(mut self, id: Id) -> Self {
        self.id = Some(id);
        self
    }

    fn metrics(&self, ctx: &Context) -> (f32, f32, f32, f32) {
        let h = ctx.theme.control_height;
        match self.size {
            ButtonSize::Small => (h * 0.8, 12.0, 14.0, 11.0),
            ButtonSize::Regular => (h, 14.0, 17.0, 15.0),
            ButtonSize::Large => (h * 1.36, 16.0, 21.0, 22.0),
        }
    }
}

impl Widget for Button<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let (h, font, icon_size, pad) = self.metrics(ui.ctx);
        let st = TextStyle::new(font).weight(Weight::Semibold);
        let text = (!self.label.is_empty()).then(|| ui.layout_text(self.label, &st));
        let tw = text.as_ref().map_or(0.0, |t| t.size.x);
        let gap = if self.icon.is_some() && text.is_some() { 7.0 } else { 0.0 };
        let iw = if self.icon.is_some() { icon_size } else { 0.0 };
        let w = if text.is_none() { h } else { (tw + gap + iw + pad * 2.0).max(self.min_width) };
        let w = self.width.unwrap_or(w);
        let rect = ui.alloc(vec2(w, h));
        let id = match self.id {
            Some(id) => id,
            None => ui.next_id(),
        };
        let r = ui.interact(id, rect, Sense::Click);
        if ui.enabled {
            ui.ctx.focusable(id);
        }
        if r.hovered {
            ui.ctx.cursor = Cursor::Pointer;
        }

        let strength = if ui.enabled { 1.0 } else { 0.0 };
        let gel = Gel::update(ui.ctx, &r, strength);
        let on_glass = ui.surface.is_glass();
        let accent = ui.ctx.theme.accent;
        let fill = match self.kind {
            ButtonKind::Prominent => Some(accent),
            ButtonKind::Tinted(c) => Some(c),
            _ if self.selected => Some(accent),
            _ => None,
        };
        let mut color = ui.surface.text();
        let theme_regular = ui.ctx.theme.regular;
        let theme_clear = ui.ctx.theme.clear;
        match (self.kind, on_glass) {
            (ButtonKind::Plain, _) => {
                color = accent;
                let a = gel.hover * 0.8 + gel.press * 0.8;
                let f = ui.surface.fill().fade(a);
                ui.painter().fill(gel.rect, self.corner, f);
            }
            // Glass on glass muddies both. On a glass surface buttons are fills.
            (_, true) => {
                let base = fill.unwrap_or(ui.surface.fill());
                let lit = Color::white(0.05 * gel.hover + 0.08 * gel.press.max(0.0));
                let mut p = ui.painter();
                p.fill(gel.rect, self.corner, base);
                p.fill(gel.rect, self.corner, lit);
                if fill.is_some() {
                    color = Color::WHITE;
                }
            }
            (kind, false) => {
                let mut m = if kind == ButtonKind::Clear { theme_clear } else { theme_regular };
                if let Some(c) = fill {
                    m = tinted(m, c);
                }
                let mut g = Glass::new(gel.rect, self.corner, m);
                let a = ui.ctx.prepare_glass(id, &mut g, true);
                gel.apply(&mut g);
                color = if fill.is_some() { Color::WHITE } else { a.surface().text() };
                ui.painter().glass(&g);
            }
        }

        let c = gel.rect.center();
        let total = (tw + gap + iw) * gel.scale;
        let mut x = c.x - total * 0.5;
        let mut p = ui.painter();
        if let Some(i) = self.icon {
            p.icon(vec2(x + iw * gel.scale * 0.5, c.y), icon_size * gel.scale, i, color);
            x += (iw + gap) * gel.scale;
        }
        if let Some(t) = &text {
            let tr = Rect::from_center_size(vec2(x + tw * gel.scale * 0.5, c.y), t.size);
            p.text_centered(tr, t, color, gel.scale);
        }
        ui.focus_ring(id, gel.rect, self.corner);
        r
    }
}

/// Colored glass: denser and less blurred through than the neutral material.
pub fn tinted(mut m: Material, c: Color) -> Material {
    m.tint = Some(c.alpha(0.86));
    m.inner_glow = 0.15;
    m.rim = 0.8;
    m.shadow_tint = 0.7;
    m
}

/// A row of icon buttons on one piece of glass.
pub struct ButtonGroup<'a> {
    items: &'a [(Icon, &'a str)],
}

impl<'a> ButtonGroup<'a> {
    /// Icon and tooltip per button.
    pub fn new(items: &'a [(Icon, &'a str)]) -> Self {
        Self { items }
    }

    /// Returns the clicked index.
    pub fn show(self, ui: &mut Ui) -> Option<usize> {
        let h = ui.ctx.theme.control_height + 4.0;
        let cell = h + 4.0;
        let n = self.items.len() as f32;
        let rect = ui.alloc(vec2(cell * n, h));
        let id = ui.next_id();
        let mut clicked = None;
        let mut gels = Vec::with_capacity(self.items.len());
        let mut bar = rect;
        let mut press: f32 = 0.0;
        for (i, item) in self.items.iter().enumerate() {
            let r = Rect::from_min_size(vec2(rect.min.x + cell * i as f32, rect.min.y), vec2(cell, h));
            let res = ui.interact(id.with(i), r, Sense::Click);
            ui.ctx.focusable(res.id);
            if res.clicked {
                clicked = Some(i);
            }
            let g = Gel::update(ui.ctx, &res, 1.0);
            press = press.max(g.press);
            if g.press.abs() > 0.001 {
                // The whole bar swells around the pressed item.
                bar = bar.union(g.rect.shrink2(vec2(0.0, 1.0)));
            }
            if res.hovered {
                crate::widgets::overlay::tooltip(ui.ctx, res.id, r, item.1);
            }
            gels.push((g, r));
        }
        let m = ui.ctx.theme.regular;
        let mut glass = Glass::new(bar, Corner::Capsule, m);
        let a = ui.ctx.prepare_glass(id, &mut glass, true);
        glass.lit = 0.04 * press;
        let color = a.surface().text();
        let hl = a.surface().fill();
        let mut p = ui.painter();
        p.glass(&glass);
        for (i, (g, r)) in gels.iter().enumerate() {
            p.fill(r.shrink(4.0), Corner::Capsule, hl.fade(g.hover * 0.8));
            p.icon(g.rect.center(), 18.0 * g.scale, self.items[i].0, color);
        }
        clicked
    }
}

/// A floating action button that splits into several when pressed. The pieces
/// are one fused glass surface, so they part and merge like drops.
pub struct SplitButton<'a> {
    pub icon: Icon,
    pub items: &'a [(Icon, Color)],
}

impl SplitButton<'_> {
    pub fn show(self, ui: &mut Ui) -> Option<usize> {
        let d = 52.0;
        let gap = 12.0;
        let n = self.items.len();
        let rect = ui.alloc(vec2(d + (d + gap) * n as f32, d));
        let id = ui.next_id();
        let mut open: bool = ui.ctx.get(id);
        let home = Rect::from_min_size(rect.min, Vec2::splat(d));
        let main = ui.interact(id.with("main"), home, Sense::Click);
        if main.clicked {
            open = !open;
        }
        ui.ctx.set(id, open);
        let gel = Gel::update(ui.ctx, &main, 1.0);
        let mut clicked = None;
        let mut shapes = vec![GlassShape::new(gel.rect, Corner::Capsule)];
        let mut faces = Vec::new();
        let (mut lead, mut tail) = (0.0f32, 1.0f32);
        for (i, item) in self.items.iter().enumerate() {
            // Each drop leaves a little later than the one before.
            let t = ui.ctx.animate(
                id.with(("drop", i)),
                open as u32 as f32,
                Tuning::new(0.40 + 0.055 * i as f32, 0.72),
            );
            lead = lead.max(t.clamp(0.0, 1.0));
            tail = tail.min(t.clamp(0.0, 1.0));
            let x = home.min.x + (d + gap) * (i + 1) as f32 * t;
            let size = d * (0.45 + 0.55 * t.clamp(0.0, 1.0));
            let r = Rect::from_center_size(vec2(x + d * 0.5, home.center().y), Vec2::splat(size));
            let hit = if open { r } else { Rect::NOTHING };
            let res = ui.interact(id.with(("item", i)), hit, if open { Sense::Click } else { Sense::Hover });
            let g = Gel::update(ui.ctx, &Response { rect: r, ..res }, if open { 1.0 } else { 0.0 });
            if res.clicked {
                clicked = Some(i);
                ui.ctx.set(id, false);
            }
            shapes.push(
                GlassShape::new(g.rect, Corner::Capsule).tint(Some(item.1.alpha(0.62 * t.clamp(0.0, 1.0)))),
            );
            faces.push((g, item.0, t));
        }
        // The pull between drops exists only while they are in motion: none
        // while they sit inside the main one, none once they have parted.
        let part = crate::geom::smoothstep(0.55, 0.96, tail);
        let fuse = 22.0 * crate::geom::smoothstep(0.0, 0.25, lead) * (1.0 - part);
        let m = ui.ctx.theme.regular;
        let mut glass = Glass::group(shapes, m).fuse(fuse);
        let a = ui.ctx.prepare_glass(id, &mut glass, true);
        glass.lit = 0.05 * gel.press.max(0.0);
        let color = a.surface().text();
        let t = ui.ctx.animate(id.with("rot"), open as u32 as f32, Tuning::BOUNCY);
        let mut p = ui.painter();
        p.glass(&glass);
        let c = gel.rect.center();
        // Plus that turns into a cross.
        let a0 = t * std::f32::consts::FRAC_PI_4;
        if self.icon == Icon::Plus {
            for k in 0..2 {
                let a = a0 + k as f32 * std::f32::consts::FRAC_PI_2;
                let v = vec2(a.cos(), a.sin()) * 8.5 * gel.scale;
                p.line(c - v, c + v, 2.4, color);
            }
        } else {
            p.icon(c, 22.0 * gel.scale, self.icon, color);
        }
        for (g, icon, t) in &faces {
            let k = ((t - 0.35) / 0.5).clamp(0.0, 1.0);
            p.icon(g.rect.center(), 22.0 * g.scale * (0.6 + 0.4 * k), *icon, Color::white(k));
        }
        clicked
    }
}

impl Ui<'_> {
    pub fn button(&mut self, label: &str) -> Response {
        self.add(Button::new(label))
    }
    pub fn prominent_button(&mut self, label: &str) -> Response {
        self.add(Button::new(label).prominent())
    }
    pub fn icon_button(&mut self, icon: Icon) -> Response {
        self.add(Button::icon_only(icon))
    }

    /// Text that behaves like a link.
    pub fn link(&mut self, label: &str) -> Response {
        let st = TextStyle::new(self.ctx.theme.body).weight(Weight::Medium);
        let l = self.layout_text(label, &st);
        let r = self.alloc_interact(l.size, Sense::Click);
        let c = self.ctx.theme.accent;
        let u = self.ctx.animate_bool(r.id.with("u"), r.hovered, Tuning::SNAP);
        if r.hovered {
            self.ctx.cursor = Cursor::Pointer;
        }
        self.ctx.focusable(r.id);
        let mut p = self.painter();
        p.text(r.rect.min, &l, c);
        let y = r.rect.max.y - 1.0;
        let w = r.rect.width();
        p.line(vec2(r.rect.min.x, y), vec2(r.rect.min.x + w * u, y), 1.2, c.fade(u));
        self.focus_ring(r.id, r.rect, 4.0);
        r
    }

    pub fn hyperlink(&mut self, label: &str, url: &str) -> Response {
        let r = self.link(label);
        if r.clicked {
            self.ctx.host.push(crate::context::HostCmd::OpenUrl(url.into()));
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{Context, Order};
    use crate::input::Event;

    #[test]
    fn drops_part_completely() {
        let mut ctx = Context::new();
        let mut frame = |i: usize, ev: &[Event]| {
            ctx.begin_frame(vec2(600.0, 200.0), 1.0, i as f64 / 60.0, ev);
            let layer = ctx.layer(Id::new("l"), Order::Content);
            let mut ui = Ui::new(&mut ctx, layer, Id::new("u"), crate::geom::rect(20.0, 20.0, 500.0, 100.0));
            SplitButton { icon: Icon::Plus, items: &[(Icon::Star, Color::RED), (Icon::Heart, Color::BLUE)] }
                .show(&mut ui);
            ctx.end_frame();
            ctx.paint.data.glasses[0].fuse
        };
        assert_eq!(frame(0, &[]), 0.0);
        frame(1, &[Event::PointerMoved(vec2(45.0, 45.0))]);
        frame(2, &[Event::PointerDown]);
        frame(3, &[Event::PointerUp]);
        let fuse: Vec<f32> = (4..120).map(|i| frame(i, &[])).collect();
        assert!(fuse.iter().cloned().fold(0.0, f32::max) > 10.0, "they pull while parting");
        assert_eq!(*fuse.last().unwrap(), 0.0);
    }
}
