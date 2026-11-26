//! Input state for one frame.

use crate::access::Action;
use crate::context::Id;
use crate::geom::{Vec2, vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    Char(char),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    /// Command on macOS, control elsewhere.
    pub command: bool,
}

#[derive(Clone, Debug)]
pub enum Event {
    PointerMoved(Vec2),
    PointerLeft,
    PointerDown,
    PointerUp,
    SecondaryDown,
    Scroll(Vec2),
    Key(Key),
    Text(String),
    Modifiers(Modifiers),
    Focus(bool),
    Paste(String),
    /// Text being composed by an input method, not committed yet. The range
    /// is the cursor or selection inside it, in bytes. An empty string ends
    /// the composition.
    Preedit(String, Option<(usize, usize)>),
    /// A request from assistive technology, such as a screen reader.
    Access(Id, Action),
}

/// Composition of an input method, shown at the caret of the focused field.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Preedit {
    pub text: String,
    pub cursor: Option<(usize, usize)>,
}

#[derive(Clone, Debug, Default)]
pub struct Input {
    pub pointer: Option<Vec2>,
    pub delta: Vec2,
    pub down: bool,
    pub pressed: bool,
    pub released: bool,
    pub secondary: bool,
    pub double: bool,
    pub press_origin: Vec2,
    pub scroll: Vec2,
    pub keys: Vec<(Key, Modifiers)>,
    pub text: String,
    pub modifiers: Modifiers,
    pub focused: bool,
    /// Text an input method is composing. Persists across frames.
    pub preedit: Option<Preedit>,
    last_press: f64,
    /// A press and its release in the same frame: the release waits one frame.
    deferred_up: bool,
}

impl Input {
    pub fn new() -> Self {
        Self { focused: true, last_press: -10.0, ..Default::default() }
    }

    pub fn begin(&mut self, events: &[Event], time: f64) {
        self.delta = Vec2::ZERO;
        self.pressed = false;
        self.released = false;
        self.secondary = false;
        self.double = false;
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
                    self.double = time - self.last_press < 0.35 && (p - self.press_origin).length() < 5.0;
                    self.last_press = if self.double { -10.0 } else { time };
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
                Event::SecondaryDown => self.secondary = true,
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
    pub fn pending(&self) -> bool {
        self.deferred_up
    }

    pub fn key(&self, k: Key) -> bool {
        self.keys.iter().any(|x| x.0 == k)
    }

    /// Consume a key press.
    pub fn take_key(&mut self, k: Key) -> bool {
        let n = self.keys.len();
        self.keys.retain(|x| x.0 != k);
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
            assert_eq!(i.double, want);
            i.begin(&[Event::PointerUp], t + 0.05);
        }
    }
}
