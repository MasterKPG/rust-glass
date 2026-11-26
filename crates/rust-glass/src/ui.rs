//! Layout. A `Ui` is a region that hands out rectangles to widgets.

use std::hash::Hash;
use std::rc::Rc;

use crate::accessibility::{Node, Role};
use crate::color::Color;
use crate::context::{Context, Id, Response, Sense};
use crate::geometry::{Rect, Vec2, vec2};
use crate::material::{Corner, Glass, Material};
use crate::paint::Painter;
use crate::text::{TextLayout, TextStyle, Weight};
use crate::theme::Surface;

/// How children line up across the direction of a layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    /// At the left, or the top in a row.
    Start,
    /// In the middle.
    Center,
    /// At the right, or the bottom in a row.
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
    /// Lays out and draws the widget, and returns what happened to it.
    fn ui(self, ui: &mut Ui) -> Response;
}

/// A region that places widgets one after another and hands each a
/// rectangle.
///
/// A `Ui` stacks widgets vertically unless it was made by
/// [`Ui::horizontal`]. Widgets are drawn on its layer and clipped to
/// [`Ui::clip`].
pub struct Ui<'a> {
    /// The window state.
    pub ctx: &'a mut Context,
    /// The layer widgets are drawn on.
    pub layer: usize,
    /// Widgets are only drawn and only react inside this rectangle.
    pub clip: Rect,
    /// Mask applied to everything drawn. 0 is none.
    pub mask: u32,
    /// Opacity of everything drawn, from 0 to 1.
    pub opacity: f32,
    /// The space this `Ui` may use.
    pub max_rect: Rect,
    /// Where the next widget goes.
    pub cursor: Vec2,
    /// Gap between widgets, in points.
    pub spacing: f32,
    /// How widgets line up across the layout direction.
    pub align: Align,
    /// Widgets react to input. Disabled widgets are drawn faded.
    pub enabled: bool,
    /// What widgets are drawn on. It decides label and fill colors.
    pub surface: Surface,
    /// Id of this `Ui`. Widget ids are derived from it.
    pub id: Id,
    dir: Dir,
    content: Rect,
    auto: u64,
    first: bool,
}

#[derive(Clone, Copy, Default)]
struct SizeMem(Vec2);

impl<'a> Ui<'a> {
    /// Creates a vertical layout in `max_rect`. Most code gets a `Ui` from
    /// [`region`](crate::prelude::region) or [`Panel`](crate::prelude::Panel)
    /// instead.
    pub fn new(ctx: &'a mut Context, layer: usize, id: Id, max: Rect) -> Self {
        let spacing = ctx.theme.spacing;
        let surface = Surface::Content { dark: ctx.theme.dark };
        Ui {
            ctx,
            layer,
            clip: Rect::EVERYTHING,
            mask: 0,
            opacity: 1.0,
            max_rect: max,
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
            max_rect: max,
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

    /// Returns a new id for the next widget, derived from its position in this
    /// layout.
    pub fn next_id(&mut self) -> Id {
        self.auto += 1;
        self.id.with(self.auto)
    }
    /// Derives an id from this `Ui`'s id and `source`.
    pub fn make_id(&self, source: impl Hash) -> Id {
        self.id.with(source)
    }

    /// Returns the width left in this layout.
    pub fn available_width(&self) -> f32 {
        (self.max_rect.max.x - self.cursor.x).max(0.0)
    }
    /// Returns the height left in this layout.
    pub fn available_height(&self) -> f32 {
        (self.max_rect.max.y - self.cursor.y).max(0.0)
    }
    /// Returns the space left in this layout.
    pub fn available(&self) -> Rect {
        Rect { min: self.cursor, max: self.max_rect.max.max(self.cursor) }
    }
    /// Returns the space used so far.
    pub fn used(&self) -> Rect {
        self.content
    }
    /// Returns `true` in a row.
    pub fn is_horizontal(&self) -> bool {
        matches!(self.dir, Dir::Horizontal { .. })
    }

    /// Takes space for a widget of `size` and returns where it goes.
    pub fn allocate(&mut self, size: Vec2) -> Rect {
        let gap = if self.first { 0.0 } else { self.spacing };
        self.first = false;
        let r = match self.dir {
            Dir::Vertical => {
                self.cursor.y += gap;
                let free = (self.max_rect.width() - size.x).max(0.0);
                let x = match self.align {
                    Align::Start => self.max_rect.min.x,
                    Align::Center => self.max_rect.min.x + free * 0.5,
                    Align::End => self.max_rect.min.x + free,
                };
                let r = Rect::from_min_size(vec2(x, self.cursor.y), size);
                self.cursor.y += size.y;
                r
            }
            Dir::Horizontal { row } => {
                self.cursor.x += gap;
                let y = self.max_rect.min.y + ((row - size.y) * 0.5).max(0.0);
                let r = Rect::from_min_size(vec2(self.cursor.x, y), size);
                self.cursor.x += size.x;
                r
            }
        };
        self.content = self.content.union(r);
        // Whole pixels keep edges and text crisp.
        let p = self.ctx.pixels_per_point;
        let snap = |v: f32| (v * p).round() / p;
        Rect { min: vec2(snap(r.min.x), snap(r.min.y)), max: vec2(snap(r.max.x), snap(r.max.y)) }
    }

    /// Returns where the next widget will start.
    pub fn next_pos(&self) -> Vec2 {
        let gap = if self.first { 0.0 } else { self.spacing };
        match self.dir {
            Dir::Vertical => vec2(self.max_rect.min.x, self.cursor.y + gap),
            Dir::Horizontal { .. } => vec2(self.cursor.x + gap, self.max_rect.min.y),
        }
    }

    /// Takes space for a widget of `size` and senses input on it.
    pub fn allocate_response(&mut self, size: Vec2, sense: Sense) -> Response {
        let r = self.allocate(size);
        let id = self.next_id();
        self.interact(id, r, sense)
    }

    /// Senses input on `rect` without taking space.
    pub fn interact(&mut self, id: Id, rect: Rect, sense: Sense) -> Response {
        if !self.enabled {
            let mut res = self.ctx.interact(id, rect, self.clip, self.layer, Sense::Hover);
            res.hovered = false;
            return res;
        }
        self.ctx.interact(id, rect, self.clip, self.layer, sense)
    }

    /// Makes the space used so far at least `size`, measured from the top left.
    pub fn reserve(&mut self, size: Vec2) {
        self.content.max = self.content.max.max(self.max_rect.min + size);
        match self.dir {
            Dir::Vertical => self.cursor.y = self.cursor.y.max(self.content.max.y),
            Dir::Horizontal { .. } => self.cursor.x = self.cursor.x.max(self.content.max.x),
        }
    }

    /// Adds an empty gap of `amount` points.
    pub fn space(&mut self, amount: f32) {
        match self.dir {
            Dir::Vertical => self.cursor.y += amount,
            Dir::Horizontal { .. } => self.cursor.x += amount,
        }
        self.content.max = self.content.max.max(vec2(
            if self.is_horizontal() { self.cursor.x } else { self.content.max.x },
            if self.is_horizontal() { self.content.max.y } else { self.cursor.y },
        ));
    }

    /// Returns a painter on this `Ui`'s layer, with its clip, mask and opacity.
    pub fn painter(&mut self) -> Painter<'_> {
        let (l, c, m, o) = (self.layer, self.clip, self.mask, self.opacity);
        let o = if self.enabled { o } else { o * 0.4 };
        let mut p = self.ctx.painter(l, c);
        p.mask = m;
        p.opacity = o;
        p
    }

    /// Lays out `text` in `style`. The result can be painted and measured.
    pub fn layout_text(&mut self, text: &str, style: &TextStyle) -> Rc<TextLayout> {
        self.ctx.text.layout(text, style)
    }

    /// Adds a widget, such as [`Button`](crate::prelude::Button) or
    /// [`Slider`](crate::prelude::Slider).
    pub fn add(&mut self, widget: impl Widget) -> Response {
        widget.ui(self)
    }

    /// Describe an element for screen readers. `None` while none is listening.
    ///
    /// ```
    /// # use rust_glass::prelude::*;
    /// # use rust_glass::accessibility::Role;
    /// # fn f(ui: &mut Ui, r: Response, on: bool) {
    /// if let Some(n) = ui.access(r.id, Role::Switch, r.rect) {
    ///     n.label("Airplane mode").toggled(on);
    /// }
    /// # }
    /// ```
    pub fn access(&mut self, id: Id, role: Role, rect: Rect) -> Option<&mut Node> {
        let enabled = self.enabled;
        let n = self.ctx.access_node(id, role, rect, self.layer)?;
        n.disabled = !enabled;
        Some(n)
    }

    /// Runs `f` with widgets enabled or disabled.
    pub fn add_enabled<R>(&mut self, enabled: bool, f: impl FnOnce(&mut Ui) -> R) -> R {
        let old = self.enabled;
        self.enabled &= enabled;
        let r = f(self);
        self.enabled = old;
        r
    }

    // ----- containers -----

    pub(crate) fn size_of(&mut self, id: Id) -> Vec2 {
        match self.ctx.try_get::<SizeMem>(id) {
            Some(s) => s.0,
            None => {
                self.ctx.request_relayout();
                Vec2::ZERO
            }
        }
    }
    pub(crate) fn remember(&mut self, id: Id, size: Vec2) {
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

    /// Lays out the widgets added in `f` side by side, centered vertically.
    pub fn horizontal<R>(&mut self, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.row(Align::Start, f)
    }
    /// Like [`Self::horizontal`], centered in the available width.
    pub fn horizontal_centered<R>(&mut self, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.row(Align::Center, f)
    }
    /// Like [`Self::horizontal`], pushed to the right edge.
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
            Dir::Horizontal { row: outer } => self.max_rect.min.y + ((outer - row) * 0.5).max(0.0),
            Dir::Vertical => start.y,
        };
        let max = Rect { min: vec2(x, y), max: vec2(self.max_rect.max.x.max(x), self.max_rect.max.y) };
        let mut ui = self.sub(id, max, Dir::Horizontal { row });
        ui.align = Align::Start;
        let out = f(&mut ui);
        let used = ui.content.size();
        self.remember(id, used);
        let taken = match (align, self.dir) {
            (Align::Start, _) | (_, Dir::Horizontal { .. }) => used,
            _ => vec2(avail, used.y),
        };
        self.allocate(taken);
        out
    }

    /// Stacks the widgets added in `f` in a column of `width` points.
    pub fn vertical<R>(&mut self, width: f32, f: impl FnOnce(&mut Ui) -> R) -> R {
        let id = self.next_id();
        let prev = self.size_of(id);
        let gap = if self.first { 0.0 } else { self.spacing };
        let start = match self.dir {
            Dir::Horizontal { row } => {
                vec2(self.cursor.x + gap, self.max_rect.min.y + ((row - prev.y) * 0.5).max(0.0))
            }
            Dir::Vertical => self.cursor + vec2(0.0, gap),
        };
        let max = Rect { min: start, max: vec2(start.x + width, self.max_rect.max.y) };
        let mut ui = self.sub(id, max, Dir::Vertical);
        let out = f(&mut ui);
        let used = ui.content.size();
        self.remember(id, used);
        self.allocate(vec2(width, used.y));
        out
    }

    /// Stacks the widgets added in `f` in a column as wide as they need, placed
    /// like a single widget. Returns the result of `f` and the rectangle used.
    pub fn scope<R>(&mut self, f: impl FnOnce(&mut Ui) -> R) -> (R, Rect) {
        let id = self.next_id();
        self.scope_id(id, f)
    }

    /// Like [`Self::scope`] with an id of your own, so the state of the widgets
    /// inside follows the content when it moves in the layout.
    pub fn scope_id<R>(&mut self, id: Id, f: impl FnOnce(&mut Ui) -> R) -> (R, Rect) {
        let prev = self.size_of(id);
        let gap = if self.first { 0.0 } else { self.spacing };
        let start = match self.dir {
            Dir::Horizontal { row } => {
                vec2(self.cursor.x + gap, self.max_rect.min.y + ((row - prev.y) * 0.5).max(0.0))
            }
            Dir::Vertical => self.cursor + vec2(0.0, gap),
        };
        let max = Rect { min: start, max: self.max_rect.max.max(start) };
        let mut ui = self.sub(id, max, Dir::Vertical);
        let out = f(&mut ui);
        let used = ui.content.size();
        self.remember(id, used);
        let align = std::mem::replace(&mut self.align, Align::Start);
        let r = self.allocate(used);
        self.align = align;
        (out, r)
    }

    /// Splits the width into `columns` columns of equal width with `gap` between
    /// them. `f` is called once per column with its index.
    pub fn grid(&mut self, columns: usize, gap: f32, mut f: impl FnMut(&mut Ui, usize)) {
        let n = columns;
        let id = self.next_id();
        let w = ((self.available_width() - gap * (n as f32 - 1.0)) / n as f32).max(1.0);
        let g = if self.first { 0.0 } else { self.spacing };
        let top = self.cursor.y + g;
        let left = self.cursor.x;
        let mut h: f32 = 0.0;
        for i in 0..n {
            let min = vec2(left + i as f32 * (w + gap), top);
            let max = Rect { min, max: vec2(min.x + w, self.max_rect.max.y) };
            let mut ui = self.sub(id.with(i), max, Dir::Vertical);
            f(&mut ui, i);
            h = h.max(ui.content.height());
        }
        self.allocate(vec2(w * n as f32 + gap * (n as f32 - 1.0), h));
    }

    /// Insets the widgets added in `f` by `padding` on each side.
    pub fn padded<R>(&mut self, padding: Vec2, f: impl FnOnce(&mut Ui) -> R) -> R {
        let pad = padding;
        let id = self.next_id();
        let gap = if self.first || self.is_horizontal() { 0.0 } else { self.spacing };
        let start = self.cursor + vec2(0.0, gap);
        let max = Rect { min: start + pad, max: vec2(self.max_rect.max.x - pad.x, self.max_rect.max.y) };
        let mut ui = self.sub(id, max, Dir::Vertical);
        let out = f(&mut ui);
        let used = ui.content.size();
        self.allocate(vec2(self.available_width(), used.y + pad.y * 2.0));
        out
    }

    /// Lays out the widgets added in `f` in `rect`, without taking space here.
    pub fn in_rect<R>(&mut self, id: impl Hash, rect: Rect, f: impl FnOnce(&mut Ui) -> R) -> R {
        let r = rect;
        let id = self.id.with(id);
        let mut ui = self.sub(id, r, Dir::Vertical);
        f(&mut ui)
    }

    /// Like [`Self::in_rect`], and also returns the size used.
    pub fn in_rect_sized<R>(&mut self, id: impl Hash, rect: Rect, f: impl FnOnce(&mut Ui) -> R) -> (R, Vec2) {
        let r = rect;
        let id = self.id.with(id);
        let mut ui = self.sub(id, r, Dir::Vertical);
        let out = f(&mut ui);
        (out, ui.content.size())
    }

    /// Lays out the widgets added in `f` side by side in `rect`, centered on its
    /// height.
    pub fn row_in_rect<R>(&mut self, id: impl Hash, rect: Rect, f: impl FnOnce(&mut Ui) -> R) -> R {
        let r = rect;
        let id = self.id.with(id);
        let mut ui = self.sub(id, r, Dir::Horizontal { row: r.height() });
        f(&mut ui)
    }

    pub(crate) fn row_sized<R>(&mut self, id: Id, r: Rect, f: impl FnOnce(&mut Ui) -> R) -> (R, Vec2) {
        let mut ui = self.sub(id, r, Dir::Horizontal { row: r.height() });
        let out = f(&mut ui);
        (out, ui.content.size())
    }

    /// Groups the widgets added in `f` on a rounded, nearly opaque card. Cards
    /// are content, not glass: glass belongs on floating controls.
    pub fn card<R>(&mut self, f: impl FnOnce(&mut Ui) -> R) -> R {
        let id = self.next_id();
        let w = self.available_width();
        let gap = if self.first || self.is_horizontal() { 0.0 } else { self.spacing };
        let start = self.cursor + vec2(0.0, gap);
        let h = self.size_of(id.with("card")).y;
        let r = Rect::from_min_size(start, vec2(w, h));
        let fill =
            if self.surface.darkness() > 0.5 { Color::gray(0.13).alpha(0.82) } else { Color::white(0.80) };
        self.painter().fill(r, 20.0, fill);
        let max = Rect { min: start + vec2(18.0, 16.0), max: vec2(start.x + w - 18.0, self.max_rect.max.y) };
        let mut ui = self.sub(id, max, Dir::Vertical);
        let out = f(&mut ui);
        let used = ui.content.height() + 32.0;
        self.remember(id.with("card"), vec2(w, used));
        self.allocate(vec2(w, used));
        out
    }

    /// Groups the widgets added in `f` on a glass panel that sizes itself to the
    /// content.
    pub fn glass_panel<R>(&mut self, material: Material, f: impl FnOnce(&mut Ui) -> R) -> R {
        let id = self.next_id();
        let w = self.available_width();
        let gap = if self.first || self.is_horizontal() { 0.0 } else { self.spacing };
        let start = self.cursor + vec2(0.0, gap);
        let h = self.size_of(id.with("card")).y;
        let r = Rect::from_min_size(start, vec2(w, h));
        let surface = self.glass(id, r, Corner::Radius(24.0), material, false);
        let max = Rect { min: start + vec2(18.0, 16.0), max: vec2(start.x + w - 18.0, self.max_rect.max.y) };
        let mut ui = self.sub(id, max, Dir::Vertical);
        ui.surface = surface;
        let out = f(&mut ui);
        let used = ui.content.height() + 32.0;
        self.remember(id.with("card"), vec2(w, used));
        self.allocate(vec2(w, used));
        out
    }

    /// Paints an adaptive glass element and returns the surface for content on
    /// it.
    pub fn glass(
        &mut self,
        id: Id,
        rect: Rect,
        corner: impl Into<Corner>,
        material: Material,
        adaptive: bool,
    ) -> Surface {
        let mut g = Glass::new(rect, corner, material);
        let a = self.ctx.prepare_glass(id, &mut g, adaptive);
        self.painter().glass(&g);
        a.surface()
    }

    // ----- text -----

    /// Adds text in the body size.
    pub fn label(&mut self, text: &str) -> Response {
        let c = self.surface.text_color();
        self.styled_text(text, TextStyle::new(self.ctx.theme.body_size), c)
    }
    /// Adds less important text in the body size.
    pub fn secondary(&mut self, text: &str) -> Response {
        let c = self.surface.secondary_color();
        self.styled_text(text, TextStyle::new(self.ctx.theme.body_size), c)
    }
    /// Adds small, less important text.
    pub fn caption(&mut self, text: &str) -> Response {
        let c = self.surface.secondary_color();
        self.styled_text(text, TextStyle::new(self.ctx.theme.caption_size), c)
    }
    /// Adds bold text in the body size.
    pub fn strong(&mut self, text: &str) -> Response {
        let c = self.surface.text_color();
        self.styled_text(text, TextStyle::new(self.ctx.theme.body_size).weight(Weight::Semibold), c)
    }
    /// Adds a large title.
    pub fn heading(&mut self, text: &str) -> Response {
        let c = self.surface.text_color();
        let st = TextStyle::new(self.ctx.theme.title_size).weight(Weight::Bold);
        self.text_role(text, st, c, Role::Heading)
    }
    /// Adds a smaller title.
    pub fn subheading(&mut self, text: &str) -> Response {
        let c = self.surface.text_color();
        self.styled_text(text, TextStyle::new(17.0).weight(Weight::Semibold), c)
    }

    /// Adds text in `style` and `color`.
    pub fn styled_text(&mut self, text: &str, style: TextStyle, color: Color) -> Response {
        self.text_role(text, style, color, Role::Label)
    }

    fn text_role(&mut self, text: &str, mut st: TextStyle, c: Color, role: Role) -> Response {
        if !self.is_horizontal() && st.wrap.is_none() {
            st.wrap = Some(self.max_rect.width().max(10.0));
        }
        let l = self.layout_text(text, &st);
        let r = self.allocate(l.size);
        self.painter().text(r.min, &l, c);
        let id = self.next_id();
        if let Some(n) = self.access(id, role, r) {
            n.label(text);
        }
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

    /// Adds a thin line across the layout direction.
    pub fn separator(&mut self) {
        let c = self.surface.separator_color();
        match self.dir {
            Dir::Vertical => {
                let w = self.available_width();
                let r = self.allocate(vec2(w, 1.0));
                self.painter().fill(r, 0.0, c);
            }
            Dir::Horizontal { row } => {
                let h = row.max(20.0) * 0.6;
                let r = self.allocate(vec2(1.0, h));
                self.painter().fill(r, 0.0, c);
            }
        }
    }

    /// Draws the keyboard focus ring around `rect` while `id` has focus from the
    /// keyboard.
    pub fn focus_ring(&mut self, id: Id, rect: Rect, corner: impl Into<Corner>) {
        if self.ctx.shows_focus(id) {
            let c = self.ctx.theme.accent;
            self.painter().stroke(rect.expand(3.5), corner, 2.5, c.alpha(0.85));
        }
    }
}
