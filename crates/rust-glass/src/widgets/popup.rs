//! Popups and tooltips. A popup grows out of the control that opened it.

use crate::animation::{Spring, SpringRect, Tuning};
use crate::context::{Context, Id, Order, Response, Sense};
use crate::geometry::{Rect, Vec2, lerp, smoothstep, vec2};
use crate::input::Key;
use crate::material::{Corner, Glass, Material};
use crate::paint::Painter;
use crate::text::{TextStyle, Weight};
use crate::ui::Ui;

#[derive(Clone, Copy, Default)]
struct PopupState {
    shape: SpringRect,
    t: Spring,
    size: Vec2,
    live: bool,
    anchor: Vec2,
    /// Frame in which it opened, so the opening click does not close it.
    born: u64,
}

/// How far a popup has formed, 0 to 1. The anchor fades out with it.
pub fn popup_progress(ctx: &mut Context, id: Id) -> f32 {
    let s: PopupState = ctx.get(id.with("popup"));
    if s.live { s.t.value.clamp(0.0, 1.0) } else { 0.0 }
}

/// Glass bubble that grows out of `anchor` while [`Context::popup_open`] is set
/// for `id`, and flows back when it closes.
pub fn popup(ctx: &mut Context, id: Id, anchor: Rect, width: f32, f: impl FnOnce(&mut Ui)) {
    let open = ctx.popup_open(id);
    let key = id.with("popup");
    let mut s: PopupState = ctx.get(key);
    if !open && !s.live {
        return;
    }
    if open && !s.live {
        s = PopupState {
            shape: SpringRect::new(anchor),
            t: Spring::new(0.0),
            size: s.size,
            live: true,
            anchor: anchor.min,
            born: ctx.frame,
        };
    }
    // Stay glued to an anchor that scrolls or moves with its window.
    let carried = anchor.min - s.anchor;
    s.anchor = anchor.min;
    s.shape.center.x.value += carried.x;
    s.shape.center.y.value += carried.y;
    let dt = ctx.dt;
    let size = if s.size == Vec2::ZERO { vec2(width, 60.0) } else { s.size };
    let full = place(anchor, size, ctx.screen);
    let target = if open { full } else { anchor };
    let (pt, st) = if open {
        (ctx.tuning(Tuning::new(0.36, 0.80)), ctx.tuning(Tuning::new(0.40, 0.72)))
    } else {
        (ctx.tuning(Tuning::new(0.30, 0.95)), ctx.tuning(Tuning::new(0.30, 0.95)))
    };
    let mut moving = s.shape.step(target, pt, st, dt);
    moving |= s.t.step(open as u32 as f32, ctx.tuning(Tuning::new(if open { 0.30 } else { 0.24 }, 1.0)), dt);
    // A closing popup is gone once it has faded out, even if its shape is
    // still a fraction of a point from the anchor.
    if !open && (!moving || s.t.value < 0.01) {
        s.live = false;
        ctx.set(key, s);
        ctx.request_repaint();
        return;
    }
    if moving {
        ctx.request_repaint();
    }
    let t = s.t.value.clamp(0.0, 1.0);
    let shape = s.shape.rect();
    let layer = ctx.layer(key, Order::Popup);
    ctx.interact(key.with("block"), shape, Rect::EVERYTHING, layer, Sense::Hover);

    let m = ctx.theme.regular.lerp(&ctx.theme.thick, t);
    let radius = lerp(anchor.height() * 0.5, 22.0, t).min(shape.height() * 0.5);
    // The bubble fades in and out over the same stretch in which the
    // control that opened it fades the other way, so the two cross over
    // instead of the bubble lingering on top.
    let presence = popup_presence(t);
    let mut g = Glass::new(shape, Corner::Radius(radius), m).presence(presence);
    let a = ctx.prepare_glass(key, &mut g, false);
    ctx.painter(layer, Rect::EVERYTHING).glass(&g);

    // Content is laid out at full size and revealed as the bubble opens.
    let show = smoothstep(0.30, 0.85, t);
    let mut used = size;
    if open || show > 0.01 {
        let clip = shape.shrink(1.0);
        let offset = (shape.center() - full.center()) * 0.6;
        let r = full.translate(offset);
        let mut ui =
            Ui::new(ctx, layer, key, Rect { min: r.min + vec2(8.0, 8.0), max: r.max - vec2(8.0, 8.0) });
        ui.max_rect.max.y = 1.0e5;
        ui.clip = clip;
        ui.opacity = show;
        ui.surface = a.surface();
        ui.spacing = 2.0;
        f(&mut ui);
        used = vec2(width, ui.used().height() + 16.0);
    }
    // The content comes into focus as the bubble opens. The blur fades with
    // the bubble, so it never outlasts it.
    let blur = BLUR_WHILE_FORMING * (1.0 - smoothstep(0.30, 0.95, t));
    let inside = Corner::Radius((radius - 1.0).max(0.0));
    forming_blur(&mut ctx.painter(layer, Rect::EVERYTHING), shape.shrink(1.0), inside, blur, presence);
    if (used - s.size).length() > 0.5 {
        s.size = used;
        ctx.request_repaint();
    }
    let young = ctx.frame <= s.born + 1;
    ctx.set(key, s);
    if open && !young {
        let outside = ctx.pressed_outside(shape.union(anchor));
        if outside || ctx.input.take_key(Key::Escape) {
            ctx.set_popup(id, false);
        }
    }
}

/// Blur radius of the content of an overlay that has just started to form.
pub(crate) const BLUR_WHILE_FORMING: f32 = 10.0;

/// How present the glass of a popup is at progress `t`. A menu button fades
/// its own glass with `1 - popup_presence(t)`.
pub(crate) fn popup_presence(t: f32) -> f32 {
    smoothstep(0.0, 0.4, t)
}

/// Blurs what is under `rect` by `radius`, mixed in at `presence`, so the
/// blur of an overlay fades out together with the overlay.
pub(crate) fn forming_blur(p: &mut Painter, rect: Rect, corner: Corner, radius: f32, presence: f32) {
    if radius < 0.5 || presence <= 0.001 {
        return;
    }
    let opacity = std::mem::replace(&mut p.opacity, 1.0);
    p.glass(&Glass::new(rect, corner, Material::blur(radius)).presence(presence));
    p.opacity = opacity;
}

/// Below the anchor if it fits, otherwise above, always inside the screen.
fn place(anchor: Rect, size: Vec2, screen: Rect) -> Rect {
    let below = anchor.max.y + 6.0 + size.y <= screen.max.y - 8.0;
    let y = if below || anchor.min.y - size.y < 8.0 { anchor.min.y } else { anchor.max.y - size.y };
    let x = if anchor.center().x > screen.center().x { anchor.max.x - size.x } else { anchor.min.x };
    Rect::from_min_size(vec2(x, y), size).clamp_inside(screen.shrink(8.0))
}

/// Shows `text` in a small bubble above `anchor` after the pointer rests on
/// it. Call it every frame the anchor is hovered.
pub fn tooltip(ctx: &mut Context, id: Id, anchor: Rect, text: &str) {
    if text.is_empty() {
        return;
    }
    let key = id.with("tooltip");
    #[derive(Clone, Copy, Default)]
    struct Since {
        t: f64,
        frame: u64,
    }
    let mut s: Since = ctx.get(key);
    if s.frame + 1 < ctx.frame {
        s.t = ctx.time;
    }
    s.frame = ctx.frame;
    ctx.set(key, s);
    let wait = 0.45;
    if ctx.input.down {
        return;
    }
    let t = ((ctx.time - s.t - wait) / 0.16).clamp(0.0, 1.0) as f32;
    if t < 1.0 {
        ctx.request_repaint();
    }
    if t <= 0.0 {
        return;
    }
    let l = ctx.text.layout(text, &TextStyle::new(12.0).weight(Weight::Medium));
    let size = l.size + vec2(22.0, 13.0);
    let mut c = vec2(anchor.center().x, anchor.min.y - 8.0 - size.y * 0.5);
    if c.y - size.y * 0.5 < 6.0 {
        c.y = anchor.max.y + 8.0 + size.y * 0.5;
    }
    let r = Rect::from_center_size(c, size).clamp_inside(ctx.screen.shrink(6.0));
    let e = smoothstep(0.0, 1.0, t);
    let r = r.scale_from_center(0.9 + 0.1 * e, 0.9 + 0.1 * e);
    let layer = ctx.layer(Id::new("tooltip"), Order::Tooltip);
    let mut g = Glass::new(r, Corner::Capsule, ctx.theme.thick).presence(e);
    g.material.shadow_radius = 14.0;
    g.material.shadow_y = 5.0;
    let a = ctx.prepare_glass(key, &mut g, false);
    let mut p = ctx.painter(layer, Rect::EVERYTHING);
    p.glass(&g);
    p.text_centered(r, &l, a.surface().text_color().fade(e), 1.0);
}

impl Response {
    /// Show a tooltip while hovered.
    pub fn on_hover_text(self, ui: &mut Ui, text: &str) -> Response {
        if self.hovered {
            tooltip(ui.ctx, self.id, self.rect, text);
        }
        self
    }
}
