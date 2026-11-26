//! Alerts, modal panels and sheets.

use crate::animation::{Spring, Tuning};
use crate::color::Color;
use crate::context::{Context, Id, Order, Sense};
use crate::geometry::{Rect, Vec2, lerp, smoothstep, vec2};
use crate::input::Key;
use crate::material::{Corner, Glass};
use crate::text::TextStyle;
use crate::ui::Ui;
use crate::widgets::button::{Button, ButtonKind};
use crate::widgets::press::rubber_band;

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
        let c = ui.surface.secondary_color();
        let st = TextStyle::new(ui.ctx.theme.body_size).align(crate::text::TextAlign::Center);
        ui.styled_text(message, st, c);
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
        s.t = Spring::new(0.0);
        s.init = true;
    }
    let tune = if *open { Tuning::new(0.42, 0.72) } else { Tuning::new(0.28, 1.0) };
    if s.t.step(*open as u32 as f32, ctx.tuning(tune), ctx.dt) {
        ctx.request_repaint();
    }
    let t = s.t.value;
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
    ui.max_rect.max.y = 1.0e5;
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
        s.y = Spring::new(1.0);
        s.init = true;
    }
    let screen = ctx.screen;
    let travel = height + 40.0;
    let layer = ctx.layer(id, Order::Modal);
    let target = if *open { s.drag / travel } else { 1.0 };
    let tune = if s.drag != 0.0 { Tuning::new(0.08, 1.0) } else { Tuning::new(0.42, 0.82) };
    if s.y.step(target, ctx.tuning(tune), ctx.dt) {
        ctx.request_repaint();
    }
    if s.y.value > 0.998 && !*open {
        s.drag = 0.0;
        ctx.set(id, s);
        return;
    }
    let k = (1.0 - s.y.value).clamp(0.0, 1.0);
    let top = screen.max.y - height - 14.0 + s.y.value * travel;
    let r = Rect::from_min_size(vec2(screen.center().x - width * 0.5, top), vec2(width, height));
    let dim = ctx.interact(id.with("dim"), screen, Rect::EVERYTHING, layer, Sense::Click);
    ctx.interact(id.with("block"), r, Rect::EVERYTHING, layer, Sense::Hover);
    let grip = Rect::from_min_size(r.min, vec2(width, 34.0));
    let gr = ctx.interact(id.with("grip"), grip, Rect::EVERYTHING, layer, Sense::Drag);
    if gr.held {
        let d = ctx.pointer_or(Vec2::ZERO).y - ctx.input.press_origin.y;
        s.drag = if d < 0.0 { rubber_band(d, 24.0) } else { d };
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
    let stretch = (-s.y.value * travel).max(0.0);
    let body = Rect { min: r.min, max: vec2(r.max.x, r.max.y + stretch) };
    let mut g = Glass::new(body, Corner::Radius(34.0), ctx.theme.thick).presence(k.max(0.4));
    let a = ctx.prepare_glass(id, &mut g, false);
    let mut p = ctx.painter(layer, screen.expand(64.0));
    p.fill(screen.expand(64.0), 0.0, Color::black(0.22 * k));
    p.glass(&g);
    let c = a.surface().tertiary_color();
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
