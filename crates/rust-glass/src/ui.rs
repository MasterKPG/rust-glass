//! Layout. A `Ui` is a region that hands out rectangles to widgets.

use std::hash::Hash;
use std::rc::Rc;

use crate::color::Color;
use crate::context::{Context, Id, Response, Sense};
use crate::geom::{Rect, Vec2, vec2};
use crate::material::{Corner, Glass, Material};
use crate::paint::Painter;
use crate::text::{TextLayout, TextStyle, Weight};
use crate::theme::Surface;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Dir {
    Vertical,
    /// Row of the given height; children are centered in it.
    Horizontal {
        row: f32,
    },
}

/// Anything that can be added with [`Ui::add`].
pub trait Widget {
    fn ui(self, ui: &mut Ui) -> Response;
}

pub struct Ui<'a> {
    pub ctx: &'a mut Context,
    pub layer: usize,
    pub clip: Rect,
    pub mask: u32,
    pub opacity: f32,
    pub max: Rect,
    pub cursor: Vec2,
    pub spacing: f32,
    pub align: Align,
    pub enabled: bool,
    pub surface: Surface,
    pub id: Id,
    dir: Dir,
    content: Rect,
    auto: u64,
    first: bool,
}

#[derive(Clone, Copy, Default)]
struct SizeMem(Vec2);

impl<'a> Ui<'a> {
    pub fn new(ctx: &'a mut Context, layer: usize, id: Id, max: Rect) -> Self {
        let spacing = ctx.theme.spacing;
        let surface = Surface::Content { dark: ctx.theme.dark };
        Ui {
            ctx,
            layer,
            clip: Rect::EVERYTHING,
            mask: 0,
            opacity: 1.0,
            max,
            cursor: max.min,
            spacing,
            align: Align::Start,
            enabled: true,
            surface,
            id,
            dir: Dir::Vertical,
            content: Rect::from_min_size(max.min, Vec2::ZERO),
            auto: 0,
            first: true,
        }
    }

    fn sub(&mut self, id: Id, max: Rect, dir: Dir) -> Ui<'_> {
        Ui {
            layer: self.layer,
            clip: self.clip,
            mask: self.mask,
            opacity: self.opacity,
            max,
            cursor: max.min,
            spacing: self.spacing,
            align: self.align,
            enabled: self.enabled,
            surface: self.surface,
            id,
            dir,
            content: Rect::from_min_size(max.min, Vec2::ZERO),
            auto: 0,
            first: true,
            ctx: self.ctx,
        }
    }

    /// Next automatic id.
    pub fn next_id(&mut self) -> Id {
        self.auto += 1;
        self.id.with(self.auto)
    }
    pub fn make_id(&self, v: impl Hash) -> Id {
        self.id.with(v)
    }

    pub fn available_width(&self) -> f32 {
        (self.max.max.x - self.cursor.x).max(0.0)
    }
    pub fn available_height(&self) -> f32 {
        (self.max.max.y - self.cursor.y).max(0.0)
    }
    pub fn available(&self) -> Rect {
        Rect { min: self.cursor, max: self.max.max.max(self.cursor) }
    }
    /// Space used so far.
    pub fn used(&self) -> Rect {
        self.content
    }
    pub fn is_horizontal(&self) -> bool {
        matches!(self.dir, Dir::Horizontal { .. })
    }

    pub fn alloc(&mut self, size: Vec2) -> Rect {
        let gap = if self.first { 0.0 } else { self.spacing };
        self.first = false;
        let r = match self.dir {
            Dir::Vertical => {
                self.cursor.y += gap;
                let free = (self.max.width() - size.x).max(0.0);
                let x = match self.align {
                    Align::Start => self.max.min.x,
                    Align::Center => self.max.min.x + free * 0.5,
                    Align::End => self.max.min.x + free,
                };
                let r = Rect::from_min_size(vec2(x, self.cursor.y), size);
                self.cursor.y += size.y;
                r
            }
            Dir::Horizontal { row } => {
                self.cursor.x += gap;
                let y = self.max.min.y + ((row - size.y) * 0.5).max(0.0);
                let r = Rect::from_min_size(vec2(self.cursor.x, y), size);
                self.cursor.x += size.x;
                r
            }
        };
        self.content = self.content.union(r);
        // Whole pixels keep edges and text crisp.
        let p = self.ctx.ppp;
        let snap = |v: f32| (v * p).round() / p;
        Rect::from_min_size(vec2(snap(r.min.x), snap(r.min.y)), size)
    }

    /// Where the next widget will start.
    pub fn next_pos(&self) -> Vec2 {
        let gap = if self.first { 0.0 } else { self.spacing };
        match self.dir {
            Dir::Vertical => vec2(self.max.min.x, self.cursor.y + gap),
            Dir::Horizontal { .. } => vec2(self.cursor.x + gap, self.max.min.y),
        }
    }

    /// Reserve space and sense input on it.
    pub fn alloc_interact(&mut self, size: Vec2, sense: Sense) -> Response {
        let r = self.alloc(size);
        let id = self.next_id();
        self.interact(id, r, sense)
    }

    pub fn interact(&mut self, id: Id, r: Rect, sense: Sense) -> Response {
        if !self.enabled {
            let mut res = self.ctx.interact(id, r, self.clip, self.layer, Sense::Hover);
            res.hovered = false;
            return res;
        }
        self.ctx.interact(id, r, self.clip, self.layer, sense)
    }

    pub fn space(&mut self, v: f32) {
        match self.dir {
            Dir::Vertical => self.cursor.y += v,
            Dir::Horizontal { .. } => self.cursor.x += v,
        }
        self.content.max = self.content.max.max(vec2(
            if self.is_horizontal() { self.cursor.x } else { self.content.max.x },
            if self.is_horizontal() { self.content.max.y } else { self.cursor.y },
        ));
    }

    pub fn painter(&mut self) -> Painter<'_> {
        let (l, c, m, o) = (self.layer, self.clip, self.mask, self.opacity);
        let o = if self.enabled { o } else { o * 0.4 };
        let mut p = self.ctx.painter(l, c);
        p.mask = m;
        p.opacity = o;
        p
    }

    pub fn layout_text(&mut self, text: &str, st: &TextStyle) -> Rc<TextLayout> {
        self.ctx.text.layout(text, st)
    }

    pub fn add(&mut self, w: impl Widget) -> Response {
        w.ui(self)
    }

    pub fn add_enabled<R>(&mut self, enabled: bool, f: impl FnOnce(&mut Ui) -> R) -> R {
        let old = self.enabled;
        self.enabled &= enabled;
        let r = f(self);
        self.enabled = old;
        r
    }

    // ----- containers -----

    fn size_of(&mut self, id: Id) -> Vec2 {
        match self.ctx.try_get::<SizeMem>(id) {
            Some(s) => s.0,
            None => {
                self.ctx.request_relayout();
                Vec2::ZERO
            }
        }
    }
    fn remember(&mut self, id: Id, size: Vec2) {
        let old = self.ctx.try_get::<SizeMem>(id).map(|s| s.0);
        if old.is_none_or(|o| (o - size).length() > 0.5) {
            if old.is_some() {
                self.ctx.request_repaint();
            }
            self.ctx.set(id, SizeMem(size));
        } else {
            self.ctx.set(id, SizeMem(old.unwrap()));
        }
    }

    /// Children side by side, vertically centered.
    pub fn horizontal<R>(&mut self, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.row(Align::Start, f)
    }
    /// Row centered in the available width.
    pub fn horizontal_centered<R>(&mut self, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.row(Align::Center, f)
    }
    /// Row pushed to the right edge.
    pub fn horizontal_right<R>(&mut self, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.row(Align::End, f)
    }

    fn row<R>(&mut self, align: Align, f: impl FnOnce(&mut Ui) -> R) -> R {
        let id = self.next_id();
        let prev = self.size_of(id);
        let avail = self.available_width();
        let gap = if self.first { 0.0 } else { self.spacing };
        let start = self.cursor + if self.is_horizontal() { vec2(gap, 0.0) } else { vec2(0.0, gap) };
        let x = match align {
            Align::Start => start.x,
            Align::Center => start.x + ((avail - prev.x) * 0.5).max(0.0),
            Align::End => start.x + (avail - prev.x).max(0.0),
        };
        let row = prev.y;
        // Nested in another row: center this one too.
        let y = match self.dir {
            Dir::Horizontal { row: outer } => self.max.min.y + ((outer - row) * 0.5).max(0.0),
            Dir::Vertical => start.y,
        };
        let max = Rect { min: vec2(x, y), max: vec2(self.max.max.x.max(x), self.max.max.y) };
        let mut ui = self.sub(id, max, Dir::Horizontal { row });
        ui.align = Align::Start;
        let out = f(&mut ui);
        let used = ui.content.size();
        self.remember(id, used);
        let taken = match (align, self.dir) {
            (Align::Start, _) | (_, Dir::Horizontal { .. }) => used,
            _ => vec2(avail, used.y),
        };
        self.alloc(taken);
        out
    }

    /// Children stacked in a column of the given width.
    pub fn vertical<R>(&mut self, width: f32, f: impl FnOnce(&mut Ui) -> R) -> R {
        let id = self.next_id();
        let prev = self.size_of(id);
        let gap = if self.first { 0.0 } else { self.spacing };
        let start = match self.dir {
            Dir::Horizontal { row } => {
                vec2(self.cursor.x + gap, self.max.min.y + ((row - prev.y) * 0.5).max(0.0))
            }
            Dir::Vertical => self.cursor + vec2(0.0, gap),
        };
        let max = Rect { min: start, max: vec2(start.x + width, self.max.max.y) };
        let mut ui = self.sub(id, max, Dir::Vertical);
        let out = f(&mut ui);
        let used = ui.content.size();
        self.remember(id, used);
        self.alloc(vec2(width, used.y));
        out
    }

    /// Equal width columns. `f` is called once per column.
    pub fn grid(&mut self, n: usize, gap: f32, mut f: impl FnMut(&mut Ui, usize)) {
        let id = self.next_id();
        let w = ((self.available_width() - gap * (n as f32 - 1.0)) / n as f32).max(1.0);
        let g = if self.first { 0.0 } else { self.spacing };
        let top = self.cursor.y + g;
        let left = self.cursor.x;
        let mut h: f32 = 0.0;
        for i in 0..n {
            let min = vec2(left + i as f32 * (w + gap), top);
            let max = Rect { min, max: vec2(min.x + w, self.max.max.y) };
            let mut ui = self.sub(id.with(i), max, Dir::Vertical);
            f(&mut ui, i);
            h = h.max(ui.content.height());
        }
        self.alloc(vec2(w * n as f32 + gap * (n as f32 - 1.0), h));
    }

    /// Inset region.
    pub fn padded<R>(&mut self, pad: Vec2, f: impl FnOnce(&mut Ui) -> R) -> R {
        let id = self.next_id();
        let gap = if self.first || self.is_horizontal() { 0.0 } else { self.spacing };
        let start = self.cursor + vec2(0.0, gap);
        let max = Rect { min: start + pad, max: vec2(self.max.max.x - pad.x, self.max.max.y) };
        let mut ui = self.sub(id, max, Dir::Vertical);
        let out = f(&mut ui);
        let used = ui.content.size();
        self.alloc(vec2(self.available_width(), used.y + pad.y * 2.0));
        out
    }

    /// Run `f` in an explicit rectangle, without affecting this layout.
    pub fn in_rect<R>(&mut self, id: impl Hash, r: Rect, f: impl FnOnce(&mut Ui) -> R) -> R {
        let id = self.id.with(id);
        let mut ui = self.sub(id, r, Dir::Vertical);
        f(&mut ui)
    }

    /// Same, returning the size that was used.
    pub fn in_rect_sized<R>(&mut self, id: impl Hash, r: Rect, f: impl FnOnce(&mut Ui) -> R) -> (R, Vec2) {
        let id = self.id.with(id);
        let mut ui = self.sub(id, r, Dir::Vertical);
        let out = f(&mut ui);
        (out, ui.content.size())
    }

    /// Row layout in an explicit rectangle, children centered on its height.
    pub fn row_in_rect<R>(&mut self, id: impl Hash, r: Rect, f: impl FnOnce(&mut Ui) -> R) -> R {
        let id = self.id.with(id);
        let mut ui = self.sub(id, r, Dir::Horizontal { row: r.height() });
        f(&mut ui)
    }

    pub(crate) fn row_sized<R>(&mut self, id: Id, r: Rect, f: impl FnOnce(&mut Ui) -> R) -> (R, Vec2) {
        let mut ui = self.sub(id, r, Dir::Horizontal { row: r.height() });
        let out = f(&mut ui);
        (out, ui.content.size())
    }

    /// Opaque-ish content card, not glass. Glass belongs on floating controls.
    pub fn card<R>(&mut self, f: impl FnOnce(&mut Ui) -> R) -> R {
        let id = self.next_id();
        let w = self.available_width();
        let gap = if self.first || self.is_horizontal() { 0.0 } else { self.spacing };
        let start = self.cursor + vec2(0.0, gap);
        let h = self.size_of(id.with("card")).y;
        let r = Rect::from_min_size(start, vec2(w, h));
        let fill = if self.surface.dark() > 0.5 { Color::gray(0.13).alpha(0.82) } else { Color::white(0.80) };
        self.painter().fill(r, 20.0, fill);
        let max = Rect { min: start + vec2(18.0, 16.0), max: vec2(start.x + w - 18.0, self.max.max.y) };
        let mut ui = self.sub(id, max, Dir::Vertical);
        let out = f(&mut ui);
        let used = ui.content.height() + 32.0;
        self.remember(id.with("card"), vec2(w, used));
        self.alloc(vec2(w, used));
        out
    }

    /// A glass panel sized to its content, part of the normal layout.
    pub fn glass_panel<R>(&mut self, material: Material, f: impl FnOnce(&mut Ui) -> R) -> R {
        let id = self.next_id();
        let w = self.available_width();
        let gap = if self.first || self.is_horizontal() { 0.0 } else { self.spacing };
        let start = self.cursor + vec2(0.0, gap);
        let h = self.size_of(id.with("card")).y;
        let r = Rect::from_min_size(start, vec2(w, h));
        let surface = self.glass(id, r, Corner::Radius(24.0), material, false);
        let max = Rect { min: start + vec2(18.0, 16.0), max: vec2(start.x + w - 18.0, self.max.max.y) };
        let mut ui = self.sub(id, max, Dir::Vertical);
        ui.surface = surface;
        let out = f(&mut ui);
        let used = ui.content.height() + 32.0;
        self.remember(id.with("card"), vec2(w, used));
        self.alloc(vec2(w, used));
        out
    }

    /// Paint an adaptive glass element and return the surface for its content.
    pub fn glass(&mut self, id: Id, r: Rect, corner: impl Into<Corner>, m: Material, flips: bool) -> Surface {
        let mut g = Glass::new(r, corner, m);
        let a = self.ctx.prepare_glass(id, &mut g, flips);
        self.painter().glass(&g);
        a.surface()
    }

    // ----- text -----

    pub fn label(&mut self, text: &str) -> Response {
        let c = self.surface.text();
        self.text_with(text, TextStyle::new(self.ctx.theme.body), c)
    }
    pub fn secondary(&mut self, text: &str) -> Response {
        let c = self.surface.secondary();
        self.text_with(text, TextStyle::new(self.ctx.theme.body), c)
    }
    pub fn caption(&mut self, text: &str) -> Response {
        let c = self.surface.secondary();
        self.text_with(text, TextStyle::new(self.ctx.theme.caption), c)
    }
    pub fn strong(&mut self, text: &str) -> Response {
        let c = self.surface.text();
        self.text_with(text, TextStyle::new(self.ctx.theme.body).weight(Weight::Semibold), c)
    }
    pub fn heading(&mut self, text: &str) -> Response {
        let c = self.surface.text();
        self.text_with(text, TextStyle::new(self.ctx.theme.title).weight(Weight::Bold), c)
    }
    pub fn subheading(&mut self, text: &str) -> Response {
        let c = self.surface.text();
        self.text_with(text, TextStyle::new(17.0).weight(Weight::Semibold), c)
    }

    pub fn text_with(&mut self, text: &str, mut st: TextStyle, c: Color) -> Response {
        if !self.is_horizontal() && st.wrap.is_none() {
            st.wrap = Some(self.max.width().max(10.0));
        }
        let l = self.layout_text(text, &st);
        let r = self.alloc(l.size);
        self.painter().text(r.min, &l, c);
        let id = self.next_id();
        Response {
            id,
            rect: r,
            hovered: self.ctx.input.pointer.is_some_and(|p| r.contains(p)),
            held: false,
            pressed: false,
            released: false,
            clicked: false,
            double_clicked: false,
            secondary_clicked: false,
            focused: false,
            changed: false,
            delta: Vec2::ZERO,
            pointer: self.ctx.input.pointer,
        }
    }

    pub fn separator(&mut self) {
        let c = self.surface.separator();
        match self.dir {
            Dir::Vertical => {
                let w = self.available_width();
                let r = self.alloc(vec2(w, 1.0));
                self.painter().fill(r, 0.0, c);
            }
            Dir::Horizontal { row } => {
                let h = row.max(20.0) * 0.6;
                let r = self.alloc(vec2(1.0, h));
                self.painter().fill(r, 0.0, c);
            }
        }
    }

    /// Keyboard focus ring.
    pub fn focus_ring(&mut self, id: Id, r: Rect, corner: impl Into<Corner>) {
        if self.ctx.shows_focus(id) {
            let c = self.ctx.theme.accent;
            self.painter().stroke(r.expand(3.5), corner, 2.5, c.alpha(0.85));
        }
    }
}
