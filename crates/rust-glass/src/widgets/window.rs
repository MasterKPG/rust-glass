//! Floating windows.

use crate::animation::{Spring, Tuning};
use crate::context::{Context, Id, Order, Sense};
use crate::geometry::{Rect, Vec2, lerp, smoothstep, vec2};
use crate::icon::Icon;
use crate::material::{Corner, Glass};
use crate::text::{TextStyle, Weight};
use crate::ui::Ui;
use crate::widgets::press::PressEffect;

/// Draggable, collapsible glass window.
pub struct Window<'a> {
    title: &'a str,
    pos: Vec2,
    width: f32,
    open: Option<&'a mut bool>,
}

impl<'a> Window<'a> {
    /// Creates a window. The title also identifies it, so keep it unique.
    pub fn new(title: &'a str) -> Self {
        Self { title, pos: vec2(80.0, 80.0), width: 280.0, open: None }
    }
    /// Sets where the window first appears.
    pub fn pos(mut self, position: Vec2) -> Self {
        self.pos = position;
        self
    }
    /// Sets the width. The height follows the content.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
    /// Adds a close button that sets `open` to `false`. The window shows
    /// while `open` is `true`.
    pub fn open(mut self, open: &'a mut bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Shows the window with the content drawn by `f`.
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
            t: Spring::new(0.0),
            fold: Spring::new(0.0),
            folded: false,
        });
        let open = self.open.as_deref().copied().unwrap_or(true);
        let tune = if open { Tuning::new(0.44, 0.70) } else { Tuning::new(0.26, 1.0) };
        if s.t.step(open as u32 as f32, ctx.tuning(tune), ctx.dt) {
            ctx.request_repaint();
        }
        if s.t.value < 0.005 && !open {
            ctx.set(id, s);
            return;
        }
        let layer = ctx.layer(id, Order::Window);
        let bar_h = 46.0;
        if s.fold.step(s.folded as u32 as f32, ctx.tuning(Tuning::new(0.40, 0.80)), ctx.dt) {
            ctx.request_repaint();
        }
        let full_h = s.h.max(bar_h);
        let h = lerp(full_h, bar_h, s.fold.value);
        let k = s.t.value;
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
        r = Rect::from_min_size(s.pos, r.size()).snap(ctx.pixels_per_point);
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
        p.text_centered(Rect::from_min_size(r.min, vec2(self.width, bar_h)), &title, surf.text_color(), 1.0);

        // Close and collapse buttons.
        let mk = |x: f32| Rect::from_center_size(vec2(x, r.min.y + bar_h * 0.5), Vec2::splat(24.0));
        let mut close = false;
        let mut spots = vec![(mk(r.max.x - 24.0), 1u8)];
        if self.open.is_some() {
            spots.push((mk(r.min.x + 24.0), 0u8));
        }
        for (cr, what) in spots {
            let res = ctx.interact(id.with(("btn", what)), cr, Rect::EVERYTHING, layer, Sense::Click);
            let gel = PressEffect::update(ctx, &res, 1.0);
            let mut p = ctx.painter(layer, Rect::EVERYTHING);
            p.opacity = op;
            p.fill(gel.rect, Corner::Capsule, surf.fill_color().fade(1.0 + gel.hover));
            if what == 0 {
                p.icon(gel.rect.center(), 13.0 * gel.scale, Icon::Close, surf.text_color());
                close |= res.clicked;
            } else {
                let i = if s.folded { Icon::ChevronDown } else { Icon::ChevronUp };
                p.icon(gel.rect.center(), 13.0 * gel.scale, i, surf.text_color());
                if res.clicked {
                    s.folded = !s.folded;
                }
            }
        }
        if s.fold.value < 0.995 {
            let inner = Rect { min: vec2(r.min.x + 18.0, r.min.y + bar_h), max: vec2(r.max.x - 18.0, 1.0e5) };
            let mut ui = Ui::new(ctx, layer, id, inner);
            ui.clip = Rect { min: body.min, max: vec2(body.max.x, r.min.y + h) };
            ui.surface = surf;
            ui.opacity = op * (1.0 - s.fold.value).clamp(0.0, 1.0);
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
