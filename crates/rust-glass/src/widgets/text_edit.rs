use unicode_segmentation::UnicodeSegmentation;

use crate::access::Role;
use crate::anim::Tuning;
use crate::color::Color;
use crate::context::{Cursor, HostCmd, Response, Sense};
use crate::geom::{Rect, vec2};
use crate::icons::Icon;
use crate::input::Key;
use crate::material::{Corner, Glass};
use crate::text::{TextLayout, TextStyle};
use crate::ui::{Ui, Widget};

/// Single or multi line text field with undo, selection, clipboard and input
/// method composition.
pub struct TextEdit<'a> {
    text: &'a mut String,
    hint: &'a str,
    icon: Option<Icon>,
    password: bool,
    rows: usize,
    width: Option<f32>,
    clear_button: bool,
}

#[derive(Clone, Copy, Default, Debug)]
struct Edit {
    cursor: usize,
    anchor: usize,
    scroll: f32,
    blink: f64,
}

impl Edit {
    fn range(&self) -> (usize, usize) {
        (self.cursor.min(self.anchor), self.cursor.max(self.anchor))
    }
    fn clamp(&mut self, s: &str) {
        let fix = |i: usize| {
            let mut i = i.min(s.len());
            while !s.is_char_boundary(i) {
                i -= 1;
            }
            i
        };
        self.cursor = fix(self.cursor);
        self.anchor = fix(self.anchor);
    }
    fn replace(&mut self, s: &mut String, with: &str) {
        let (a, b) = self.range();
        s.replace_range(a..b, with);
        self.cursor = a + with.len();
        self.anchor = self.cursor;
    }
}

/// Kinds of edits. Runs of the same kind undo in one step.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
enum Kind {
    #[default]
    None,
    Insert,
    Delete,
    /// Paste, cut, new line, clear: always a step of its own.
    Other,
}

#[derive(Clone, Debug)]
struct Snap {
    text: String,
    cursor: usize,
    anchor: usize,
}

/// Undo and redo stacks of one field.
#[derive(Clone, Default, Debug)]
struct History {
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    /// Text after the last edit seen here. Anything else was changed by the
    /// app, which starts a new history.
    known: Option<String>,
    kind: Kind,
    at: f64,
    caret: usize,
}

const UNDO_LIMIT: usize = 200;
/// A pause this long starts a new undo step even in a run of typing.
const UNDO_PAUSE: f64 = 1.2;

impl History {
    /// Call before a change, with the state it changes.
    fn record(&mut self, kind: Kind, text: &str, e: &Edit, now: f64) {
        let merge = kind != Kind::Other
            && kind == self.kind
            && now - self.at < UNDO_PAUSE
            && e.cursor == self.caret
            && !self.undo.is_empty();
        if !merge {
            self.undo.push(Snap { text: text.to_string(), cursor: e.cursor, anchor: e.anchor });
            if self.undo.len() > UNDO_LIMIT {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.kind = kind;
        self.at = now;
    }

    /// Step back (`redo` false) or forward. Returns true if something changed.
    fn step(&mut self, redo: bool, text: &mut String, e: &mut Edit) -> bool {
        let (from, to) =
            if redo { (&mut self.redo, &mut self.undo) } else { (&mut self.undo, &mut self.redo) };
        let Some(s) = from.pop() else { return false };
        to.push(Snap { text: std::mem::replace(text, s.text), cursor: e.cursor, anchor: e.anchor });
        e.cursor = s.cursor;
        e.anchor = s.anchor;
        self.kind = Kind::None;
        true
    }
}

fn prev(s: &str, i: usize) -> usize {
    s[..i].grapheme_indices(true).next_back().map_or(0, |g| g.0)
}
fn next(s: &str, i: usize) -> usize {
    s[i..].graphemes(true).next().map_or(s.len(), |g| i + g.len())
}
fn prev_word(s: &str, i: usize) -> usize {
    s[..i].unicode_word_indices().next_back().map_or(0, |w| w.0)
}
fn next_word(s: &str, i: usize) -> usize {
    s[i..].unicode_word_indices().next().map_or(s.len(), |w| i + w.0 + w.1.len())
}

impl<'a> TextEdit<'a> {
    pub fn new(text: &'a mut String) -> Self {
        Self { text, hint: "", icon: None, password: false, rows: 1, width: None, clear_button: false }
    }
    pub fn hint(mut self, h: &'a str) -> Self {
        self.hint = h;
        self
    }
    pub fn icon(mut self, i: Icon) -> Self {
        self.icon = Some(i);
        self
    }
    pub fn password(mut self) -> Self {
        self.password = true;
        self
    }
    pub fn rows(mut self, n: usize) -> Self {
        self.rows = n.max(1);
        self
    }
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    /// Magnifier icon and a clear button.
    pub fn search(mut self) -> Self {
        self.icon = Some(Icon::Search);
        self.clear_button = true;
        self
    }
}

impl Widget for TextEdit<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let multi = self.rows > 1;
        let font = ui.ctx.theme.body;
        let line = (font * 1.28).ceil();
        let h = if multi { line * self.rows as f32 + 22.0 } else { ui.ctx.theme.control_height };
        let w = self.width.unwrap_or_else(|| ui.available_width().clamp(80.0, 420.0));
        let mut r = ui.alloc_interact(vec2(w, h), Sense::Drag);
        let id = r.id;
        ui.ctx.focusable(id);
        let mut e: Edit = ui.ctx.get(id);
        let mut hist: History = ui.ctx.take(id.with("undo"));
        if hist.known.as_deref() != Some(self.text.as_str()) {
            // Changed by the app: older steps would undo its change.
            hist = History { known: Some(self.text.clone()), ..Default::default() };
        }
        let now = ui.ctx.time;
        if r.hovered {
            ui.ctx.cursor = Cursor::Text;
        }
        let corner = if multi { Corner::Radius(16.0) } else { Corner::Capsule };
        let mut left = 14.0;
        if self.icon.is_some() {
            left += 22.0;
        }
        let show_clear = self.clear_button && !self.text.is_empty();
        let right = if show_clear { 34.0 } else { 14.0 };
        let top = if multi { 11.0 } else { (h - line) * 0.5 };
        let inner = Rect {
            min: r.rect.min + vec2(left, top),
            max: r.rect.max - vec2(right, if multi { 11.0 } else { 0.0 }),
        };
        let mut st = TextStyle::new(font);
        if multi {
            st = st.wrap(inner.width());
        }

        let before = self.text.clone();
        e.clamp(self.text);
        if r.pressed {
            ui.ctx.set_focus(Some(id));
            r.focused = true;
            e.blink = now;
        }
        if let Some(v) = ui.ctx.take_access_text(id) {
            hist.record(Kind::Other, self.text, &e, now);
            *self.text = v;
            e.cursor = self.text.len();
            e.anchor = e.cursor;
        }
        // While an input method composes, keys belong to it.
        let preedit = if r.focused && !self.password { ui.ctx.input.preedit.clone() } else { None };
        if r.focused && preedit.is_none() {
            let keys = ui.ctx.input.keys.clone();
            let typed: String =
                ui.ctx.input.text.chars().filter(|c| !c.is_control() || (multi && *c == '\n')).collect();
            for (k, m) in keys {
                e.blink = now;
                let word = m.ctrl || m.alt;
                let moved = match k {
                    Key::Left if !m.shift && e.cursor != e.anchor => Some(e.range().0),
                    Key::Right if !m.shift && e.cursor != e.anchor => Some(e.range().1),
                    Key::Left => {
                        Some(if word { prev_word(self.text, e.cursor) } else { prev(self.text, e.cursor) })
                    }
                    Key::Right => {
                        Some(if word { next_word(self.text, e.cursor) } else { next(self.text, e.cursor) })
                    }
                    Key::Up | Key::Down if multi => {
                        let l = ui.layout_text(self.text, &st);
                        let (x, y, lh) = l.caret(e.cursor);
                        let y = y + lh * if k == Key::Up { -0.5 } else { 1.5 };
                        Some(if y < 0.0 {
                            0
                        } else if y > l.size.y {
                            self.text.len()
                        } else {
                            l.hit(vec2(x, y))
                        })
                    }
                    Key::Home | Key::Up => Some(line_start(self.text, e.cursor, k == Key::Up)),
                    Key::End | Key::Down => Some(line_end(self.text, e.cursor, k == Key::Down)),
                    _ => None,
                };
                if let Some(to) = moved {
                    e.cursor = to;
                    if !m.shift {
                        e.anchor = to;
                    }
                    continue;
                }
                match k {
                    Key::Backspace | Key::Delete => {
                        if e.cursor == e.anchor {
                            e.anchor = match (k == Key::Backspace, word) {
                                (true, true) => prev_word(self.text, e.cursor),
                                (true, false) => prev(self.text, e.cursor),
                                (false, true) => next_word(self.text, e.cursor),
                                (false, false) => next(self.text, e.cursor),
                            };
                        }
                        if e.cursor != e.anchor {
                            let kind = if e.range().1 - e.range().0 > 4 { Kind::Other } else { Kind::Delete };
                            hist.record(kind, self.text, &e, now);
                            e.replace(self.text, "");
                        }
                    }
                    Key::Enter if multi => {
                        hist.record(Kind::Other, self.text, &e, now);
                        e.replace(self.text, "\n");
                    }
                    Key::Escape | Key::Enter => {
                        ui.ctx.set_focus(None);
                        r.focused = false;
                    }
                    Key::Char('a') if m.command => {
                        e.anchor = 0;
                        e.cursor = self.text.len();
                    }
                    Key::Char('z') if m.command => {
                        hist.step(m.shift, self.text, &mut e);
                    }
                    Key::Char('y') if m.command && !cfg!(target_os = "macos") => {
                        hist.step(true, self.text, &mut e);
                    }
                    Key::Char(c @ ('c' | 'x')) if m.command && !self.password => {
                        let (a, b) = e.range();
                        if a != b {
                            ui.ctx.host.push(HostCmd::Copy(self.text[a..b].to_string()));
                            if c == 'x' {
                                hist.record(Kind::Other, self.text, &e, now);
                                e.replace(self.text, "");
                            }
                        }
                    }
                    _ => {}
                }
            }
            if !typed.is_empty() {
                // A single character continues a run of typing; a space ends
                // the word, so the next one starts a new step.
                let one = typed.graphemes(true).count() == 1 && e.cursor == e.anchor;
                let kind = if one { Kind::Insert } else { Kind::Other };
                hist.record(kind, self.text, &e, now);
                e.replace(self.text, &typed);
                if typed.chars().all(char::is_whitespace) {
                    hist.kind = Kind::None;
                }
                e.blink = now;
            }
        }

        let shown =
            if self.password { "\u{2022}".repeat(self.text.chars().count()) } else { self.text.clone() };
        // Byte offsets differ between the text and its dots.
        let text_ref: &str = self.text;
        let password = self.password;
        let to_shown = |i: usize| if password { text_ref[..i].chars().count() * 3 } else { i };
        let from_shown = |i: usize| {
            if password { text_ref.char_indices().nth(i / 3).map_or(text_ref.len(), |c| c.0) } else { i }
        };
        let l = ui.layout_text(&shown, &st);
        if r.held
            && preedit.is_none()
            && let Some(p) = r.pointer
        {
            let q = p - inner.min + if multi { vec2(0.0, e.scroll) } else { vec2(e.scroll, 0.0) };
            e.cursor = from_shown(l.hit(q));
            if r.pressed && !ui.ctx.input.modifiers.shift {
                e.anchor = e.cursor;
            }
        }
        if r.double_clicked {
            e.anchor = prev_word(text_ref, next(text_ref, e.cursor).max(e.cursor));
            e.cursor = next_word(text_ref, e.anchor);
        }

        // The composition is drawn in place of the selection, underlined.
        let (a, b) = e.range();
        let (sa, sb) = (to_shown(a), to_shown(b));
        let (l, caret, composing) = match &preedit {
            Some(p) => {
                let s = format!("{}{}{}", &shown[..sa], p.text, &shown[sb..]);
                let at = sa + p.cursor.map_or(p.text.len(), |c| c.1.min(p.text.len()));
                (ui.layout_text(&s, &st), at, Some((sa, sa + p.text.len())))
            }
            None => (l, to_shown(e.cursor), None),
        };

        // Keep the caret visible.
        let (cx, cy, ch) = l.caret(caret);
        if multi {
            let view = inner.height();
            if cy < e.scroll {
                e.scroll = cy;
            }
            if cy + ch > e.scroll + view {
                e.scroll = cy + ch - view;
            }
            e.scroll = e.scroll.clamp(0.0, (l.size.y - view).max(0.0));
        } else {
            let view = inner.width();
            if cx < e.scroll {
                e.scroll = cx;
            }
            if cx > e.scroll + view {
                e.scroll = cx - view;
            }
            e.scroll = e.scroll.clamp(0.0, (l.size.x - view).max(0.0));
        }
        ui.ctx.set(id, e);
        if before != *self.text || hist.known.is_none() {
            hist.known = Some(self.text.clone());
        }
        hist.caret = e.cursor;
        ui.ctx.set(id.with("undo"), hist);

        let f = ui.ctx.animate_bool(id.with("f"), r.focused, Tuning::SMOOTH);
        let hov = ui.ctx.animate_bool(id.with("h"), r.hovered, Tuning::SNAP);
        let accent = ui.ctx.theme.accent;
        let mut s = ui.surface;
        if ui.surface.is_glass() {
            let c = s.fill();
            ui.painter().fill(r.rect, corner, c.fade(1.0 + 0.4 * hov));
        } else {
            let mut m = ui.ctx.theme.regular;
            m.blur = m.blur.max(4.0);
            let mut g = Glass::new(r.rect, corner, m);
            s = ui.ctx.prepare_glass(id, &mut g, !multi).surface();
            ui.painter().glass(&g);
        }
        let off = if multi { vec2(0.0, e.scroll) } else { vec2(e.scroll, 0.0) };
        let origin = inner.min - off;
        let caret_on = r.focused && (composing.is_some() || ((now - e.blink) % 1.06) < 0.56);
        if r.focused {
            ui.ctx.request_repaint();
            let c = origin + vec2(cx, cy);
            ui.ctx.ime = Some(Rect { min: c, max: c + vec2(1.0, ch) });
        }
        let hint = ui.layout_text(self.hint, &st);
        let old_clip = ui.clip;
        ui.clip =
            ui.clip.intersect(Rect { min: inner.min - vec2(2.0, 2.0), max: inner.max + vec2(2.0, 2.0) });
        let text_color = s.text();
        let mut p = ui.painter();
        if composing.is_none() && sa != sb && f > 0.01 {
            for (s0, s1, top, height) in spans(&l, sa, sb) {
                let sel = Rect { min: origin + vec2(s0, top), max: origin + vec2(s1, top + height) };
                p.fill(sel, 3.0, accent.alpha(0.30 * f));
            }
        }
        if let Some((c0, c1)) = composing {
            for (s0, s1, top, height) in spans(&l, c0, c1) {
                let y = top + height - 2.0;
                p.line(origin + vec2(s0, y), origin + vec2(s1, y), 1.4, text_color.fade(0.8));
            }
        }
        if shown.is_empty() && composing.is_none() {
            p.text(origin, &hint, s.tertiary());
        }
        p.text(origin, &l, text_color);
        if caret_on {
            let c = origin + vec2(cx, cy);
            p.fill(Rect { min: c + vec2(-1.0, 1.0), max: c + vec2(1.0, ch - 1.0) }, 1.0, accent);
        }
        ui.clip = old_clip;
        if let Some(i) = self.icon {
            let y = if multi { r.rect.min.y + 11.0 + line * 0.5 } else { r.rect.center().y };
            let c = s.secondary();
            ui.painter().icon(vec2(r.rect.min.x + 22.0, y), 16.0, i, c);
        }
        if show_clear {
            let c = vec2(r.rect.max.x - 19.0, r.rect.center().y);
            let cr = Rect::from_center_size(c, vec2(20.0, 20.0));
            let x = ui.interact(id.with("clear"), cr, Sense::Click);
            if let Some(n) = ui.access(x.id, Role::Button, cr) {
                n.label("Clear text");
            }
            if x.clicked {
                let mut hist: History = ui.ctx.take(id.with("undo"));
                hist.record(Kind::Other, self.text, &e, now);
                self.text.clear();
                hist.known = Some(String::new());
                ui.ctx.set(id.with("undo"), hist);
                ui.ctx.set_focus(Some(id));
            }
            if x.hovered {
                ui.ctx.cursor = Cursor::Pointer;
            }
            let (bg, fg) = (s.tertiary(), if s.dark() > 0.5 { Color::BLACK } else { Color::WHITE });
            let mut p = ui.painter();
            p.fill(cr.shrink(1.5), Corner::Capsule, bg);
            p.icon(c, 12.0, Icon::Close, fg);
        }
        if f > 0.01 {
            ui.painter().stroke(r.rect.expand(2.0), corner, 2.0, accent.alpha(0.55 * f.clamp(0.0, 1.0)));
        }
        let role = if self.password {
            Role::PasswordInput
        } else if multi {
            Role::MultilineTextInput
        } else if self.icon == Some(Icon::Search) {
            Role::SearchInput
        } else {
            Role::TextInput
        };
        if let Some(n) = ui.access(id, role, r.rect) {
            n.label(self.hint).value(shown);
        }
        r.changed = before != *self.text;
        r
    }
}

/// Horizontal pieces of the byte range `a..b` per line: (x0, x1, top, height).
fn spans(l: &TextLayout, a: usize, b: usize) -> Vec<(f32, f32, f32, f32)> {
    let mut out = Vec::new();
    for ln in &l.lines {
        let s0 = a.max(ln.start);
        let s1 = b.min(ln.end);
        if s0 >= s1 {
            continue;
        }
        let x = |i: usize| ln.stops.iter().rev().find(|s| s.0 <= i).map_or(0.0, |s| s.1);
        out.push((x(s0), x(s1), ln.top, ln.height));
    }
    out
}

fn line_start(s: &str, i: usize, all: bool) -> usize {
    if all { 0 } else { s[..i].rfind('\n').map_or(0, |p| p + 1) }
}
fn line_end(s: &str, i: usize, all: bool) -> usize {
    if all { s.len() } else { s[i..].find('\n').map_or(s.len(), |p| i + p) }
}

impl Ui<'_> {
    pub fn text_edit(&mut self, text: &mut String, hint: &str) -> Response {
        self.add(TextEdit::new(text).hint(hint))
    }
    pub fn search_field(&mut self, text: &mut String, hint: &str) -> Response {
        self.add(TextEdit::new(text).hint(hint).search())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grapheme_and_word_steps() {
        let s = "a👩‍🚀b two";
        assert_eq!(next(s, 1), s.find('b').unwrap());
        assert_eq!(prev(s, s.find('b').unwrap()), 1);
        assert_eq!(prev_word(s, s.len()), s.find("two").unwrap());
    }

    #[test]
    fn replace_selection() {
        let mut s = String::from("hello world");
        let mut e = Edit { cursor: 0, anchor: 5, ..Default::default() };
        e.replace(&mut s, "bye");
        assert_eq!(s, "bye world");
        assert_eq!(e.cursor, 3);
    }

    #[test]
    fn clamp_lands_on_char_boundary() {
        let mut e = Edit { cursor: 2, anchor: 99, ..Default::default() };
        e.clamp("é");
        assert_eq!((e.cursor, e.anchor), (2, 2));
        let mut e = Edit { cursor: 1, ..Default::default() };
        e.clamp("é");
        assert_eq!(e.cursor, 0);
    }

    #[test]
    fn typing_runs_undo_in_one_step() {
        let mut h = History::default();
        let mut s = String::new();
        let mut e = Edit::default();
        for (i, c) in "ab".chars().enumerate() {
            h.record(Kind::Insert, &s, &e, i as f64 * 0.1);
            e.replace(&mut s, &c.to_string());
            h.caret = e.cursor;
        }
        // After a pause, typing starts a new step.
        h.record(Kind::Insert, &s, &e, 5.0);
        e.replace(&mut s, "c");
        h.caret = e.cursor;
        assert_eq!(s, "abc");
        assert!(h.step(false, &mut s, &mut e));
        assert_eq!((s.as_str(), e.cursor), ("ab", 2));
        assert!(h.step(false, &mut s, &mut e));
        assert_eq!(s, "");
        assert!(!h.step(false, &mut s, &mut e));
        assert!(h.step(true, &mut s, &mut e));
        assert!(h.step(true, &mut s, &mut e));
        assert_eq!(s, "abc");
    }
}
