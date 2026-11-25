use unicode_segmentation::UnicodeSegmentation;

use crate::anim::Tuning;
use crate::color::Color;
use crate::context::{Cursor, HostCmd, Response, Sense};
use crate::geom::{Rect, vec2};
use crate::icons::Icon;
use crate::input::Key;
use crate::material::{Corner, Glass};
use crate::text::TextStyle;
use crate::ui::{Ui, Widget};

/// Single or multi line text field.
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
            e.blink = ui.ctx.time;
        }
        if r.focused {
            let keys = ui.ctx.input.keys.clone();
            let typed: String =
                ui.ctx.input.text.chars().filter(|c| !c.is_control() || (multi && *c == '\n')).collect();
            for (k, m) in keys {
                e.blink = ui.ctx.time;
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
                    Key::Backspace => {
                        if e.cursor == e.anchor {
                            e.anchor =
                                if word { prev_word(self.text, e.cursor) } else { prev(self.text, e.cursor) };
                        }
                        e.replace(self.text, "");
                    }
                    Key::Delete => {
                        if e.cursor == e.anchor {
                            e.anchor =
                                if word { next_word(self.text, e.cursor) } else { next(self.text, e.cursor) };
                        }
                        e.replace(self.text, "");
                    }
                    Key::Enter if multi => e.replace(self.text, "\n"),
                    Key::Escape | Key::Enter => {
                        ui.ctx.set_focus(None);
                        r.focused = false;
                    }
                    Key::Char('a') if m.command => {
                        e.anchor = 0;
                        e.cursor = self.text.len();
                    }
                    Key::Char(c @ ('c' | 'x')) if m.command && !self.password => {
                        let (a, b) = e.range();
                        if a != b {
                            ui.ctx.host.push(HostCmd::Copy(self.text[a..b].to_string()));
                            if c == 'x' {
                                e.replace(self.text, "");
                            }
                        }
                    }
                    _ => {}
                }
            }
            if !typed.is_empty() {
                e.replace(self.text, &typed);
                e.blink = ui.ctx.time;
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

        // Keep the caret visible.
        let (cx, cy, ch) = l.caret(to_shown(e.cursor));
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
            let at = r.pointer.unwrap_or(r.rect.center());
            g.glow = Some((at, 0.05 * hov + 0.10 * f));
            ui.painter().glass(&g);
        }
        let off = if multi { vec2(0.0, e.scroll) } else { vec2(e.scroll, 0.0) };
        let origin = inner.min - off;
        let caret_on = r.focused && ((ui.ctx.time - e.blink) % 1.06) < 0.56;
        if r.focused {
            ui.ctx.request_repaint();
        }
        let hint = ui.layout_text(self.hint, &st);
        let old_clip = ui.clip;
        ui.clip =
            ui.clip.intersect(Rect { min: inner.min - vec2(2.0, 2.0), max: inner.max + vec2(2.0, 2.0) });
        let (a, b) = e.range();
        let (a, b) = (to_shown(a), to_shown(b));
        let mut p = ui.painter();
        if a != b && f > 0.01 {
            for ln in &l.lines {
                let s0 = a.max(ln.start);
                let s1 = b.min(ln.end);
                if s0 >= s1 {
                    continue;
                }
                let x = |i: usize| ln.stops.iter().rev().find(|s| s.0 <= i).map_or(0.0, |s| s.1);
                let sel =
                    Rect { min: origin + vec2(x(s0), ln.top), max: origin + vec2(x(s1), ln.top + ln.height) };
                p.fill(sel, 3.0, accent.alpha(0.30 * f));
            }
        }
        if shown.is_empty() {
            p.text(origin, &hint, s.tertiary());
        }
        p.text(origin, &l, s.text());
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
            if x.clicked {
                self.text.clear();
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
        r.changed = before != *self.text;
        r
    }
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
}
