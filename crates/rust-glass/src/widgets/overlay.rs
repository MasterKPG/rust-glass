//! Things that float: popups and menus, tooltips, alerts, sheets, toasts and
//! windows. They materialize by forming from another shape and by ramping their
//! optics, not by fading a flat rectangle.

use crate::access::Role;
use crate::anim::{Spring, SpringRect, Tuning};
use crate::color::Color;
use crate::context::{Context, Cursor, Id, Order, Response, Sense};
use crate::geom::{Rect, Vec2, lerp, smoothstep, vec2};
use crate::icons::Icon;
use crate::input::Key;
use crate::material::{Corner, Glass, GlassShape};
use crate::text::{TextStyle, Weight};
use crate::ui::{Ui, Widget};
use crate::widgets::button::{Button, ButtonKind};
use crate::widgets::gel::{Gel, rubber};

// ----- popup -----

#[derive(Clone, Copy, Default)]
struct PopupState {
    shape: SpringRect,
    t: Spring,
    size: Vec2,
    live: bool,
    anchor: Vec2,
    /// Frame in which it opened, so the opening click does not close it.
    born: u64,
}

/// How far a popup has formed, 0 to 1. The anchor fades out with it.
pub fn popup_progress(ctx: &mut Context, id: Id) -> f32 {
    let s: PopupState = ctx.get(id.with("popup"));
    if s.live { s.t.x.clamp(0.0, 1.0) } else { 0.0 }
}

/// Glass bubble that grows out of `anchor` while [`Context::popup_open`] is set
/// for `id`, and flows back when it closes.
pub fn popup(ctx: &mut Context, id: Id, anchor: Rect, width: f32, f: impl FnOnce(&mut Ui)) {
    let open = ctx.popup_open(id);
    let key = id.with("popup");
    let mut s: PopupState = ctx.get(key);
    if !open && !s.live {
        return;
    }
    if open && !s.live {
        s = PopupState {
            shape: SpringRect::at(anchor),
            t: Spring::at(0.0),
            size: s.size,
            live: true,
            anchor: anchor.min,
            born: ctx.frame,
        };
    }
    // Stay glued to an anchor that scrolls or moves with its window.
    let carried = anchor.min - s.anchor;
    s.anchor = anchor.min;
    s.shape.center.x.x += carried.x;
    s.shape.center.y.x += carried.y;
    let dt = ctx.dt;
    let size = if s.size == Vec2::ZERO { vec2(width, 60.0) } else { s.size };
    let full = place(anchor, size, ctx.screen);
    let target = if open { full } else { anchor };
    let (pt, st) = if open {
        (ctx.tune(Tuning::new(0.36, 0.80)), ctx.tune(Tuning::new(0.40, 0.72)))
    } else {
        (ctx.tune(Tuning::new(0.30, 0.95)), ctx.tune(Tuning::new(0.30, 0.95)))
    };
    let mut moving = s.shape.step(target, pt, st, dt);
    moving |= s.t.step(open as u32 as f32, ctx.tune(Tuning::new(if open { 0.30 } else { 0.26 }, 1.0)), dt);
    if moving {
        ctx.request_repaint();
    } else if !open {
        s.live = false;
        ctx.set(key, s);
        return;
    }
    let t = s.t.x.clamp(0.0, 1.0);
    let shape = s.shape.rect();
    let layer = ctx.layer(key, Order::Popup);
    ctx.interact(key.with("block"), shape, Rect::EVERYTHING, layer, Sense::Hover);

    let m = ctx.theme.regular.lerp(&ctx.theme.thick, t);
    let radius = lerp(anchor.height() * 0.5, 22.0, t).min(shape.height() * 0.5);
    let mut g = Glass::new(shape, Corner::Radius(radius), m).presence(smoothstep(0.0, 0.12, t).max(0.35));
    let a = ctx.prepare_glass(key, &mut g, false);
    ctx.painter(layer, Rect::EVERYTHING).glass(&g);

    // Content is laid out at full size and revealed as the bubble opens.
    let show = smoothstep(0.30, 0.85, t);
    let mut used = size;
    if open || show > 0.01 {
        let clip = shape.shrink(1.0);
        let offset = (shape.center() - full.center()) * 0.6;
        let r = full.translate(offset);
        let mut ui =
            Ui::new(ctx, layer, key, Rect { min: r.min + vec2(8.0, 8.0), max: r.max - vec2(8.0, 8.0) });
        ui.max.max.y = 1.0e5;
        ui.clip = clip;
        ui.opacity = show;
        ui.surface = a.surface();
        ui.spacing = 2.0;
        f(&mut ui);
        used = vec2(width, ui.used().height() + 16.0);
    }
    if (used - s.size).length() > 0.5 {
        s.size = used;
        ctx.request_repaint();
    }
    let young = ctx.frame <= s.born + 1;
    ctx.set(key, s);
    if open && !young {
        let outside = ctx.pressed_outside(shape.union(anchor));
        if outside || ctx.input.take_key(Key::Escape) {
            ctx.set_popup(id, false);
        }
    }
}

/// Below the anchor if it fits, otherwise above, always inside the screen.
fn place(anchor: Rect, size: Vec2, screen: Rect) -> Rect {
    let below = anchor.max.y + 6.0 + size.y <= screen.max.y - 8.0;
    let y = if below || anchor.min.y - size.y < 8.0 { anchor.min.y } else { anchor.max.y - size.y };
    let x = if anchor.center().x > screen.center().x { anchor.max.x - size.x } else { anchor.min.x };
    Rect::from_min_size(vec2(x, y), size).clamp_inside(screen.shrink(8.0))
}

/// Row inside a menu.
pub struct MenuItem<'a> {
    label: &'a str,
    icon: Option<Icon>,
    shortcut: &'a str,
    checked: bool,
    destructive: bool,
}

impl<'a> MenuItem<'a> {
    pub fn new(label: &'a str) -> Self {
        Self { label, icon: None, shortcut: "", checked: false, destructive: false }
    }
    pub fn icon(mut self, i: Icon) -> Self {
        self.icon = Some(i);
        self
    }
    pub fn shortcut(mut self, s: &'a str) -> Self {
        self.shortcut = s;
        self
    }
    pub fn checked(mut self, c: bool) -> Self {
        self.checked = c;
        self
    }
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }
}

impl Widget for MenuItem<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let w = ui.available_width();
        let mut r = ui.alloc_interact(vec2(w, 36.0), Sense::Click);
        ui.ctx.focusable(r.id);
        if let Some(n) = ui.access(r.id, Role::MenuItem, r.rect) {
            n.label(self.label);
            if self.checked {
                n.selected(true);
            }
        }
        // Press on the menu button, slide, release on an item.
        if r.rect.contains(ui.ctx.pointer_or(vec2(-1.0, -1.0)))
            && ui.ctx.input.released
            && ui.ctx.input.press_origin != ui.ctx.pointer_or(Vec2::ZERO)
            && ui.opacity > 0.6
            && (ui.ctx.input.press_origin - ui.ctx.pointer_or(Vec2::ZERO)).length() > 12.0
        {
            r.clicked = true;
        }
        let h = ui.ctx.animate_bool(r.id, r.hovered, Tuning::SNAP);
        let st = TextStyle::new(ui.ctx.theme.body).weight(Weight::Medium);
        let l = ui.layout_text(self.label, &st);
        let s = ui.layout_text(self.shortcut, &TextStyle::new(ui.ctx.theme.caption));
        let c = if self.destructive { Color::RED } else { ui.surface.text() };
        let (fill, sec) = (ui.surface.fill(), ui.surface.secondary());
        if r.hovered {
            ui.ctx.cursor = Cursor::Pointer;
        }
        let mut p = ui.painter();
        p.fill(r.rect, 14.0, fill.fade(h));
        let cy = r.rect.center().y;
        let mut x = r.rect.min.x + 12.0;
        if let Some(i) = self.icon {
            p.icon(vec2(x + 9.0, cy), 18.0, i, c);
            x += 30.0;
        }
        p.text(vec2(x, cy - l.size.y * 0.5), &l, c);
        let mut rx = r.rect.max.x - 12.0;
        if self.checked {
            p.icon(vec2(rx - 8.0, cy), 16.0, Icon::Check, c);
            rx -= 24.0;
        }
        p.text(vec2(rx - s.size.x, cy - s.size.y * 0.5), &s, sec);
        ui.focus_ring(r.id, r.rect, 14.0);
        r
    }
}

/// Button whose surface turns into the menu.
pub struct MenuButton<'a> {
    label: &'a str,
    icon: Option<Icon>,
    chevron: bool,
    width: f32,
    min_width: f32,
}

impl<'a> MenuButton<'a> {
    pub fn new(label: &'a str) -> Self {
        Self { label, icon: None, chevron: false, width: 230.0, min_width: 0.0 }
    }
    pub fn icon(mut self, i: Icon) -> Self {
        self.icon = Some(i);
        self
    }
    pub fn chevron(mut self) -> Self {
        self.chevron = true;
        self
    }
    /// Width of the menu.
    pub fn menu_width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }
    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = w;
        self
    }

    pub fn show(self, ui: &mut Ui, f: impl FnOnce(&mut Ui)) -> Response {
        let st = TextStyle::new(ui.ctx.theme.body).weight(Weight::Semibold);
        let l = ui.layout_text(self.label, &st);
        let h = ui.ctx.theme.control_height;
        let has_text = !self.label.is_empty();
        let iw = if self.icon.is_some() { 17.0 } else { 0.0 };
        let gap = if has_text && self.icon.is_some() { 7.0 } else { 0.0 };
        let cw = if self.chevron { 18.0 } else { 0.0 };
        let w = if has_text { (l.size.x + iw + gap + cw + 30.0).max(self.min_width) } else { h };
        let r = ui.alloc_interact(vec2(w, h), Sense::Click);
        ui.ctx.focusable(r.id);
        if r.hovered {
            ui.ctx.cursor = Cursor::Pointer;
        }
        if r.clicked && !ui.ctx.popup_open(r.id) {
            ui.ctx.set_popup(r.id, true);
        }
        let open = ui.ctx.popup_open(r.id);
        let role = if self.chevron { Role::ComboBox } else { Role::Button };
        if let Some(n) = ui.access(r.id, role, r.rect) {
            n.label(if has_text { self.label } else { self.icon.map_or("Menu", Icon::name) }).expanded(open);
        }
        let t = popup_progress(ui.ctx, r.id);
        let gel = Gel::update(ui.ctx, &r, 1.0);
        let fade = 1.0 - smoothstep(0.0, 0.35, t);
        let mut s = ui.surface;
        if ui.surface.is_glass() {
            let c = s.fill();
            ui.painter().fill(gel.rect, Corner::Capsule, c.fade(fade * (1.0 + 0.5 * gel.hover)));
        } else {
            let mut g = Glass::new(gel.rect, Corner::Capsule, ui.ctx.theme.regular);
            s = ui.ctx.prepare_glass(r.id, &mut g, true).surface();
            gel.apply(&mut g);
            g.presence = fade;
            ui.painter().glass(&g);
        }
        let c = s.text().fade(fade);
        let content = l.size.x + iw + gap + cw;
        let k = gel.scale;
        let mut x = if self.min_width > 0.0 && has_text {
            gel.rect.min.x + 15.0
        } else {
            gel.rect.center().x - content * k * 0.5
        };
        let cy = gel.rect.center().y;
        let sec = s.secondary().fade(fade);
        let mut p = ui.painter();
        if let Some(i) = self.icon {
            p.icon(vec2(x + iw * k * 0.5, cy), 17.0 * k, i, c);
            x += (iw + gap) * k;
        }
        if has_text {
            p.text_centered(Rect::from_center_size(vec2(x + l.size.x * k * 0.5, cy), l.size), &l, c, k);
        }
        if self.chevron {
            p.icon(vec2(gel.rect.max.x - 18.0, cy), 15.0, Icon::ChevronsUpDown, sec);
        }
        ui.focus_ring(r.id, gel.rect, Corner::Capsule);
        let anchor = gel.rect;
        let width = self.width.max(anchor.width());
        popup(ui.ctx, r.id, anchor, width, f);
        r
    }
}

/// Drop down list.
pub struct ComboBox<'a> {
    pub selected: &'a mut usize,
    pub options: &'a [&'a str],
    pub width: f32,
}

impl ComboBox<'_> {
    pub fn show(self, ui: &mut Ui) -> bool {
        let now = (*self.selected).min(self.options.len().saturating_sub(1));
        let label = self.options.get(now).copied().unwrap_or("");
        let mut pick = None;
        let r =
            MenuButton::new(label).chevron().min_width(self.width).menu_width(self.width).show(ui, |ui| {
                for (i, o) in self.options.iter().enumerate() {
                    if ui.add(MenuItem::new(o).checked(i == now)).clicked {
                        pick = Some(i);
                    }
                }
            });
        if let Some(i) = pick {
            ui.ctx.set_popup(r.id, false);
            *self.selected = i;
            return i != now;
        }
        false
    }
}

/// Menu on secondary click, forming at the pointer.
pub fn context_menu(ctx: &mut Context, target: &Response, width: f32, f: impl FnOnce(&mut Ui)) {
    let id = target.id.with("ctx");
    if target.secondary_clicked {
        let p = ctx.pointer_or(target.rect.center());
        ctx.set(id, p);
        ctx.set_popup(id, true);
    }
    let at: Vec2 = ctx.get(id);
    popup(ctx, id, Rect::from_center_size(at, Vec2::splat(18.0)), width, f);
}

// ----- tooltip -----

pub fn tooltip(ctx: &mut Context, id: Id, anchor: Rect, text: &str) {
    if text.is_empty() {
        return;
    }
    let key = id.with("tooltip");
    #[derive(Clone, Copy, Default)]
    struct Since {
        t: f64,
        frame: u64,
    }
    let mut s: Since = ctx.get(key);
    if s.frame + 1 < ctx.frame {
        s.t = ctx.time;
    }
    s.frame = ctx.frame;
    ctx.set(key, s);
    let wait = 0.45;
    if ctx.input.down {
        return;
    }
    let t = ((ctx.time - s.t - wait) / 0.16).clamp(0.0, 1.0) as f32;
    if t < 1.0 {
        ctx.request_repaint();
    }
    if t <= 0.0 {
        return;
    }
    let l = ctx.text.layout(text, &TextStyle::new(12.0).weight(Weight::Medium));
    let size = l.size + vec2(22.0, 13.0);
    let mut c = vec2(anchor.center().x, anchor.min.y - 8.0 - size.y * 0.5);
    if c.y - size.y * 0.5 < 6.0 {
        c.y = anchor.max.y + 8.0 + size.y * 0.5;
    }
    let r = Rect::from_center_size(c, size).clamp_inside(ctx.screen.shrink(6.0));
    let e = smoothstep(0.0, 1.0, t);
    let r = r.scale_from_center(0.9 + 0.1 * e, 0.9 + 0.1 * e);
    let layer = ctx.layer(Id::new("tooltip"), Order::Tooltip);
    let mut g = Glass::new(r, Corner::Capsule, ctx.theme.thick).presence(e);
    g.material.shadow_radius = 14.0;
    g.material.shadow_y = 5.0;
    let a = ctx.prepare_glass(key, &mut g, false);
    let mut p = ctx.painter(layer, Rect::EVERYTHING);
    p.glass(&g);
    p.text_centered(r, &l, a.surface().text().fade(e), 1.0);
}

impl Response {
    /// Show a tooltip while hovered.
    pub fn tip(self, ui: &mut Ui, text: &str) -> Response {
        if self.hovered {
            tooltip(ui.ctx, self.id, self.rect, text);
        }
        self
    }
}

// ----- modal -----

/// Alert in the middle of the window. Returns the index of the clicked button.
/// The last button is the primary one.
pub fn alert(
    ctx: &mut Context,
    id: Id,
    open: &mut bool,
    title: &str,
    message: &str,
    buttons: &[&str],
) -> Option<usize> {
    let mut out = None;
    let w = 300.0;
    modal(ctx, id, open, w, |ui| {
        ui.align = crate::ui::Align::Center;
        ui.subheading(title);
        let c = ui.surface.secondary();
        let st = TextStyle::new(ui.ctx.theme.body).align(crate::text::TextAlign::Center);
        ui.text_with(message, st, c);
        ui.space(8.0);
        let n = buttons.len().max(1);
        let gap = 10.0;
        let bw = (ui.available_width() - gap * (n as f32 - 1.0)) / n as f32;
        ui.spacing = gap;
        ui.horizontal(|ui| {
            for (i, b) in buttons.iter().enumerate() {
                let kind = if i + 1 == n { ButtonKind::Prominent } else { ButtonKind::Glass };
                if ui.add(Button::new(b).kind(kind).width(bw)).clicked {
                    out = Some(i);
                }
            }
        });
    });
    if out.is_some() {
        *open = false;
    }
    out
}

/// Centered glass panel over a dimmed window.
pub fn modal(ctx: &mut Context, id: Id, open: &mut bool, width: f32, f: impl FnOnce(&mut Ui)) {
    #[derive(Clone, Copy, Default)]
    struct St {
        t: Spring,
        h: f32,
        init: bool,
    }
    let mut s: St = ctx.get(id);
    if !s.init {
        s.t = Spring::at(0.0);
        s.init = true;
    }
    let tune = if *open { Tuning::new(0.42, 0.72) } else { Tuning::new(0.28, 1.0) };
    if s.t.step(*open as u32 as f32, ctx.tune(tune), ctx.dt) {
        ctx.request_repaint();
    }
    let t = s.t.x;
    if t < 0.004 && !*open {
        ctx.set(id, s);
        return;
    }
    let k = t.clamp(0.0, 1.0);
    let layer = ctx.layer(id, Order::Modal);
    let screen = ctx.screen;
    ctx.interact(id.with("dim"), screen, Rect::EVERYTHING, layer, Sense::Click);
    let h = s.h.max(80.0);
    let scale = lerp(1.16, 1.0, t);
    let r = Rect::from_center_size(screen.center(), vec2(width, h) * scale);
    let mut g = Glass::new(r, Corner::Radius(30.0), ctx.theme.thick).presence(k);
    g.material.blur *= 1.4;
    let a = ctx.prepare_glass(id, &mut g, false);
    let mut p = ctx.painter(layer, screen.expand(64.0));
    p.fill(screen.expand(64.0), 0.0, Color::black(0.28 * k));
    p.glass(&g);
    let inner = Rect::from_center_size(screen.center(), vec2(width, h)).shrink(20.0);
    let mut ui = Ui::new(ctx, layer, id, inner);
    ui.max.max.y = 1.0e5;
    ui.opacity = smoothstep(0.35, 1.0, k);
    ui.surface = a.surface();
    ui.enabled = *open;
    f(&mut ui);
    let used = ui.used().height() + 40.0;
    if (used - s.h).abs() > 0.5 {
        s.h = used;
        ctx.request_relayout();
    }
    ctx.set(id, s);
    if *open && ctx.input.take_key(Key::Escape) {
        *open = false;
    }
}

/// Panel that rises from the bottom edge and can be flicked away.
pub fn sheet(ctx: &mut Context, id: Id, open: &mut bool, width: f32, height: f32, f: impl FnOnce(&mut Ui)) {
    #[derive(Clone, Copy, Default)]
    struct St {
        y: Spring,
        drag: f32,
        init: bool,
    }
    let mut s: St = ctx.get(id);
    if !s.init {
        s.y = Spring::at(1.0);
        s.init = true;
    }
    let screen = ctx.screen;
    let travel = height + 40.0;
    let layer = ctx.layer(id, Order::Modal);
    let target = if *open { s.drag / travel } else { 1.0 };
    let tune = if s.drag != 0.0 { Tuning::new(0.08, 1.0) } else { Tuning::new(0.42, 0.82) };
    if s.y.step(target, ctx.tune(tune), ctx.dt) {
        ctx.request_repaint();
    }
    if s.y.x > 0.998 && !*open {
        s.drag = 0.0;
        ctx.set(id, s);
        return;
    }
    let k = (1.0 - s.y.x).clamp(0.0, 1.0);
    let top = screen.max.y - height - 14.0 + s.y.x * travel;
    let r = Rect::from_min_size(vec2(screen.center().x - width * 0.5, top), vec2(width, height));
    let dim = ctx.interact(id.with("dim"), screen, Rect::EVERYTHING, layer, Sense::Click);
    ctx.interact(id.with("block"), r, Rect::EVERYTHING, layer, Sense::Hover);
    let grip = Rect::from_min_size(r.min, vec2(width, 34.0));
    let gr = ctx.interact(id.with("grip"), grip, Rect::EVERYTHING, layer, Sense::Drag);
    if gr.held {
        let d = ctx.pointer_or(Vec2::ZERO).y - ctx.input.press_origin.y;
        s.drag = if d < 0.0 { rubber(d, 24.0) } else { d };
    }
    if gr.released {
        if s.drag > height * 0.28 {
            *open = false;
        }
        s.drag = 0.0;
    }
    if dim.clicked {
        *open = false;
    }
    // The sheet stretches a little when pulled up.
    let stretch = (-s.y.x * travel).max(0.0);
    let body = Rect { min: r.min, max: vec2(r.max.x, r.max.y + stretch) };
    let mut g = Glass::new(body, Corner::Radius(34.0), ctx.theme.thick).presence(k.max(0.4));
    let a = ctx.prepare_glass(id, &mut g, false);
    let mut p = ctx.painter(layer, screen.expand(64.0));
    p.fill(screen.expand(64.0), 0.0, Color::black(0.22 * k));
    p.glass(&g);
    let c = a.surface().tertiary();
    p.fill(Rect::from_center_size(vec2(r.center().x, r.min.y + 12.0), vec2(40.0, 5.0)), Corner::Capsule, c);
    let inner = Rect { min: r.min + vec2(24.0, 34.0), max: r.max - vec2(24.0, 20.0) };
    let mut ui = Ui::new(ctx, layer, id, inner);
    ui.surface = a.surface();
    ui.clip = body;
    f(&mut ui);
    ctx.set(id, s);
    if *open && ctx.input.take_key(Key::Escape) {
        *open = false;
    }
}

// ----- toast -----

#[derive(Clone, Debug)]
pub struct ToastState {
    icon: Option<Icon>,
    text: String,
    born: Option<f64>,
    t: Spring,
    y: Spring,
    key: u64,
}

impl ToastState {
    pub fn new(icon: Option<Icon>, text: String) -> Self {
        Self { icon, text, born: None, t: Spring::at(0.0), y: Spring::default(), key: 0 }
    }
}

pub(crate) fn show_toasts(ctx: &mut Context) {
    let mut list = ctx.take_toasts();
    if list.is_empty() {
        return;
    }
    let layer = ctx.layer(Id::new("toasts"), Order::Toast);
    let now = ctx.time;
    let mut y = 24.0;
    let n = list.len();
    let mut shapes = Vec::new();
    let mut faces = Vec::new();
    for (i, t) in list.iter_mut().enumerate() {
        if t.born.is_none() {
            t.born = Some(now);
            t.key = (now * 1000.0) as u64 + i as u64;
        }
        let age = now - t.born.unwrap_or(now);
        let show = age < 2.6 && i + 4 > n;
        let tune = ctx.tune(if show { Tuning::new(0.46, 0.70) } else { Tuning::new(0.30, 1.0) });
        t.t.step(show as u32 as f32, tune, ctx.dt);
        t.y.step(y, ctx.tune(Tuning::SMOOTH), ctx.dt);
        let l = ctx.text.layout(&t.text, &TextStyle::new(14.0).weight(Weight::Semibold));
        let iw = if t.icon.is_some() { 28.0 } else { 0.0 };
        let full = vec2(l.size.x + iw + 40.0, 46.0);
        // Starts as a small drop at the top edge.
        let size = vec2(lerp(46.0, full.x, t.t.x), lerp(34.0, full.y, t.t.x.min(1.1)));
        let cy = lerp(-30.0, t.y.x + 23.0, t.t.x);
        let r = Rect::from_center_size(vec2(ctx.screen.center().x, cy), size.max(Vec2::splat(8.0)));
        shapes.push(GlassShape::new(r, Corner::Capsule));
        faces.push((r, l, t.icon, t.t.x, iw));
        if show {
            y += 54.0;
        }
    }
    list.retain(|t| t.born.is_some_and(|b| now - b < 2.6) || t.t.x > 0.01);
    ctx.request_repaint();
    let mut g = Glass::group(shapes, ctx.theme.thick).fuse(5.0);
    g.material.shadow_radius = 22.0;
    g.material.blur = 6.0;
    let a = ctx.prepare_glass(Id::new("toasts"), &mut g, false);
    let c = a.surface().text();
    let mut p = ctx.painter(layer, Rect::EVERYTHING);
    p.glass(&g);
    for (r, l, icon, t, iw) in faces {
        let k = smoothstep(0.45, 0.95, t);
        let x = r.center().x - (l.size.x + iw) * 0.5;
        if let Some(i) = icon {
            p.icon(vec2(x + 9.0, r.center().y), 19.0, i, c.fade(k));
        }
        p.text(vec2(x + iw, r.center().y - l.size.y * 0.5), &l, c.fade(k));
    }
    ctx.put_toasts(list);
}

// ----- window -----

/// Draggable, collapsible glass window.
pub struct Window<'a> {
    title: &'a str,
    pos: Vec2,
    width: f32,
    open: Option<&'a mut bool>,
}

impl<'a> Window<'a> {
    pub fn new(title: &'a str) -> Self {
        Self { title, pos: vec2(80.0, 80.0), width: 280.0, open: None }
    }
    pub fn pos(mut self, p: Vec2) -> Self {
        self.pos = p;
        self
    }
    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }
    pub fn open(mut self, o: &'a mut bool) -> Self {
        self.open = Some(o);
        self
    }

    pub fn show(self, ctx: &mut Context, f: impl FnOnce(&mut Ui)) {
        #[derive(Clone, Copy)]
        struct St {
            pos: Vec2,
            grab: Vec2,
            h: f32,
            t: Spring,
            fold: Spring,
            folded: bool,
        }
        let id = Id::new(("window", self.title));
        let mut s: St = ctx.try_get(id).unwrap_or(St {
            pos: self.pos,
            grab: Vec2::ZERO,
            h: 0.0,
            t: Spring::at(0.0),
            fold: Spring::at(0.0),
            folded: false,
        });
        let open = self.open.as_deref().copied().unwrap_or(true);
        let tune = if open { Tuning::new(0.44, 0.70) } else { Tuning::new(0.26, 1.0) };
        if s.t.step(open as u32 as f32, ctx.tune(tune), ctx.dt) {
            ctx.request_repaint();
        }
        if s.t.x < 0.005 && !open {
            ctx.set(id, s);
            return;
        }
        let layer = ctx.layer(id, Order::Window);
        let bar_h = 46.0;
        if s.fold.step(s.folded as u32 as f32, ctx.tune(Tuning::new(0.40, 0.80)), ctx.dt) {
            ctx.request_repaint();
        }
        let full_h = s.h.max(bar_h);
        let h = lerp(full_h, bar_h, s.fold.x);
        let k = s.t.x;
        let mut r = Rect::from_min_size(s.pos, vec2(self.width, h));
        let bar = Rect::from_min_size(r.min, vec2(self.width, bar_h));
        ctx.interact(id.with("block"), r, Rect::EVERYTHING, layer, Sense::Click);
        let drag = ctx.interact(id.with("bar"), bar, Rect::EVERYTHING, layer, Sense::Drag);
        if drag.held
            && let Some(p) = drag.pointer
        {
            if drag.pressed {
                s.grab = p - s.pos;
            }
            s.pos = p - s.grab;
        }
        if drag.double_clicked {
            s.folded = !s.folded;
        }
        s.pos = Rect::from_min_size(s.pos, vec2(self.width, bar_h)).clamp_inside(ctx.screen).min;
        // The window is rigid, everything in it moves as one. Lifting it only
        // deepens its shadow.
        let held = ctx.animate_bool(id.with("held"), drag.held, Tuning::SMOOTH).max(0.0);
        r = Rect::from_min_size(s.pos, r.size()).snap(ctx.ppp);
        let body = if (k - 1.0).abs() < 0.001 {
            r
        } else {
            r.scale_from_center(lerp(0.82, 1.0, k), lerp(0.82, 1.0, k))
        };
        let mut g = Glass::new(body, Corner::Radius(26.0), ctx.theme.thick).presence(k);
        g.material.shadow *= 1.0 + 0.35 * held;
        g.material.shadow_y *= 1.0 + 0.3 * held;
        let a = ctx.prepare_glass(id, &mut g, false);
        let op = smoothstep(0.3, 0.9, k);
        let title = ctx.text.layout(self.title, &TextStyle::new(14.0).weight(Weight::Semibold));
        let surf = a.surface();
        let mut p = ctx.painter(layer, Rect::EVERYTHING);
        p.glass(&g);
        p.opacity = op;
        p.text_centered(Rect::from_min_size(r.min, vec2(self.width, bar_h)), &title, surf.text(), 1.0);

        // Close and collapse buttons.
        let mk = |x: f32| Rect::from_center_size(vec2(x, r.min.y + bar_h * 0.5), Vec2::splat(24.0));
        let mut close = false;
        let mut spots = vec![(mk(r.max.x - 24.0), 1u8)];
        if self.open.is_some() {
            spots.push((mk(r.min.x + 24.0), 0u8));
        }
        for (cr, what) in spots {
            let res = ctx.interact(id.with(("btn", what)), cr, Rect::EVERYTHING, layer, Sense::Click);
            let gel = Gel::update(ctx, &res, 1.0);
            let mut p = ctx.painter(layer, Rect::EVERYTHING);
            p.opacity = op;
            p.fill(gel.rect, Corner::Capsule, surf.fill().fade(1.0 + gel.hover));
            if what == 0 {
                p.icon(gel.rect.center(), 13.0 * gel.scale, Icon::Close, surf.text());
                close |= res.clicked;
            } else {
                let i = if s.folded { Icon::ChevronDown } else { Icon::ChevronUp };
                p.icon(gel.rect.center(), 13.0 * gel.scale, i, surf.text());
                if res.clicked {
                    s.folded = !s.folded;
                }
            }
        }
        if s.fold.x < 0.995 {
            let inner = Rect { min: vec2(r.min.x + 18.0, r.min.y + bar_h), max: vec2(r.max.x - 18.0, 1.0e5) };
            let mut ui = Ui::new(ctx, layer, id, inner);
            ui.clip = Rect { min: body.min, max: vec2(body.max.x, r.min.y + h) };
            ui.surface = surf;
            ui.opacity = op * (1.0 - s.fold.x).clamp(0.0, 1.0);
            ui.enabled = open;
            f(&mut ui);
            let used = ui.used().height() + bar_h + 18.0;
            if (used - s.h).abs() > 0.5 {
                s.h = used;
                ctx.request_relayout();
            }
        }
        ctx.set(id, s);
        if close && let Some(o) = self.open {
            *o = false;
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Event;

    #[test]
    fn controls_stay_put_inside_a_dragged_window() {
        let mut ctx = Context::new();
        let (mut value, mut on, mut seg) = (0.4, true, 1);
        let mut offsets: Vec<Vec<f32>> = Vec::new();
        let mut frame = |ctx: &mut Context, i: usize, ev: &[Event]| {
            ctx.begin_frame(vec2(900.0, 700.0), 1.0, i as f64 / 60.0, ev);
            Window::new("W").pos(vec2(100.0, 100.0)).width(300.0).show(ctx, |ui| {
                ui.slider(&mut value, 0.0..=1.0);
                ui.toggle(&mut on, "Toggle");
                ui.segmented(&mut seg, &["One", "Two", "Three"]);
            });
            ctx.end_frame();
            let d = &ctx.paint.data;
            let x = |g: usize| d.gshapes[d.glasses[g].shape_start as usize].center[0];
            (0..d.glasses.len()).map(|g| x(g) - x(0)).collect::<Vec<f32>>()
        };
        for i in 0..120 {
            frame(&mut ctx, i, &[]);
        }
        frame(&mut ctx, 120, &[Event::PointerMoved(vec2(200.0, 120.0))]);
        frame(&mut ctx, 121, &[Event::PointerDown]);
        for i in 0..40 {
            // Fast, changing direction.
            let x = 200.0 + 260.0 * (i as f32 * 0.35).sin().abs();
            let y = 120.0 + i as f32 * 6.0;
            offsets.push(frame(&mut ctx, 122 + i, &[Event::PointerMoved(vec2(x, y))]));
        }
        assert_eq!(offsets[0].len(), 4, "window, slider, toggle and segmented");
        for o in &offsets {
            for (a, b) in o.iter().zip(&offsets[0]) {
                assert!((a - b).abs() <= 1.01, "{o:?} moved against {:?}", offsets[0]);
            }
        }
    }
}
