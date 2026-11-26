//! The state of one window: input, theme, layers, widget memory and the
//! measurements glass adapts to.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use crate::accessibility::{Access, Action, LayerInfo, Node, Role};
use crate::animation::{Spring, Spring2d, SpringRect, Tuning};
use crate::color::Color;
use crate::geometry::{Rect, Vec2, vec2};
use crate::input::{Event, Input, Key};
use crate::material::Glass;
use crate::paint::{ImageId, Paint, Painter};
use crate::text::{TextEngine, TextStyle, Weight};
use crate::theme::{Surface, Theme};

/// Identifies a widget across frames, so it can keep state such as an
/// animation or a scroll position.
///
/// Ids are hashes. Widgets added to a [`Ui`](crate::Ui) get one from their
/// position automatically. Give your own with [`Id::new`] when the widget
/// can move in the layout, for example in a list that reorders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(pub u64);

impl Id {
    /// Creates an id from any hashable value.
    pub fn new(source: impl Hash) -> Self {
        let mut h = std::hash::DefaultHasher::new();
        source.hash(&mut h);
        Id(h.finish())
    }
    /// Derives a child id, for a part of this widget.
    pub fn with(self, source: impl Hash) -> Self {
        let mut h = std::hash::DefaultHasher::new();
        self.0.hash(&mut h);
        source.hash(&mut h);
        Id(h.finish())
    }
}

/// Where a layer sits in the stack. Later orders draw on top and get input
/// first. Within one order, the layer used or clicked last is on top.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Order {
    /// Behind everything, such as a wallpaper.
    Background,
    /// The app content.
    Content,
    /// Floating bars: toolbars, tab bars and sidebars.
    Bar,
    /// Floating windows.
    Window,
    /// Alerts and sheets, which block everything below.
    Modal,
    /// Menus and other popups.
    Popup,
    /// Toasts.
    Toast,
    /// Tooltips and items being dragged.
    Tooltip,
}

/// What a widget reacts to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sense {
    /// Only hovering. It still blocks the widgets below it.
    Hover,
    /// Clicks.
    Click,
    /// Clicks and drags.
    Drag,
}

/// The mouse cursor shape. Set [`Context::cursor`] while drawing a widget.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Cursor {
    /// The arrow.
    #[default]
    Default,
    /// A hand, for things that can be clicked.
    Pointer,
    /// A text cursor.
    Text,
    /// For things that can be dragged.
    Grab,
    /// A horizontal resize arrow.
    ResizeHorizontal,
    /// The widget is disabled.
    NotAllowed,
}

/// What happened to a widget this frame.
#[derive(Clone, Copy, Debug)]
pub struct Response {
    /// The widget's id.
    pub id: Id,
    /// Where the widget is.
    pub rect: Rect,
    /// The pointer is over the widget and nothing else is being dragged.
    pub hovered: bool,
    /// The pointer is down and the press started on this widget.
    pub held: bool,
    /// The press started on this widget this frame.
    pub pressed: bool,
    /// A press that started on this widget ended this frame.
    pub released: bool,
    /// The widget was clicked, with the pointer, the keyboard or a screen
    /// reader.
    pub clicked: bool,
    /// The press this frame is the second of a double click.
    pub double_clicked: bool,
    /// The secondary button was pressed over the widget.
    pub secondary_clicked: bool,
    /// The widget has keyboard focus.
    pub focused: bool,
    /// The value the widget edits changed this frame.
    pub changed: bool,
    /// How far the pointer moved while held.
    pub delta: Vec2,
    /// Pointer position, if the pointer is in the window.
    pub pointer: Option<Vec2>,
}

impl Response {
    /// Returns [`Response::clicked`].
    pub fn clicked(&self) -> bool {
        self.clicked
    }
    /// Returns [`Response::changed`].
    pub fn changed(&self) -> bool {
        self.changed
    }
    /// Returns [`Response::hovered`].
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

/// How a glass element should look over what is behind it. Returned by
/// [`Context::backdrop`] and [`Context::prepare_glass`].
#[derive(Clone, Copy, Debug)]
pub struct Backdrop {
    /// 0 for light glass with dark labels, 1 for dark glass with light labels.
    pub darkness: f32,
    /// Extra shadow over busy content, from 0 to 1.
    pub shadow_boost: f32,
}

impl Backdrop {
    /// Returns the surface for content drawn on this glass.
    pub fn surface(&self) -> Surface {
        Surface::Glass { dark: self.darkness }
    }
}

/// A request from the UI to the window host, handled after the frame.
#[derive(Clone, Debug)]
pub enum HostCommand {
    /// Put text on the clipboard.
    Copy(String),
    /// Open a web address in the default browser.
    OpenUrl(String),
    /// Close the window and end [`run`](crate::run).
    Close,
}

pub(crate) struct ImageUpload {
    pub id: ImageId,
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

/// The state of one window. Every widget reads and writes it.
///
/// [`App::update`](crate::App::update) gets it each frame. Most code reaches
/// it through [`Ui::ctx`](crate::Ui::ctx).
pub struct Context {
    /// Input of the current frame.
    pub input: Input,
    /// Appearance and accessibility settings.
    pub theme: Theme,
    pub(crate) paint: Paint,
    /// Text layout and fonts.
    pub text: TextEngine,
    /// Seconds since the app started.
    pub time: f64,
    /// Seconds since the last frame, at most 1/24 so an idle gap does not
    /// turn into one big animation step.
    pub dt: f32,
    /// Number of the current frame.
    pub frame: u64,
    /// The window, in points. Its top left corner is `(0, 0)`.
    pub screen: Rect,
    /// Physical pixels per point, from the display scale.
    pub pixels_per_point: f32,
    /// Color behind everything.
    pub clear_color: Color,
    /// Mouse cursor for this frame. Widgets set it while hovered.
    pub cursor: Cursor,
    /// Requests to the window host, handled after the frame.
    pub host_commands: Vec<HostCommand>,
    pub(crate) uploads: Vec<ImageUpload>,
    pub(crate) removed_images: Vec<ImageId>,
    /// Caret of the focused text field, where an input method shows its
    /// candidates. The field sets it every frame it has focus.
    pub ime_area: Option<Rect>,
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
    /// A widget that got focus from the keyboard or a screen reader and should
    /// be scrolled into view, with the frames left to find it.
    pub(crate) reveal: Option<(Id, u8)>,
    /// Where that widget was drawn this frame.
    pub(crate) reveal_rect: Option<Rect>,
    adapt: HashMap<u64, Adapt>,
    repaint: bool,
    relayout: bool,
    light: Spring,
    tilt: Vec2,
    scroll_energy: f32,
    toasts: Vec<crate::widgets::toast::ToastState>,
    open_popups: HashMap<Id, bool>,
    access: Access,
    drawn: Vec<usize>,
    drag: Option<Drag>,
    morphs: HashMap<Id, Morph>,
}

/// An element shown by one view after another. See [`Context::morph`].
#[derive(Clone, Copy)]
struct Morph {
    view: Id,
    shape: SpringRect,
    moving: bool,
    seen: u64,
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
    /// Creates the state of a window with the default theme.
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
            pixels_per_point: 1.0,
            clear_color: Color::gray(0.85),
            cursor: Cursor::Default,
            host_commands: Vec::new(),
            uploads: Vec::new(),
            removed_images: Vec::new(),
            ime_area: None,
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
            reveal: None,
            reveal_rect: None,
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
            morphs: HashMap::new(),
        }
    }

    // ----- frame -----

    /// Starts a frame of `size` points at `pixels_per_point`, `time` seconds after
    /// the start, with the input `events`. The window host and
    /// [`Headless`](crate::headless::Headless) call it for you.
    pub fn begin_frame(&mut self, size: Vec2, ppp: f32, time: f64, events: &[Event]) {
        // A long idle gap must not turn into one huge animation step.
        self.dt = ((time - self.time) as f32).clamp(0.0, 1.0 / 24.0);
        self.time = time;
        self.frame += 1;
        self.pixels_per_point = ppp;
        self.screen = Rect::from_min_size(Vec2::ZERO, size);
        self.cursor = Cursor::Default;
        self.repaint = false;
        self.relayout = false;
        self.ime_area = None;
        self.reveal_rect = None;
        self.input.begin(events, time);
        self.access.nodes.clear();
        self.access.requests.clear();
        for e in events {
            if let Event::Access(id, a) = e {
                self.access_request(*id, a.clone());
            }
        }
        self.paint.begin(ppp);
        self.paint.glass_tint = self.theme.glass_tint;
        self.paint.reduce_transparency = self.theme.reduce_transparency;
        self.paint.increase_contrast = self.theme.increase_contrast;
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

    /// Ends the frame and builds the draw list. The window host and
    /// [`Headless`](crate::headless::Headless) call it for you.
    pub fn end_frame(&mut self) {
        // A focused widget is revealed once. Give up on one that is not drawn.
        self.reveal = match self.reveal {
            Some(_) if self.reveal_rect.is_some() => None,
            Some((id, frames)) if frames > 0 => Some((id, frames - 1)),
            _ => None,
        };
        crate::widgets::toast::show_toasts(self);
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
            self.morphs.retain(|_, m| f - m.seen < 1200);
        }
        let mut order: Vec<usize> =
            (0..self.layers.len()).filter(|&i| self.layers[i].used == self.frame).collect();
        order.sort_by_key(|&i| (self.layers[i].order, self.layers[i].z, i));
        self.paint.flatten(&order);
        self.drawn = order;
        let a = self.light.value;
        let d = &mut self.paint.data;
        d.clear = self.clear_color.to_array();
        d.light = [a.sin(), -a.cos()];
        d.time = self.time as f32;
        d.ppp = self.pixels_per_point;
        if self.input.pending() || self.text.needs_rebuild() {
            self.repaint = true;
        }
    }

    /// Asks for another frame as soon as possible. Call it every frame while
    /// something animates. An idle window draws nothing.
    pub fn request_repaint(&mut self) {
        self.repaint = true;
    }
    /// Returns `true` if another frame was asked for.
    pub fn wants_repaint(&self) -> bool {
        self.repaint
    }
    /// Tells the host that sizes used for layout this frame were guesses. The
    /// frame runs again before it is shown.
    pub fn request_relayout(&mut self) {
        self.relayout = true;
    }
    /// Returns `true` if the frame should run again before it is shown.
    pub fn wants_relayout(&self) -> bool {
        self.relayout
    }

    /// Sets the tilt of the device in radians. It turns the highlights on glass.
    pub fn set_tilt(&mut self, tilt: Vec2) {
        self.tilt = tilt;
    }
    /// Adds to the movement that briefly turns the highlights while scrolling.
    pub fn add_scroll_energy(&mut self, v: f32) {
        self.scroll_energy = (self.scroll_energy + v).clamp(-1.0, 1.0);
    }

    // ----- memory -----

    /// Returns a copy of the state stored for `id`, or the default if there is
    /// none.
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
    /// Returns a copy of the state stored for `id`, if there is any.
    pub fn try_get<T: Clone + 'static>(&mut self, id: Id) -> Option<T> {
        let f = self.frame;
        let (b, used) = self.states.get_mut(&(id, TypeId::of::<T>()))?;
        *used = f;
        b.downcast_ref::<T>().cloned()
    }
    /// Moves the state out, leaving nothing stored. Cheaper than [`Self::get`] for
    /// large state. Put it back with [`Self::set`].
    pub fn take<T: Default + 'static>(&mut self, id: Id) -> T {
        match self.states.remove(&(id, TypeId::of::<T>())) {
            Some((b, _)) => b.downcast::<T>().map(|b| *b).unwrap_or_default(),
            None => T::default(),
        }
    }
    /// Stores state for `id`. State that is not used for a while is dropped.
    pub fn set<T: 'static>(&mut self, id: Id, value: T) {
        self.states.insert((id, TypeId::of::<T>()), (Box::new(value), self.frame));
    }

    /// Moves a value stored under `id` towards `target` on a spring and returns
    /// it. Keeps asking for frames until it arrives.
    pub fn animate(&mut self, id: Id, target: f32, tuning: Tuning) -> f32 {
        let mut s: Spring = self.get(id);
        if s.step(target, self.tuning(tuning), self.dt) {
            self.repaint = true;
        }
        self.set(id, s);
        s.value
    }
    /// Like [`Self::animate`], towards 1 when `on` and 0 when off.
    pub fn animate_bool(&mut self, id: Id, on: bool, tuning: Tuning) -> f32 {
        self.animate(id, on as u32 as f32, tuning)
    }
    /// Like [`Self::animate`], for a point. Returns the spring, so you can read
    /// its velocity too.
    pub fn animate_vec2(&mut self, id: Id, target: Vec2, tuning: Tuning) -> Spring2d {
        let mut s: Spring2d = self.get(id);
        if s.step(target, self.tuning(tuning), self.dt) {
            self.repaint = true;
        }
        self.set(id, s);
        s
    }

    /// Returns `tuning` as it should be used: unchanged, or short and without
    /// overshoot when [`Theme::reduce_motion`] is on.
    pub fn tuning(&self, tuning: Tuning) -> Tuning {
        if self.theme.reduce_motion { Tuning::new(tuning.response.min(0.25), 1.0) } else { tuning }
    }

    // ----- layers -----

    /// Returns the index of the layer `id`, created on first use. Layers draw in
    /// [`Order`], and within an order the one used or clicked last is on top.
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

    /// Returns a painter for `layer` that clips to `clip`.
    pub fn painter(&mut self, layer: usize, clip: Rect) -> Painter<'_> {
        self.paint.ensure_layer(layer);
        Painter { paint: &mut self.paint, text: &mut self.text, layer, clip, mask: 0, opacity: 1.0 }
    }

    /// Returns a painter for the whole window, behind all widgets.
    pub fn background(&mut self) -> Painter<'_> {
        let l = self.layer(Id::new("background"), Order::Background);
        // The scene texture is padded; cover that too.
        let r = self.screen.expand(64.0);
        self.painter(l, r)
    }

    // ----- input -----

    /// Registers `rect` on `layer` for input and returns what happened to it.
    /// Parts outside `clip` do not react. Only the topmost widget under the
    /// pointer is hovered.
    pub fn interact(&mut self, id: Id, rect: Rect, clip: Rect, layer: usize, sense: Sense) -> Response {
        if self.reveal.is_some_and(|(wanted, _)| wanted == id) {
            self.reveal_rect = Some(rect);
        }
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
            double_clicked: held && self.input.pressed && self.input.double_clicked,
            secondary_clicked: hovered && self.input.secondary_clicked,
            focused,
            changed: false,
            delta: if held { self.input.delta } else { Vec2::ZERO },
            pointer: self.input.pointer,
        }
    }

    /// Lets `id` take keyboard focus with Tab. Call it every frame the widget is
    /// visible.
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
    /// Gives keyboard focus to `id`, or takes it away with `None`.
    pub fn set_focus(&mut self, id: Option<Id>) {
        self.focus = id;
        self.focus_seen = true;
    }
    /// Returns `true` if `id` has focus from the keyboard, which is when the focus
    /// ring shows.
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
        self.reveal = self.focus.map(|id| (id, 3));
    }

    /// Registers a scroll area and returns the wheel movement it receives, on
    /// both axes.
    pub fn scroll_area(&mut self, id: Id, rect: Rect, layer: usize) -> Vec2 {
        self.scroll_area_axes(id, rect, layer, true, true)
    }

    /// Registers a scroll area that scrolls on the given axes and returns the
    /// wheel movement it receives. Movement on the other axis goes to the scroll
    /// area around it, so a horizontal row inside a page leaves vertical scrolling
    /// to the page.
    pub fn scroll_area_axes(&mut self, id: Id, rect: Rect, layer: usize, x: bool, y: bool) -> Vec2 {
        let scroll = x as u8 | (y as u8) << 1;
        self.hits.push(Hit { id, rect, layer, sense: Sense::Hover, scroll });
        let s = self.input.scroll;
        vec2(
            if x && self.scroll_target[0] == Some(id) { s.x } else { 0.0 },
            if y && self.scroll_target[1] == Some(id) { s.y } else { 0.0 },
        )
    }

    /// Returns `true` while `id` is being pressed or dragged.
    pub fn is_active(&self, id: Id) -> bool {
        self.active == Some(id)
    }
    /// Makes `id` the target of the current press or drag.
    pub fn grab(&mut self, id: Id) {
        self.active = Some(id);
    }
    /// Returns `true` while any widget is being pressed or dragged.
    pub fn any_active(&self) -> bool {
        self.active.is_some()
    }

    // ----- glass -----

    /// Returns how the glass element `id` should look over what is behind it.
    /// With `adaptive` set, small elements switch between light and dark with the
    /// backdrop. Large surfaces keep the theme appearance.
    pub fn backdrop(&mut self, id: Id, adaptive: bool) -> Backdrop {
        let dark_theme = self.theme.dark;
        let theme_adaptive = self.theme.adaptive;
        let f = self.frame;
        let dt = self.dt;
        let a = self.adapt.entry(id.0).or_insert_with(|| Adapt {
            mean: if dark_theme { 0.1 } else { 0.9 },
            is_dark: dark_theme,
            ..Default::default()
        });
        a.used = f;
        if !adaptive || !theme_adaptive {
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
        let out =
            Backdrop { darkness: a.dark.value.clamp(0.0, 1.0), shadow_boost: a.boost.value.clamp(0.0, 1.0) };
        if moving {
            self.repaint = true;
        }
        out
    }

    /// Feed measurements from the renderer. Returns true when a redraw would
    /// look different.
    pub(crate) fn apply_stats(&mut self, stats: &[(u64, f32, f32)]) -> bool {
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

    /// Adapts `glass` to what is behind it and has the backdrop measured for the
    /// next frame. Call it before painting glass you build yourself.
    pub fn prepare_glass(&mut self, id: Id, glass: &mut Glass, adaptive: bool) -> Backdrop {
        let a = self.backdrop(id, adaptive);
        glass.darkness = a.darkness;
        glass.shadow_boost = a.shadow_boost;
        glass.stat_key = Some(id.0);
        a
    }

    // ----- accessibility -----

    /// Turns collecting the accessibility tree on or off. The window host turns it
    /// on when a screen reader asks for the tree.
    pub fn set_accessibility(&mut self, on: bool) {
        self.access.on = on;
        self.repaint = true;
    }
    /// Returns `true` while the accessibility tree is collected.
    pub fn accessibility(&self) -> bool {
        self.access.on
    }

    /// Describes an element for screen readers. Returns `None` while none is
    /// listening, so building labels costs nothing then.
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

    /// Labels the node `id` described this frame, for widgets without text of
    /// their own, such as sliders.
    pub fn access_label(&mut self, id: Id, label: impl Into<String>) {
        if let Some(n) = self.access.nodes.iter_mut().rev().find(|n| n.id == id) {
            n.label = label.into();
        }
    }

    /// Labels every node described since `from` (an index into
    /// [`Self::access_nodes`]) that has no label yet. Settings rows use it to name
    /// the switch or slider at their end.
    pub fn access_label_since(&mut self, from: usize, label: &str) {
        for n in self.access.nodes.iter_mut().skip(from) {
            if n.label.is_empty() && n.role != Role::Label {
                n.label = label.to_string();
            }
        }
    }

    /// Returns the nodes described so far this frame.
    pub fn access_nodes(&self) -> &[Node] {
        &self.access.nodes
    }

    /// Returns `true` if a screen reader asked for `action` on `id` this frame.
    pub fn access_requested(&self, id: Id, action: &Action) -> bool {
        self.access.requests.iter().any(|(i, x)| *i == id && x == action)
    }

    /// Returns the text a screen reader wants to put into the field `id`.
    pub fn take_access_text(&mut self, id: Id) -> Option<String> {
        let i =
            self.access.requests.iter().position(|(i, a)| *i == id && matches!(a, Action::SetValue(_)))?;
        match self.access.requests.remove(i).1 {
            Action::SetValue(s) => Some(s),
            _ => None,
        }
    }

    /// Returns the value a screen reader wants to give the slider or stepper `id`.
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
                self.reveal = Some((id, 3));
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

    /// Returns the full accessibility tree of the last frame for AccessKit.
    pub fn access_update(&self, title: &str) -> accesskit::TreeUpdate {
        let layers: Vec<LayerInfo> = self
            .drawn
            .iter()
            .map(|&i| LayerInfo { index: i, id: self.layers[i].id, order: self.layers[i].order })
            .collect();
        crate::accessibility::tree(
            &self.access.nodes,
            &layers,
            self.focus,
            self.pixels_per_point,
            self.screen,
            title,
        )
    }

    // ----- morphing between views -----

    /// Moves an element from the view that showed it before to the view that
    /// shows it now, such as a cover that grows from a grid into a detail
    /// page.
    ///
    /// Call it every frame the element is drawn, with the same `key` from
    /// every view, the `view` that draws it, and where that view puts it.
    /// Draw the element at the returned rectangle. While the view stays the
    /// same, the result is `rect`. When the view changes, it travels from the
    /// old place to `rect` on a spring. An element that was not drawn for 20
    /// seconds starts over where it is.
    ///
    /// ```
    /// # use rust_glass::prelude::*;
    /// # fn f(ui: &mut Ui, image: ImageId, open: bool) {
    /// let key = Id::new(("cover", 7));
    /// let (view, place) = if open {
    ///     (Id::new("detail"), rect(40.0, 40.0, 320.0, 320.0))
    /// } else {
    ///     (Id::new("grid"), ui.allocate(vec2(120.0, 120.0)))
    /// };
    /// let shown = ui.ctx.morph(key, view, place);
    /// ui.painter().image(shown, 14.0, image, rect(0.0, 0.0, 1.0, 1.0), Color::WHITE);
    /// # }
    /// ```
    pub fn morph(&mut self, key: Id, view: Id, rect: Rect) -> Rect {
        let frame = self.frame;
        let fresh = Morph { view, shape: SpringRect::new(rect), moving: false, seen: frame };
        let mut morph = match self.morphs.get(&key) {
            Some(m) if frame - m.seen < 1200 => *m,
            _ => fresh,
        };
        if morph.view != view {
            morph.view = view;
            morph.moving = true;
        }
        let shown = if morph.moving {
            let center = self.tuning(Tuning::new(0.44, 0.84));
            let size = self.tuning(Tuning::new(0.44, 0.92));
            morph.moving = morph.shape.step(rect, center, size, self.dt);
            self.repaint = true;
            if morph.moving { morph.shape.rect() } else { rect }
        } else {
            morph.shape = SpringRect::new(rect);
            rect
        };
        morph.seen = frame;
        self.morphs.insert(key, morph);
        shown
    }

    /// Returns `true` while the element `key` travels between views.
    pub fn is_morphing(&self, key: Id) -> bool {
        self.morphs.get(&key).is_some_and(|m| m.moving)
    }

    // ----- drag and drop -----

    /// Picks up `payload`. [`Ui::drag_source`](crate::Ui::drag_source) calls it
    /// for you.
    pub fn start_drag(&mut self, source: Id, payload: Box<dyn Any>, grab: Vec2, size: Vec2) {
        self.drag = Some(Drag { source, payload, grab, size });
        self.active = Some(source);
        self.repaint = true;
    }
    /// Returns the id of the widget being dragged.
    pub fn dragged_id(&self) -> Option<Id> {
        self.drag.as_ref().map(|d| d.source)
    }
    /// Returns the payload being dragged, if it has type `P`.
    pub fn dragged<P: Clone + 'static>(&self) -> Option<P> {
        self.drag.as_ref()?.payload.downcast_ref::<P>().cloned()
    }
    /// Returns where the dragged item is drawn: at the pointer, keeping the
    /// offset where it was grabbed.
    pub fn drag_rect(&self) -> Option<Rect> {
        let d = self.drag.as_ref()?;
        let p = self.input.pointer?;
        Some(Rect::from_min_size(p - d.grab, d.size))
    }
    /// Returns `true` while a payload of type `P` is dragged over `rect`.
    pub fn drag_over<P: 'static>(&self, rect: Rect) -> bool {
        self.drag.as_ref().is_some_and(|d| d.payload.is::<P>())
            && self.input.pointer.is_some_and(|p| rect.contains(p))
    }
    /// Returns the payload when it is released over `rect`. Only the first caller
    /// gets it.
    pub fn take_drop<P: 'static>(&mut self, rect: Rect) -> Option<P> {
        if !self.input.released || !self.drag_over::<P>(rect) {
            return None;
        }
        let d = self.drag.take()?;
        self.repaint = true;
        d.payload.downcast::<P>().ok().map(|b| *b)
    }

    // ----- misc -----

    /// Adds an image of `width` by `height` pixels in RGBA order, 8 bits per
    /// channel, and returns its id. Draw it with [`Ui::image`](crate::Ui::image)
    /// or [`Painter::image`](crate::Painter::image).
    pub fn add_image(&mut self, width: u32, height: u32, rgba: Vec<u8>) -> ImageId {
        let id = ImageId(self.next_image);
        self.next_image += 1;
        self.uploads.push(ImageUpload { id, w: width, h: height, rgba });
        id
    }

    /// Frees an image added with [`Self::add_image`]. Drawing it afterwards shows
    /// nothing.
    pub fn remove_image(&mut self, id: ImageId) {
        self.removed_images.push(id);
    }

    /// Puts `text` on the clipboard.
    pub fn copy_text(&mut self, text: impl Into<String>) {
        self.host_commands.push(HostCommand::Copy(text.into()));
    }

    /// Returns the size of `text` in one line at `size` points.
    pub fn measure(&mut self, text: &str, size: f32, weight: Weight) -> Vec2 {
        self.text.layout(text, &TextStyle::new(size).weight(weight)).size
    }

    /// Shows a short message at the top of the window for a few seconds.
    pub fn toast(&mut self, icon: Option<crate::icon::Icon>, text: impl Into<String>) {
        self.toasts.push(crate::widgets::toast::ToastState::new(icon, text.into()));
    }
    pub(crate) fn take_toasts(&mut self) -> Vec<crate::widgets::toast::ToastState> {
        std::mem::take(&mut self.toasts)
    }
    pub(crate) fn put_toasts(&mut self, t: Vec<crate::widgets::toast::ToastState>) {
        self.toasts = t;
    }

    /// Returns `true` if the popup `id` is open.
    pub fn popup_open(&self, id: Id) -> bool {
        self.open_popups.get(&id).copied().unwrap_or(false)
    }
    /// Opens or closes the popup `id`.
    pub fn set_popup(&mut self, id: Id, open: bool) {
        self.open_popups.insert(id, open);
        self.repaint = true;
    }
    /// Opens the popup `id` if it is closed, and closes it if it is open.
    pub fn toggle_popup(&mut self, id: Id) {
        let o = !self.popup_open(id);
        self.set_popup(id, o);
    }

    /// Returns `true` if the pointer was pressed this frame outside `rect`.
    pub fn pressed_outside(&self, rect: Rect) -> bool {
        self.input.pressed && self.input.pointer.is_some_and(|p| !rect.contains(p))
    }

    /// Returns the pointer position, or `fallback` while the pointer is outside
    /// the window.
    pub fn pointer_or(&self, fallback: Vec2) -> Vec2 {
        self.input.pointer.unwrap_or(fallback)
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
    use crate::geometry::rect;

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
                frame(c, &[], |c| d = c.backdrop(id, true).darkness);
            }
            d
        };
        assert_eq!(settle(&mut c, 0.9), 0.0);
        assert_eq!(settle(&mut c, 0.36), 0.0);
        assert_eq!(settle(&mut c, 0.1), 1.0);
        assert_eq!(settle(&mut c, 0.36), 1.0);
        assert_eq!(settle(&mut c, 0.6), 0.0);
    }

    #[test]
    fn morph_follows_its_view_and_travels_to_a_new_one() {
        let mut c = Context::new();
        let key = Id::new("cover");
        let (grid, detail) = (Id::new("grid"), Id::new("detail"));
        let small = rect(10.0, 10.0, 50.0, 50.0);
        let large = rect(100.0, 100.0, 200.0, 200.0);
        let mut shown = Rect::default();
        frame(&mut c, &[], |c| shown = c.morph(key, grid, small));
        assert_eq!(shown, small);
        // Same view, new place: follows at once, as when a list scrolls.
        let moved = small.translate(vec2(0.0, 30.0));
        frame(&mut c, &[], |c| shown = c.morph(key, grid, moved));
        assert_eq!(shown, moved);
        // Another view: travels there.
        frame(&mut c, &[], |c| shown = c.morph(key, detail, large));
        assert!(shown.width() < large.width() && shown.min.x < large.min.x, "{shown:?}");
        assert!(c.is_morphing(key));
        for _ in 0..120 {
            frame(&mut c, &[], |c| shown = c.morph(key, detail, large));
        }
        assert_eq!(shown, large);
        assert!(!c.is_morphing(key));
    }
}
