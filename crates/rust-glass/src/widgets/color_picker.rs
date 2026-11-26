//! Color picker.

use crate::accessibility::Role;
use crate::animation::Tuning;
use crate::color::Color;
use crate::context::{Cursor, Response, Sense};
use crate::geometry::{Rect, Vec2, vec2};
use crate::material::{Corner, Glass};
use crate::ui::{Ui, Widget};
use crate::widgets::press::PressEffect;

/// A color swatch that opens a picker when clicked.
pub struct ColorPicker<'a> {
    color: &'a mut Color,
}

impl<'a> ColorPicker<'a> {
    /// Creates a picker that edits `color`.
    pub fn new(color: &'a mut Color) -> Self {
        Self { color }
    }
}

impl Widget for ColorPicker<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let mut r = ui.allocate_response(vec2(46.0, 30.0), Sense::Click);
        ui.ctx.focusable(r.id);
        if let Some(n) = ui.access(r.id, Role::ColorWell, r.rect) {
            n.label("Color").value(format!("#{:06X}", self.color.to_hex() >> 8));
        }
        if r.hovered {
            ui.ctx.cursor = Cursor::Pointer;
        }
        let gel = PressEffect::update(ui.ctx, &r, 1.0);
        if r.clicked {
            ui.ctx.toggle_popup(r.id);
        }
        let mut m = ui.ctx.theme.regular;
        m.tint = Some(self.color.alpha(0.95));
        m.tint_follow = 0.4;
        let mut g = Glass::new(gel.rect, Corner::Capsule, m);
        g.brightness = 0.05 * gel.amount.max(0.0);
        ui.painter().glass(&g);

        #[derive(Clone, Copy, Default)]
        struct Hue {
            h: f32,
            hex: u32,
        }
        let mut hs: Hue = ui.ctx.get(r.id);
        if hs.hex != self.color.to_hex() {
            let (h, s, _) = self.color.to_hsv();
            // Keep the hue when the color has none.
            if s > 0.01 {
                hs.h = h;
            }
        }
        let before = self.color.to_hex();
        let color = &mut *self.color;
        crate::widgets::popup::popup(ui.ctx, r.id, gel.rect, 244.0, |ui| {
            let (_, mut s, mut v) = color.to_hsv();
            let pad = ui.allocate_response(vec2(212.0, 140.0), Sense::Drag);
            if pad.held
                && let Some(p) = pad.pointer
            {
                s = ((p.x - pad.rect.min.x) / pad.rect.width()).clamp(0.0, 1.0);
                v = 1.0 - ((p.y - pad.rect.min.y) / pad.rect.height()).clamp(0.0, 1.0);
            }
            let base = Color::from_hsv(hs.h, 1.0, 1.0);
            let dot = pad.rect.min + vec2(s * pad.rect.width(), (1.0 - v) * pad.rect.height());
            let lift = ui.ctx.animate_bool(pad.id, pad.held, Tuning::PRESS);
            let lens = ui.ctx.theme.lens;
            let mut p = ui.painter();
            p.fill(pad.rect, 14.0, base);
            p.gradient(pad.rect, 14.0, Color::WHITE, Color::white(0.0), vec2(1.0, 0.0));
            p.gradient(pad.rect, 14.0, Color::black(0.0), Color::BLACK, vec2(0.0, 1.0));
            let d = Rect::from_center_size(dot, Vec2::splat(20.0 + 14.0 * lift));
            p.stroke(d, Corner::Capsule, 2.0, Color::white(0.95));
            p.glass(&Glass::new(d, Corner::Capsule, lens));

            let bar = ui.allocate_response(vec2(212.0, 22.0), Sense::Drag);
            if bar.held
                && let Some(p) = bar.pointer
            {
                hs.h = ((p.x - bar.rect.min.x - 8.0) / (bar.rect.width() - 16.0)).clamp(0.0, 0.999);
            }
            let lift = ui.ctx.animate_bool(bar.id, bar.held, Tuning::PRESS);
            let mut p = ui.painter();
            let track = bar.rect.shrink2(vec2(0.0, 6.0));
            let n = 6;
            let tm = p.add_mask(track, Corner::Capsule, false, Default::default());
            p.with_mask(tm, |p| {
                for i in 0..n {
                    let w = track.width() / n as f32;
                    let seg = Rect::from_min_size(
                        track.min + vec2(w * i as f32, 0.0),
                        vec2(w + 0.5, track.height()),
                    );
                    let a = Color::from_hsv(i as f32 / n as f32, 0.95, 1.0);
                    let b = Color::from_hsv((i + 1) as f32 / n as f32, 0.95, 1.0);
                    p.gradient(seg, 0.0, a, b, vec2(1.0, 0.0));
                }
            });
            let x = bar.rect.min.x + 8.0 + hs.h * (bar.rect.width() - 16.0);
            let d = Rect::from_center_size(
                vec2(x, bar.rect.center().y),
                vec2(26.0 + 8.0 * lift, 20.0 + 8.0 * lift),
            );
            p.glass(&Glass::new(d, Corner::Capsule, lens));
            if pad.held || bar.held {
                *color = Color::from_hsv(hs.h, s, v);
            }
            let hex = format!("#{:06X}", color.to_hex());
            ui.caption(&hex);
        });
        hs.hex = self.color.to_hex();
        ui.ctx.set(r.id, hs);
        r.changed = before != self.color.to_hex();
        r
    }
}

impl Ui<'_> {
    /// Adds a color swatch that opens a picker.
    pub fn color_picker(&mut self, color: &mut Color) -> Response {
        self.add(ColorPicker::new(color))
    }
}
