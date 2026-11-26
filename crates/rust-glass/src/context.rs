//! Per window state: input routing, animation memory, layers and backdrop
//! measurements.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use crate::access::{Access, Action, LayerInfo, Node, Role};
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
    /// Axes a scroll area takes wheel input for: bit 0 x, bit 1 y.
    scroll: u8,
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
    /// Caret of the focused text field, where an input method shows its
    /// candidates. Set by the field every frame it has focus.
    pub ime: Option<Rect>,
    next_image: u32,
    states: HashMap<(Id, TypeId), (Box<dyn Any>, u64)>,
    layers: Vec<Layer>,
    z_top: u64,
    hits: Vec<Hit>,
    prev_hits: Vec<Hit>,
    pub(crate) hovered: Option<Id>,
    scroll_target: [Option<Id>; 2],
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
    scroll_energy: f32,
    toasts: Vec<crate::widgets::overlay::ToastState>,
    open_popups: HashMap<Id, bool>,
    access: Access,
    drawn: Vec<usize>,
    drag: Option<Drag>,
}

/// Something picked up with [`Ui::drag_source`](crate::Ui::drag_source).
struct Drag {
    source: Id,
    payload: Box<dyn Any>,
    /// Pointer position inside the item when it was picked up.
    grab: Vec2,
    size: Vec2,
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
            ime: None,
            next_image: 1,
            states: HashMap::new(),
            layers: Vec::new(),
            z_top: 0,
            hits: Vec::new(),
            prev_hits: Vec::new(),
            hovered: None,
            scroll_target: [None; 2],
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
            scroll_energy: 0.0,
            toasts: Vec::new(),
            open_popups: HashMap::new(),
            access: Access::default(),
            drawn: Vec::new(),
            drag: None,
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
        self.ime = None;
        self.input.begin(events, time);
        self.access.nodes.clear();
        self.access.requests.clear();
        for e in events {
            if let Event::Access(id, a) = e {
                self.access_request(*id, a.clone());
            }
        }
        self.paint.begin(ppp);
        self.paint.frost = self.theme.frost;
        self.paint.reduce_transparency = self.theme.reduce_transparency;
        self.paint.high_contrast = self.theme.high_contrast;
        self.text.begin_frame(ppp);

        std::mem::swap(&mut self.hits, &mut self.prev_hits);
        self.hits.clear();
        self.hovered = None;
        self.scroll_target = [None; 2];
        if let Some(p) = self.input.pointer {
            let key = |h: &Hit, i: usize| {
                let l = &self.layers[h.layer];
                (l.order, l.z, i)
            };
            type Best = Option<((Order, u64, usize), Hit)>;
            let mut best: Best = None;
            let mut best_scroll: [Best; 2] = [None; 2];
            for (i, h) in self.prev_hits.iter().enumerate() {
                if !h.rect.contains(p) {
                    continue;
                }
                let k = key(h, i);
                if h.scroll == 0 {
                    if best.as_ref().is_none_or(|b| k > b.0) {
                        best = Some((k, *h));
                    }
                    continue;
                }
                for (axis, slot) in best_scroll.iter_mut().enumerate() {
                    if h.scroll & (1 << axis) != 0 && slot.as_ref().is_none_or(|b| k > b.0) {
                        *slot = Some((k, *h));
                    }
                }
            }
            // A scroll area below an opaque layer does not scroll.
            let top = best.map(|(k, _)| (k.0, k.1));
            for (axis, s) in best_scroll.iter().enumerate() {
                if let Some((ks, s)) = s
                    && top.is_none_or(|t| (ks.0, ks.1) >= t)
                {
                    self.scroll_target[axis] = Some(s.id);
                }
            }
            if let Some((_, h)) = best {
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
            }
        }
        if self.input.pressed {
            self.focus_by_key = false;
            if self.hovered != self.focus {
                self.focus = None;
            }
        }
        if self.drag.is_some() && self.input.take_key(Key::Escape) {
            self.drag = None;
            self.active = None;
        }
        if self.input.take_key(Key::Tab) {
            self.move_focus(if self.input.modifiers.shift { -1 } else { 1 });
        }
        self.focus_list.clear();
        self.focus_seen = false;

        // The virtual light stays put, except for device tilt and a hint of
        // movement while scrolling.
        let mut target = self.tilt.x + self.scroll_energy * 0.22;
        self.scroll_energy *= (-self.dt * 9.0).exp();
        if self.theme.reduce_motion {
            target = 0.0;
        }
        if self.light.step(target, Tuning::new(0.55, 1.0), self.dt) {
            self.repaint = true;
        }
    }

    pub fn end_frame(&mut self) {
        crate::widgets::overlay::show_toasts(self);
        if self.input.released || !self.input.down {
            self.active = None;
            if self.drag.take().is_some() {
                self.repaint = true;
            }
        }
        if self.drag.is_some() {
            self.repaint = true;
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
        self.drawn = order;
        let a = self.light.x;
        let d = &mut self.paint.data;
        d.clear = self.clear.to_array();
        d.light = [a.sin(), -a.cos()];
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

    /// Device tilt in radians, turns the highlights.
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
    /// Move the state out, leaving nothing stored. Cheaper than [`Self::get`]
    /// for large state; put it back with [`Self::set`].
    pub fn take<T: Default + 'static>(&mut self, id: Id) -> T {
        match self.states.remove(&(id, TypeId::of::<T>())) {
            Some((b, _)) => b.downcast::<T>().map(|b| *b).unwrap_or_default(),
            None => T::default(),
        }
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
            self.hits.push(Hit { id, rect: visible, layer, sense, scroll: 0 });
        }
        let hovered = self.hovered == Some(id) && (self.active.is_none() || self.active == Some(id));
        let held = self.active == Some(id) && (self.input.down || self.input.released);
        let focused = self.focus == Some(id);
        let key_click = (focused
            && self.focus_by_key
            && sense == Sense::Click
            && (self.input.key(Key::Enter) || self.input.key(Key::Space)))
            || (sense != Sense::Hover && self.access_requested(id, &Action::Click));
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
        if self.access.on
            && let Some(n) = self.access.nodes.iter_mut().rev().find(|n| n.id == id)
        {
            n.focusable = true;
        }
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

    /// Wheel movement for a scroll area on both axes.
    pub fn scroll_area(&mut self, id: Id, rect: Rect, layer: usize) -> Vec2 {
        self.scroll_area_axes(id, rect, layer, true, true)
    }

    /// Wheel movement on the axes the area scrolls. The other axis goes to
    /// the scroll area around it, so a horizontal row inside a page leaves
    /// vertical scrolling to the page.
    pub fn scroll_area_axes(&mut self, id: Id, rect: Rect, layer: usize, x: bool, y: bool) -> Vec2 {
        let scroll = x as u8 | (y as u8) << 1;
        self.hits.push(Hit { id, rect, layer, sense: Sense::Hover, scroll });
        let s = self.input.scroll;
        vec2(
            if x && self.scroll_target[0] == Some(id) { s.x } else { 0.0 },
            if y && self.scroll_target[1] == Some(id) { s.y } else { 0.0 },
        )
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

    // ----- accessibility -----

    /// Collect the accessibility tree. The window host turns this on when a
    /// screen reader asks for the tree and off when it goes away.
    pub fn set_accessibility(&mut self, on: bool) {
        self.access.on = on;
        self.repaint = true;
    }
    pub fn accessibility(&self) -> bool {
        self.access.on
    }

    /// Describe an element for assistive technology. Returns `None` while no
    /// screen reader is listening, so building labels costs nothing then.
    /// [`Ui::access`](crate::Ui::access) fills in the layer for you.
    pub fn access_node(&mut self, id: Id, role: Role, rect: Rect, layer: usize) -> Option<&mut Node> {
        if !self.access.on {
            return None;
        }
        let mut n = Node::new(id, role, rect);
        n.layer = layer;
        n.focusable = self.focus_list.contains(&id);
        self.access.nodes.push(n);
        self.access.nodes.last_mut()
    }

    /// Label the node `id` described in this frame, for widgets that have no
    /// text of their own, such as sliders.
    pub fn access_label(&mut self, id: Id, label: impl Into<String>) {
        if let Some(n) = self.access.nodes.iter_mut().rev().find(|n| n.id == id) {
            n.label = label.into();
        }
    }

    /// Give every node described since `from` (an index into
    /// [`Self::access_nodes`]) that has no label this one. Rows use it to name
    /// the switch or slider at their end after their title.
    pub fn access_label_since(&mut self, from: usize, label: &str) {
        for n in self.access.nodes.iter_mut().skip(from) {
            if n.label.is_empty() && n.role != Role::Label {
                n.label = label.to_string();
            }
        }
    }

    /// Nodes described so far in this frame.
    pub fn access_nodes(&self) -> &[Node] {
        &self.access.nodes
    }

    /// The request was made for `id` this frame.
    pub fn access_requested(&self, id: Id, a: &Action) -> bool {
        self.access.requests.iter().any(|(i, x)| *i == id && x == a)
    }

    /// Text a screen reader wants to put into the field `id`.
    pub fn take_access_text(&mut self, id: Id) -> Option<String> {
        let i =
            self.access.requests.iter().position(|(i, a)| *i == id && matches!(a, Action::SetValue(_)))?;
        match self.access.requests.remove(i).1 {
            Action::SetValue(s) => Some(s),
            _ => None,
        }
    }

    /// Value a screen reader wants to give the slider or stepper `id`.
    pub fn take_access_number(&mut self, id: Id) -> Option<f64> {
        let i =
            self.access.requests.iter().position(|(i, a)| *i == id && matches!(a, Action::SetNumber(_)))?;
        match self.access.requests.remove(i).1 {
            Action::SetNumber(v) => Some(v),
            _ => None,
        }
    }

    fn access_request(&mut self, id: Id, a: Action) {
        self.repaint = true;
        match &a {
            Action::Focus => {
                self.focus = Some(id);
                self.focus_by_key = true;
            }
            Action::Blur => {
                if self.focus == Some(id) {
                    self.focus = None;
                }
            }
            // Steps arrive as arrow keys, which every adjustable widget handles.
            Action::Increment | Action::Decrement => {
                self.focus = Some(id);
                self.focus_by_key = true;
                let k = if a == Action::Increment { Key::Right } else { Key::Left };
                self.input.keys.push((k, Default::default()));
            }
            Action::ReplaceSelection(t) => {
                self.focus = Some(id);
                self.input.text.push_str(t);
            }
            Action::Click | Action::SetValue(_) | Action::SetNumber(_) => {}
        }
        self.access.requests.push((id, a));
    }

    /// The full tree of the last frame, for AccessKit.
    pub fn access_update(&self, title: &str) -> accesskit::TreeUpdate {
        let layers: Vec<LayerInfo> = self
            .drawn
            .iter()
            .map(|&i| LayerInfo { index: i, id: self.layers[i].id, order: self.layers[i].order })
            .collect();
        crate::access::tree(&self.access.nodes, &layers, self.focus, self.ppp, self.screen, title)
    }

    // ----- drag and drop -----

    /// Pick up `payload`. Usually called by [`Ui::drag_source`](crate::Ui::drag_source).
    pub fn start_drag(&mut self, source: Id, payload: Box<dyn Any>, grab: Vec2, size: Vec2) {
        self.drag = Some(Drag { source, payload, grab, size });
        self.active = Some(source);
        self.repaint = true;
    }
    /// Source of what is being dragged.
    pub fn dragged_id(&self) -> Option<Id> {
        self.drag.as_ref().map(|d| d.source)
    }
    /// Payload being dragged, if it has type `P`.
    pub fn dragged<P: Clone + 'static>(&self) -> Option<P> {
        self.drag.as_ref()?.payload.downcast_ref::<P>().cloned()
    }
    /// Where the dragged item is drawn: pointer minus the grab offset.
    pub fn drag_rect(&self) -> Option<Rect> {
        let d = self.drag.as_ref()?;
        let p = self.input.pointer?;
        Some(Rect::from_min_size(p - d.grab, d.size))
    }
    /// A payload of type `P` is dragged over `r`.
    pub fn drag_over<P: 'static>(&self, r: Rect) -> bool {
        self.drag.as_ref().is_some_and(|d| d.payload.is::<P>())
            && self.input.pointer.is_some_and(|p| r.contains(p))
    }
    /// The payload when it is released over `r`. Only the first taker gets it.
    pub fn take_drop<P: 'static>(&mut self, r: Rect) -> Option<P> {
        if !self.input.released || !self.drag_over::<P>(r) {
            return None;
        }
        let d = self.drag.take()?;
        self.repaint = true;
        d.payload.downcast::<P>().ok().map(|b| *b)
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
