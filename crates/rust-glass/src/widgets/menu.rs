//! Menus, menu buttons, combo boxes and context menus.

use crate::accessibility::Role;
use crate::animation::Tuning;
use crate::color::Color;
use crate::context::{Context, Cursor, Response, Sense};
use crate::geometry::{Rect, Vec2, smoothstep, vec2};
use crate::icon::Icon;
use crate::material::{Corner, Glass};
use crate::text::{TextStyle, Weight};
use crate::ui::{Ui, Widget};
use crate::widgets::popup::{popup, popup_progress};
use crate::widgets::press::PressEffect;

/// Row inside a menu.
pub struct MenuItem<'a> {
    label: &'a str,
    icon: Option<Icon>,
    shortcut: &'a str,
    checked: bool,
    destructive: bool,
}

impl<'a> MenuItem<'a> {
    /// Creates a menu item.
    pub fn new(label: &'a str) -> Self {
        Self { label, icon: None, shortcut: "", checked: false, destructive: false }
    }
    /// Shows `icon` before the label.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }
    /// Shows a keyboard shortcut at the end, such as `Ctrl N`. It is only a label.
    pub fn shortcut(mut self, shortcut: &'a str) -> Self {
        self.shortcut = shortcut;
        self
    }
    /// Shows a check mark when `checked`.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }
    /// Shows the item in red, for actions that delete something.
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }
}

impl Widget for MenuItem<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let w = ui.available_width();
        let mut r = ui.allocate_response(vec2(w, 36.0), Sense::Click);
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
        let st = TextStyle::new(ui.ctx.theme.body_size).weight(Weight::Medium);
        let l = ui.layout_text(self.label, &st);
        let s = ui.layout_text(self.shortcut, &TextStyle::new(ui.ctx.theme.caption_size));
        let c = if self.destructive { Color::RED } else { ui.surface.text_color() };
        let (fill, sec) = (ui.surface.fill_color(), ui.surface.secondary_color());
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

/// A button that opens a menu. Its glass grows into the menu, and the menu
/// flows back into the button when it closes.
pub struct MenuButton<'a> {
    label: &'a str,
    icon: Option<Icon>,
    chevron: bool,
    width: f32,
    min_width: f32,
}

impl<'a> MenuButton<'a> {
    /// Creates a menu button. Leave `label` empty for an icon only button.
    pub fn new(label: &'a str) -> Self {
        Self { label, icon: None, chevron: false, width: 230.0, min_width: 0.0 }
    }
    /// Shows `icon` before the label.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }
    /// Shows an up and down chevron at the end, as on a combo box.
    pub fn chevron(mut self) -> Self {
        self.chevron = true;
        self
    }
    /// Sets the width of the menu. The default is 230.
    pub fn menu_width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
    /// Makes the button at least `width` points wide. Its label then starts
    /// at the left.
    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }

    /// Shows the button, and the menu items drawn by `f` while it is open.
    /// Close the menu with [`Context::set_popup`](crate::Context::set_popup)
    /// on the returned id when an item is clicked.
    pub fn show(self, ui: &mut Ui, f: impl FnOnce(&mut Ui)) -> Response {
        let st = TextStyle::new(ui.ctx.theme.body_size).weight(Weight::Semibold);
        let l = ui.layout_text(self.label, &st);
        let h = ui.ctx.theme.control_height;
        let has_text = !self.label.is_empty();
        let iw = if self.icon.is_some() { 17.0 } else { 0.0 };
        let gap = if has_text && self.icon.is_some() { 7.0 } else { 0.0 };
        let cw = if self.chevron { 18.0 } else { 0.0 };
        let w = if has_text { (l.size.x + iw + gap + cw + 30.0).max(self.min_width) } else { h };
        let r = ui.allocate_response(vec2(w, h), Sense::Click);
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
        let gel = PressEffect::update(ui.ctx, &r, 1.0);
        let fade = 1.0 - smoothstep(0.0, 0.35, t);
        let mut s = ui.surface;
        if ui.surface.is_glass() {
            let c = s.fill_color();
            ui.painter().fill(gel.rect, Corner::Capsule, c.fade(fade * (1.0 + 0.5 * gel.hover)));
        } else {
            let mut g = Glass::new(gel.rect, Corner::Capsule, ui.ctx.theme.regular);
            s = ui.ctx.prepare_glass(r.id, &mut g, true).surface();
            gel.apply(&mut g);
            g.presence = fade;
            ui.painter().glass(&g);
        }
        let c = s.text_color().fade(fade);
        let content = l.size.x + iw + gap + cw;
        let k = gel.scale;
        let mut x = if self.min_width > 0.0 && has_text {
            gel.rect.min.x + 15.0
        } else {
            gel.rect.center().x - content * k * 0.5
        };
        let cy = gel.rect.center().y;
        let sec = s.secondary_color().fade(fade);
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

/// A button that shows the selected option and opens a menu of all options.
pub struct ComboBox<'a> {
    selected: &'a mut usize,
    options: &'a [&'a str],
    width: f32,
}

impl<'a> ComboBox<'a> {
    /// Creates a combo box that sets `selected` to the index of the chosen
    /// option.
    pub fn new(selected: &'a mut usize, options: &'a [&'a str]) -> Self {
        Self { selected, options, width: 180.0 }
    }
    /// Sets the width of the button and its menu. The default is 180.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Shows the combo box. Returns `true` when the selection changed.
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
