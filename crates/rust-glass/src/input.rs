//! Keyboard, pointer and text input.
//!
//! The window host turns platform events into [`Event`]s. The state of the
//! current frame is in [`Context::input`](crate::Context::input).

use crate::accessibility::Action;
use crate::context::Id;
use crate::geometry::{Vec2, vec2};

/// A key that widgets react to. Letters arrive lowercase in [`Key::Char`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Key {
    Enter,
    Escape,
    Tab,
    Space,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    /// A character key, lowercase.
    Char(char),
}

/// Modifier keys held during a key press.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    /// Shift.
    pub shift: bool,
    /// Control.
    pub ctrl: bool,
    /// Alt, or Option on macOS.
    pub alt: bool,
    /// The key for shortcuts: Command on macOS, Control elsewhere.
    pub command: bool,
}

/// Something that happened since the last frame. Positions are in points.
#[derive(Clone, Debug)]
pub enum Event {
    /// The pointer moved to this position.
    PointerMoved(Vec2),
    /// The pointer left the window.
    PointerLeft,
    /// The primary button or a touch went down.
    PointerDown,
    /// The primary button or a touch went up.
    PointerUp,
    /// The secondary button went down.
    SecondaryDown,
    /// The wheel or touchpad scrolled by this many points.
    Scroll(Vec2),
    /// A key was pressed.
    Key(Key),
    /// Text was typed.
    Text(String),
    /// The modifier keys changed.
    Modifiers(Modifiers),
    /// The window gained (`true`) or lost focus.
    Focus(bool),
    /// Text was pasted from the clipboard.
    Paste(String),
    /// Text an input method is composing and has not committed yet. The
    /// range is the cursor or selection inside it, in bytes. An empty string
    /// ends the composition.
    Preedit(String, Option<(usize, usize)>),
    /// A request from assistive technology, such as a screen reader.
    Access(Id, Action),
}

/// Text an input method is composing, shown at the caret of the focused
/// field.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Preedit {
    /// The text so far.
    pub text: String,
    /// The cursor or selection inside the text, in bytes.
    pub cursor: Option<(usize, usize)>,
}

/// Input state of the current frame.
#[derive(Clone, Debug, Default)]
pub struct Input {
    /// Pointer position, or `None` outside the window.
    pub pointer: Option<Vec2>,
    /// How far the pointer moved since the last frame.
    pub delta: Vec2,
    /// The primary button is down.
    pub down: bool,
    /// The primary button went down this frame.
    pub pressed: bool,
    /// The primary button went up this frame.
    pub released: bool,
    /// The secondary button went down this frame.
    pub secondary_clicked: bool,
    /// This frame's press is the second of a double click.
    pub double_clicked: bool,
    /// Where the last press started.
    pub press_origin: Vec2,
    /// Wheel movement this frame. Shift turns vertical movement sideways.
    pub scroll: Vec2,
    /// Keys pressed this frame. Widgets remove the ones they handle with
    /// [`Input::take_key`].
    pub keys: Vec<(Key, Modifiers)>,
    /// Text typed or pasted this frame.
    pub text: String,
    /// Modifier keys held now.
    pub modifiers: Modifiers,
    /// The window has keyboard focus.
    pub focused: bool,
    /// Text an input method is composing. Persists across frames.
    pub preedit: Option<Preedit>,
    last_press: f64,
    /// A press and its release in the same frame: the release waits one frame.
    deferred_up: bool,
}

impl Input {
    /// Creates the state of a focused window without input.
    pub fn new() -> Self {
        Self { focused: true, last_press: -10.0, ..Default::default() }
    }

    /// Starts a frame: clears what lasted one frame and applies `events`.
    pub(crate) fn begin(&mut self, events: &[Event], time: f64) {
        self.delta = Vec2::ZERO;
        self.pressed = false;
        self.released = false;
        self.secondary_clicked = false;
        self.double_clicked = false;
        self.scroll = Vec2::ZERO;
        self.keys.clear();
        self.text.clear();
        if std::mem::take(&mut self.deferred_up) {
            self.released = true;
            self.down = false;
        }
        for e in events {
            match e {
                Event::PointerMoved(p) => {
                    if let Some(old) = self.pointer {
                        self.delta += *p - old;
                    }
                    self.pointer = Some(*p);
                }
                Event::PointerLeft => {
                    if !self.down {
                        self.pointer = None;
                    }
                }
                Event::PointerDown => {
                    self.pressed = true;
                    self.down = true;
                    let p = self.pointer.unwrap_or(vec2(-1.0, -1.0));
                    self.double_clicked =
                        time - self.last_press < 0.35 && (p - self.press_origin).length() < 5.0;
                    self.last_press = if self.double_clicked { -10.0 } else { time };
                    self.press_origin = p;
                }
                Event::PointerUp => {
                    if self.pressed {
                        self.deferred_up = true;
                    } else {
                        self.released = true;
                        self.down = false;
                    }
                }
                Event::SecondaryDown => self.secondary_clicked = true,
                Event::Scroll(d) => self.scroll += *d,
                Event::Key(k) => self.keys.push((*k, self.modifiers)),
                Event::Text(t) => self.text.push_str(t),
                Event::Paste(t) => self.text.push_str(t),
                Event::Preedit(t, c) => {
                    self.preedit = (!t.is_empty()).then(|| Preedit { text: t.clone(), cursor: *c });
                }
                Event::Access(..) => {}
                Event::Modifiers(m) => self.modifiers = *m,
                Event::Focus(f) => {
                    self.focused = *f;
                    if !*f && self.down {
                        self.down = false;
                        self.released = true;
                    }
                }
            }
        }
        // Shift turns a mouse wheel sideways.
        if self.modifiers.shift && self.scroll.x == 0.0 {
            self.scroll = vec2(self.scroll.y, 0.0);
        }
    }

    /// Another frame is needed to deliver everything.
    pub(crate) fn pending(&self) -> bool {
        self.deferred_up
    }

    /// Returns `true` if `key` was pressed this frame and no widget took it.
    pub fn key(&self, key: Key) -> bool {
        self.keys.iter().any(|x| x.0 == key)
    }

    /// Returns `true` if `key` was pressed this frame, and removes it so
    /// widgets drawn later do not see it.
    pub fn take_key(&mut self, key: Key) -> bool {
        let n = self.keys.len();
        self.keys.retain(|x| x.0 != key);
        n != self.keys.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_within_one_frame_is_not_lost() {
        let mut i = Input::new();
        i.begin(&[Event::PointerMoved(vec2(1.0, 1.0)), Event::PointerDown, Event::PointerUp], 0.0);
        assert!(i.pressed && i.down && !i.released && i.pending());
        i.begin(&[], 0.016);
        assert!(!i.pressed && !i.down && i.released);
    }

    #[test]
    fn double_click() {
        let mut i = Input::new();
        for (t, want) in [(0.0, false), (0.2, true), (0.3, false)] {
            i.begin(&[Event::PointerMoved(vec2(1.0, 1.0)), Event::PointerDown], t);
            assert_eq!(i.double_clicked, want);
            i.begin(&[Event::PointerUp], t + 0.05);
        }
    }
}
