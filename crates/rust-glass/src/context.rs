//! Per window state: input routing, animation memory, layers and backdrop
//! measurements.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use crate::anim::{Spring, Spring2, Tuning};
use crate::color::Color;
use crate::geom::{Rect, Vec2, vec2};
use crate::input::{Event, Input, Key};
use crate::material::Glass;
use crate::paint::{ImageId, Paint, Painter};
use crate::text::{TextEngine, TextStyle, Weight};
use crate::theme::{Surface, Theme};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(pub u64);

impl Id {
    pub fn new(v: impl Hash) -> Self {
        let mut h = std::hash::DefaultHasher::new();
        v.hash(&mut h);
        Id(h.finish())
    }
    pub fn with(self, v: impl Hash) -> Self {
        let mut h = std::hash::DefaultHasher::new();
        self.0.hash(&mut h);
        v.hash(&mut h);
        Id(h.finish())
    }
}

/// Stacking order of layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Order {
    Background,
    Content,
    /// Floating bars: toolbars, tab bars, sidebars.
    Bar,
    Window,
    Modal,
    Popup,
    Toast,
    Tooltip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sense {
    /// Blocks widgets below, reacts to hover only.
    Hover,
    Click,
    Drag,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Cursor {
    #[default]
    Default,
    Pointer,
    Text,
    Grab,
    ResizeH,
    NotAllowed,
}

#[derive(Clone, Copy, Debug)]
pub struct Response {
    pub id: Id,
    pub rect: Rect,
    pub hovered: bool,
    /// Pointer is down and started on this widget.
    pub held: bool,
    pub pressed: bool,
    pub released: bool,
    pub clicked: bool,
    pub double_clicked: bool,
    pub secondary_clicked: bool,
    pub focused: bool,
    pub changed: bool,
    pub delta: Vec2,
    pub pointer: Option<Vec2>,
}

impl Response {
    pub fn clicked(&self) -> bool {
        self.clicked
    }
    pub fn changed(&self) -> bool {
        self.changed
    }
    pub fn hovered(&self) -> bool {
        self.hovered
    }
}

#[derive(Clone, Copy)]
struct Hit {
    id: Id,
    rect: Rect,
    layer: usize,
    sense: Sense,
    scroll: bool,
}

struct Layer {
    id: Id,
    order: Order,
    z: u64,
    used: u64,
}

/// What is behind a glass element, smoothed.
#[derive(Clone, Copy, Debug, Default)]
struct Adapt {
    mean: f32,
    dev: f32,
    known: bool,
    is_dark: bool,
    dark: Spring,
    boost: Spring,
    used: u64,
}

/// Result of [`Context::adapt`].
#[derive(Clone, Copy, Debug)]
pub struct Adapted {
    /// 0 light glass with dark labels, 1 dark glass with light labels.
    pub dark: f32,
    /// Extra shadow over busy content.
    pub boost: f32,
}

impl Adapted {
    pub fn surface(&self) -> Surface {
        Surface::Glass { dark: self.dark }
    }
}

/// Requests from the UI to the host window.
#[derive(Clone, Debug)]
pub enum HostCmd {
    Copy(String),
    OpenUrl(String),
    Close,
}

pub struct ImageUpload {
    pub id: ImageId,
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

pub struct Context {
    pub input: Input,
    pub theme: Theme,
    pub paint: Paint,
    pub text: TextEngine,
    pub time: f64,
    pub dt: f32,
    pub frame: u64,
    pub screen: Rect,
    pub ppp: f32,
    pub clear: Color,
    pub cursor: Cursor,
    pub host: Vec<HostCmd>,
    pub uploads: Vec<ImageUpload>,
    next_image: u32,
    states: HashMap<(Id, TypeId), (Box<dyn Any>, u64)>,
    layers: Vec<Layer>,
    z_top: u64,
    hits: Vec<Hit>,
    prev_hits: Vec<Hit>,
    pub(crate) hovered: Option<Id>,
    scroll_target: Option<Id>,
    pub(crate) active: Option<Id>,
    pub(crate) focus: Option<Id>,
    focus_by_key: bool,
    focus_list: Vec<Id>,
    focus_seen: bool,
    adapt: HashMap<u64, Adapt>,
    repaint: bool,
    relayout: bool,
    light: Spring,
    tilt: Vec2,
    glow: Spring,
    glow_pos: Vec2,
    scroll_energy: f32,
    toasts: Vec<crate::widgets::overlay::ToastState>,
    open_popups: HashMap<Id, bool>,
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    pub fn new() -> Self {
        Self {
            input: Input::new(),
            theme: Theme::default(),
            paint: Paint::default(),
            text: TextEngine::new(),
            time: 0.0,
            dt: 0.0,
            frame: 0,
            screen: Rect::default(),
            ppp: 1.0,
            clear: Color::gray(0.85),
            cursor: Cursor::Default,
            host: Vec::new(),
            uploads: Vec::new(),
            next_image: 1,
            states: HashMap::new(),
            layers: Vec::new(),
            z_top: 0,
            hits: Vec::new(),
            prev_hits: Vec::new(),
            hovered: None,
            scroll_target: None,
            active: None,
            focus: None,
            focus_by_key: false,
            focus_list: Vec::new(),
            focus_seen: false,
            adapt: HashMap::new(),
            repaint: false,
            relayout: false,
            light: Spring::default(),
            tilt: Vec2::ZERO,
            glow: Spring::default(),
            glow_pos: Vec2::ZERO,
            scroll_energy: 0.0,
            toasts: Vec::new(),
            open_popups: HashMap::new(),
        }
    }

    // ----- frame -----

    pub fn begin_frame(&mut self, size: Vec2, ppp: f32, time: f64, events: &[Event]) {
        // A long idle gap must not turn into one huge animation step.
        self.dt = ((time - self.time) as f32).clamp(0.0, 1.0 / 24.0);
        self.time = time;
        self.frame += 1;
        self.ppp = ppp;
        self.screen = Rect::from_min_size(Vec2::ZERO, size);
        self.cursor = Cursor::Default;
        self.repaint = false;
        self.relayout = false;
        self.input.begin(events, time);
        self.paint.begin(ppp);
        self.paint.frost = self.theme.frost;
        self.paint.reduce_transparency = self.theme.reduce_transparency;
        self.paint.high_contrast = self.theme.high_contrast;
        self.text.begin_frame(ppp);

        std::mem::swap(&mut self.hits, &mut self.prev_hits);
        self.hits.clear();
        self.hovered = None;
        self.scroll_target = None;
        if let Some(p) = self.input.pointer {
            let key = |h: &Hit, i: usize| {
                let l = &self.layers[h.layer];
                (l.order, l.z, i)
            };
            let mut best: Option<((Order, u64, usize), Hit)> = None;
            let mut best_scroll: Option<((Order, u64, usize), Hit)> = None;
            for (i, h) in self.prev_hits.iter().enumerate() {
                if !h.rect.contains(p) {
                    continue;
                }
                let k = key(h, i);
                let slot = if h.scroll { &mut best_scroll } else { &mut best };
                if slot.as_ref().is_none_or(|b| k > b.0) {
                    *slot = Some((k, *h));
                }
            }
            if let Some((k, h)) = best {
                self.hovered = Some(h.id);
                if self.input.pressed {
                    if h.sense != Sense::Hover {
                        self.active = Some(h.id);
                    }
                    // Raise the clicked window.
                    self.z_top += 1;
                    let z = self.z_top;
                    self.layers[h.layer].z = z;
                }
                // A scroll area below an opaque layer does not scroll.
                if let Some((ks, s)) = best_scroll
                    && (ks.0, ks.1) >= (k.0, k.1)
                {
                    self.scroll_target = Some(s.id);
                }
            } else if let Some((_, s)) = best_scroll {
                self.scroll_target = Some(s.id);
            }
        }
        if self.input.pressed {
            self.focus_by_key = false;
            if self.hovered != self.focus {
                self.focus = None;
            }
        }
        if self.input.take_key(Key::Tab) {
            self.move_focus(if self.input.modifiers.shift { -1 } else { 1 });
        }
        self.focus_list.clear();
        self.focus_seen = false;

        // The virtual light turns a little with the pointer, the stand in for
        // device tilt on a desktop. Phones feed real tilt through `set_tilt`.
        let mut target = self.tilt.x;
        if let Some(p) = self.input.pointer
            && self.tilt == Vec2::ZERO
        {
            target = (p.x / size.x.max(1.0) - 0.5) * 0.55;
        }
        target += self.scroll_energy * 0.35;
        self.scroll_energy *= (-self.dt * 9.0).exp();
        if self.theme.reduce_motion {
            target = 0.0;
        }
        if self.light.step(target, Tuning::new(0.55, 1.0), self.dt) {
            self.repaint = true;
        }
        if let Some(p) = self.input.pointer {
            self.glow_pos = p;
        }
        let g = if self.input.down && self.active.is_some() { 1.0 } else { 0.0 };
        if self.glow.step(g, Tuning::new(if g > 0.0 { 0.16 } else { 0.5 }, 1.0), self.dt) {
            self.repaint = true;
        }
    }

    pub fn end_frame(&mut self) {
        crate::widgets::overlay::show_toasts(self);
        if self.input.released || !self.input.down {
            self.active = None;
        }
        if self.focus.is_some() && !self.focus_seen {
            self.focus = None;
        }
        // Forget the state of widgets that have not been drawn for a while.
        if self.frame.is_multiple_of(600) {
            let f = self.frame;
            self.states.retain(|_, v| f - v.1 < 1200);
            self.adapt.retain(|_, a| f - a.used < 1200);
        }
        let mut order: Vec<usize> =
            (0..self.layers.len()).filter(|&i| self.layers[i].used == self.frame).collect();
        order.sort_by_key(|&i| (self.layers[i].order, self.layers[i].z, i));
        self.paint.flatten(&order);
        let a = self.light.x;
        let d = &mut self.paint.data;
        d.clear = self.clear.to_array();
        d.light = [a.sin(), -a.cos()];
        d.pointer = [self.glow_pos.x * self.ppp, self.glow_pos.y * self.ppp];
        d.pointer_glow = self.glow.x.clamp(0.0, 1.0) * 0.16;
        d.time = self.time as f32;
        d.ppp = self.ppp;
        if self.input.pending() || self.text.needs_rebuild() {
            self.repaint = true;
        }
    }

    /// Keep animating: draw another frame as soon as possible.
    pub fn request_repaint(&mut self) {
        self.repaint = true;
    }
    pub fn wants_repaint(&self) -> bool {
        self.repaint
    }
    /// Sizes used for layout were guessed. The frame is run again before it is
    /// shown.
    pub fn request_relayout(&mut self) {
        self.relayout = true;
    }
    pub fn wants_relayout(&self) -> bool {
        self.relayout
    }

    /// Device tilt in radians. Replaces the pointer driven light.
    pub fn set_tilt(&mut self, t: Vec2) {
        self.tilt = t;
    }
    pub fn add_scroll_energy(&mut self, v: f32) {
        self.scroll_energy = (self.scroll_energy + v).clamp(-1.0, 1.0);
    }

    // ----- memory -----

    /// Copy of the state stored for a widget.
    pub fn get<T: Clone + Default + 'static>(&mut self, id: Id) -> T {
        let f = self.frame;
        match self.states.get_mut(&(id, TypeId::of::<T>())) {
            Some((b, used)) => {
                *used = f;
                b.downcast_ref::<T>().cloned().unwrap_or_default()
            }
            None => T::default(),
        }
    }
    pub fn try_get<T: Clone + 'static>(&mut self, id: Id) -> Option<T> {
        let f = self.frame;
        let (b, used) = self.states.get_mut(&(id, TypeId::of::<T>()))?;
        *used = f;
        b.downcast_ref::<T>().cloned()
    }
    pub fn set<T: 'static>(&mut self, id: Id, v: T) {
        self.states.insert((id, TypeId::of::<T>()), (Box::new(v), self.frame));
    }

    /// Spring driven value stored under `id`.
    pub fn animate(&mut self, id: Id, target: f32, t: Tuning) -> f32 {
        let mut s: Spring = self.get(id);
        if s.step(target, self.tune(t), self.dt) {
            self.repaint = true;
        }
        self.set(id, s);
        s.x
    }
    pub fn animate_bool(&mut self, id: Id, on: bool, t: Tuning) -> f32 {
        self.animate(id, on as u32 as f32, t)
    }
    pub fn animate2(&mut self, id: Id, target: Vec2, t: Tuning) -> Spring2 {
        let mut s: Spring2 = self.get(id);
        if s.step(target, self.tune(t), self.dt) {
            self.repaint = true;
        }
        self.set(id, s);
        s
    }

    /// Remove the elastic part when motion is reduced.
    pub fn tune(&self, t: Tuning) -> Tuning {
        if self.theme.reduce_motion { Tuning::new(t.response.min(0.25), 1.0) } else { t }
    }

    // ----- layers -----

    /// Index of a layer, created on first use.
    pub fn layer(&mut self, id: Id, order: Order) -> usize {
        let i = match self.layers.iter().position(|l| l.id == id) {
            Some(i) => i,
            None => {
                self.z_top += 1;
                // Reuse slots of layers that are gone.
                let f = self.frame;
                let l = Layer { id, order, z: self.z_top, used: f };
                match self.layers.iter().position(|l| f - l.used > 240) {
                    Some(i) => {
                        self.layers[i] = l;
                        i
                    }
                    None => {
                        self.layers.push(l);
                        self.layers.len() - 1
                    }
                }
            }
        };
        if self.layers[i].used + 1 < self.frame {
            // Reappearing layers go to the front of their order.
            self.z_top += 1;
            self.layers[i].z = self.z_top;
        }
        self.layers[i].order = order;
        self.layers[i].used = self.frame;
        self.paint.ensure_layer(i);
        i
    }

    pub fn painter(&mut self, layer: usize, clip: Rect) -> Painter<'_> {
        self.paint.ensure_layer(layer);
        Painter { paint: &mut self.paint, text: &mut self.text, layer, clip, mask: 0, opacity: 1.0 }
    }

    /// Painter for the whole window behind all widgets.
    pub fn background(&mut self) -> Painter<'_> {
        let l = self.layer(Id::new("background"), Order::Background);
        // The scene texture is padded; cover that too.
        let r = self.screen.expand(64.0);
        self.painter(l, r)
    }

    // ----- input -----

    pub fn interact(&mut self, id: Id, rect: Rect, clip: Rect, layer: usize, sense: Sense) -> Response {
        let visible = rect.intersect(clip);
        if visible.is_positive() {
            self.hits.push(Hit { id, rect: visible, layer, sense, scroll: false });
        }
        let hovered = self.hovered == Some(id) && (self.active.is_none() || self.active == Some(id));
        let held = self.active == Some(id) && (self.input.down || self.input.released);
        let focused = self.focus == Some(id);
        let key_click = focused
            && self.focus_by_key
            && sense == Sense::Click
            && (self.input.key(Key::Enter) || self.input.key(Key::Space));
        Response {
            id,
            rect,
            hovered,
            held: held && self.input.down,
            pressed: held && self.input.pressed,
            released: held && self.input.released,
            clicked: (held && self.input.released && hovered && sense != Sense::Hover) || key_click,
            double_clicked: held && self.input.pressed && self.input.double,
            secondary_clicked: hovered && self.input.secondary,
            focused,
            changed: false,
            delta: if held { self.input.delta } else { Vec2::ZERO },
            pointer: self.input.pointer,
        }
    }

    /// Register a widget for keyboard focus. Call every frame it is visible.
    pub fn focusable(&mut self, id: Id) {
        self.focus_list.push(id);
        if self.focus == Some(id) {
            self.focus_seen = true;
        }
    }
    pub fn set_focus(&mut self, id: Option<Id>) {
        self.focus = id;
        self.focus_seen = true;
    }
    /// Focus ring should be drawn.
    pub fn shows_focus(&self, id: Id) -> bool {
        self.focus == Some(id) && self.focus_by_key
    }
    fn move_focus(&mut self, dir: i32) {
        if self.focus_list.is_empty() {
            return;
        }
        let n = self.focus_list.len() as i32;
        let at = self.focus.and_then(|f| self.focus_list.iter().position(|x| *x == f));
        let next = match at {
            Some(i) => (i as i32 + dir).rem_euclid(n),
            None if dir > 0 => 0,
            None => n - 1,
        };
        self.focus = Some(self.focus_list[next as usize]);
        self.focus_by_key = true;
    }

    pub fn scroll_area(&mut self, id: Id, rect: Rect, layer: usize) -> Vec2 {
        self.hits.push(Hit { id, rect, layer, sense: Sense::Hover, scroll: true });
        if self.scroll_target == Some(id) { self.input.scroll } else { Vec2::ZERO }
    }

    pub fn is_active(&self, id: Id) -> bool {
        self.active == Some(id)
    }
    /// Take over the current drag.
    pub fn grab(&mut self, id: Id) {
        self.active = Some(id);
    }
    pub fn any_active(&self) -> bool {
        self.active.is_some()
    }

    // ----- glass -----

    /// Adaptive state for a glass element. `flips` is false for large surfaces,
    /// which keep the theme's appearance.
    pub fn adapt(&mut self, id: Id, flips: bool) -> Adapted {
        let dark_theme = self.theme.dark;
        let adaptive = self.theme.adaptive;
        let f = self.frame;
        let dt = self.dt;
        let a = self.adapt.entry(id.0).or_insert_with(|| Adapt {
            mean: if dark_theme { 0.1 } else { 0.9 },
            is_dark: dark_theme,
            ..Default::default()
        });
        a.used = f;
        if !flips || !adaptive {
            a.is_dark = dark_theme;
        } else if a.known {
            // Hysteresis keeps the style stable over mid tones. Dark appearance
            // only gives way to really bright content and the other way round.
            let (to_dark, to_light) = if dark_theme { (0.62, 0.78) } else { (0.30, 0.44) };
            if a.is_dark && a.mean > to_light {
                a.is_dark = false;
            } else if !a.is_dark && a.mean < to_dark {
                a.is_dark = true;
            }
        }
        let mut moving = a.dark.step(a.is_dark as u32 as f32, Tuning::new(0.32, 1.0), dt);
        moving |= a.boost.step((a.dev * 4.0).min(1.0), Tuning::new(0.4, 1.0), dt);
        let out = Adapted { dark: a.dark.x.clamp(0.0, 1.0), boost: a.boost.x.clamp(0.0, 1.0) };
        if moving {
            self.repaint = true;
        }
        out
    }

    /// Feed measurements from the renderer. Returns true when a redraw would
    /// look different.
    pub fn apply_stats(&mut self, stats: &[(u64, f32, f32)]) -> bool {
        let mut changed = false;
        for &(k, mean, dev) in stats {
            if let Some(a) = self.adapt.get_mut(&k) {
                if !a.known {
                    changed = true;
                    a.known = true;
                    // First measurement: adopt without a visible transition.
                    if self.theme.adaptive && a.frame_young(self.frame) {
                        a.dark = Spring::default();
                    }
                }
                changed |= (a.mean - mean).abs() > 0.012 || (a.dev - dev).abs() > 0.02;
                a.mean = mean;
                a.dev = dev;
            }
        }
        changed
    }

    /// Set the adaptive fields of `glass` and have its backdrop measured.
    pub fn prepare_glass(&mut self, id: Id, glass: &mut Glass, flips: bool) -> Adapted {
        let a = self.adapt(id, flips);
        glass.darkness = a.dark;
        glass.shadow_boost = a.boost;
        glass.stat_key = Some(id.0);
        a
    }

    // ----- misc -----

    pub fn add_image(&mut self, w: u32, h: u32, rgba: Vec<u8>) -> ImageId {
        let id = ImageId(self.next_image);
        self.next_image += 1;
        self.uploads.push(ImageUpload { id, w, h, rgba });
        id
    }

    pub fn copy(&mut self, s: impl Into<String>) {
        self.host.push(HostCmd::Copy(s.into()));
    }

    pub fn measure(&mut self, text: &str, size: f32, w: Weight) -> Vec2 {
        self.text.layout(text, &TextStyle::new(size).weight(w)).size
    }

    pub fn toast(&mut self, icon: Option<crate::icons::Icon>, text: impl Into<String>) {
        self.toasts.push(crate::widgets::overlay::ToastState::new(icon, text.into()));
    }
    pub(crate) fn take_toasts(&mut self) -> Vec<crate::widgets::overlay::ToastState> {
        std::mem::take(&mut self.toasts)
    }
    pub(crate) fn put_toasts(&mut self, t: Vec<crate::widgets::overlay::ToastState>) {
        self.toasts = t;
    }

    pub fn popup_open(&self, id: Id) -> bool {
        self.open_popups.get(&id).copied().unwrap_or(false)
    }
    pub fn set_popup(&mut self, id: Id, open: bool) {
        self.open_popups.insert(id, open);
        self.repaint = true;
    }
    pub fn toggle_popup(&mut self, id: Id) {
        let o = !self.popup_open(id);
        self.set_popup(id, o);
    }

    /// Pointer pressed this frame outside of `r`.
    pub fn pressed_outside(&self, r: Rect) -> bool {
        self.input.pressed && self.input.pointer.is_some_and(|p| !r.contains(p))
    }

    pub fn pointer_or(&self, p: Vec2) -> Vec2 {
        self.input.pointer.unwrap_or(p)
    }

    pub fn center(&self) -> Vec2 {
        vec2(self.screen.width() * 0.5, self.screen.height() * 0.5)
    }
}

impl Adapt {
    fn frame_young(&self, frame: u64) -> bool {
        self.used + 4 >= frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::rect;

    fn frame(ctx: &mut Context, ev: &[Event], f: impl FnOnce(&mut Context)) {
        let t = ctx.time + 1.0 / 60.0;
        ctx.begin_frame(vec2(200.0, 200.0), 1.0, t, ev);
        f(ctx);
        ctx.end_frame();
    }

    #[test]
    fn topmost_layer_gets_the_click() {
        let mut c = Context::new();
        let (low, high) = (Id::new("low"), Id::new("high"));
        let mut got = (false, false);
        let mut run = |c: &mut Context, ev: &[Event]| {
            frame(c, ev, |c| {
                let a = c.layer(Id::new("a"), Order::Content);
                let b = c.layer(Id::new("b"), Order::Popup);
                let r = rect(0.0, 0.0, 100.0, 100.0);
                got.1 |= c.interact(high, r, r, b, Sense::Click).clicked;
                got.0 |= c.interact(low, r, r, a, Sense::Click).clicked;
            });
        };
        run(&mut c, &[Event::PointerMoved(vec2(10.0, 10.0))]);
        run(&mut c, &[Event::PointerDown]);
        run(&mut c, &[Event::PointerUp]);
        assert_eq!(got, (false, true));
    }

    #[test]
    fn tab_moves_focus_and_space_clicks() {
        let mut c = Context::new();
        let (a, b) = (Id::new("a"), Id::new("b"));
        let mut clicked = None;
        let mut run = |c: &mut Context, ev: &[Event]| {
            frame(c, ev, |c| {
                let l = c.layer(Id::new("l"), Order::Content);
                for id in [a, b] {
                    c.focusable(id);
                    if c.interact(id, rect(0.0, 0.0, 9.0, 9.0), Rect::EVERYTHING, l, Sense::Click).clicked {
                        clicked = Some(id);
                    }
                }
            });
        };
        run(&mut c, &[]);
        run(&mut c, &[Event::Key(Key::Tab)]);
        run(&mut c, &[Event::Key(Key::Tab)]);
        run(&mut c, &[Event::Key(Key::Space)]);
        assert_eq!(clicked, Some(b));
    }

    #[test]
    fn glass_flips_with_hysteresis() {
        let mut c = Context::new();
        let id = Id::new("g");
        let settle = |c: &mut Context, luma: f32| {
            let mut d = 0.0;
            for _ in 0..120 {
                c.apply_stats(&[(id.0, luma, 0.0)]);
                frame(c, &[], |c| d = c.adapt(id, true).dark);
            }
            d
        };
        assert_eq!(settle(&mut c, 0.9), 0.0);
        assert_eq!(settle(&mut c, 0.36), 0.0);
        assert_eq!(settle(&mut c, 0.1), 1.0);
        assert_eq!(settle(&mut c, 0.36), 1.0);
        assert_eq!(settle(&mut c, 0.6), 0.0);
    }
}
