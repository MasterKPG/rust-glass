//! Toasts: short messages at the top of the window.

use crate::animation::{POINTS, Spring, Tuning};
use crate::context::{Context, Id, Order};
use crate::geometry::{Rect, Vec2, lerp, smoothstep, vec2};
use crate::icon::Icon;
use crate::material::{Corner, Glass, GlassShape};
use crate::text::{TextStyle, Weight};

#[derive(Clone, Debug)]
pub(crate) struct ToastState {
    icon: Option<Icon>,
    text: String,
    born: Option<f64>,
    t: Spring,
    y: Spring,
    key: u64,
}

impl ToastState {
    pub(crate) fn new(icon: Option<Icon>, text: String) -> Self {
        Self { icon, text, born: None, t: Spring::new(0.0), y: Spring::default(), key: 0 }
    }
}

pub(crate) fn show_toasts(ctx: &mut Context) {
    let mut list = ctx.take_toasts();
    if list.is_empty() {
        return;
    }
    let layer = ctx.layer(Id::new("toasts"), Order::Toast);
    let now = ctx.time;
    let mut y = 24.0;
    let n = list.len();
    let mut shapes = Vec::new();
    let mut faces = Vec::new();
    let mut moving = false;
    let mut next_hide = f64::MAX;
    for (i, t) in list.iter_mut().enumerate() {
        if t.born.is_none() {
            t.born = Some(now);
            t.key = (now * 1000.0) as u64 + i as u64;
        }
        let age = now - t.born.unwrap_or(now);
        let show = age < 2.6 && i + 4 > n;
        let tune = ctx.tuning(if show { Tuning::new(0.46, 0.70) } else { Tuning::new(0.30, 1.0) });
        moving |= t.t.step(show as u32 as f32, tune, ctx.dt);
        moving |= t.y.step_within(y, ctx.tuning(Tuning::SMOOTH), ctx.dt, POINTS);
        if show {
            next_hide = next_hide.min(2.6 - age);
        }
        let l = ctx.text.layout(&t.text, &TextStyle::new(14.0).weight(Weight::Semibold));
        let iw = if t.icon.is_some() { 28.0 } else { 0.0 };
        let full = vec2(l.size.x + iw + 40.0, 46.0);
        // Starts as a small drop at the top edge.
        let size = vec2(lerp(46.0, full.x, t.t.value), lerp(34.0, full.y, t.t.value.min(1.1)));
        let cy = lerp(-30.0, t.y.value + 23.0, t.t.value);
        let r = Rect::from_center_size(vec2(ctx.screen.center().x, cy), size.max(Vec2::splat(8.0)));
        shapes.push(GlassShape::new(r, Corner::Capsule));
        faces.push((r, l, t.icon, t.t.value, iw));
        if show {
            y += 54.0;
        }
    }
    list.retain(|t| t.born.is_some_and(|b| now - b < 2.6) || t.t.value > 0.01);
    // A toast that rests needs no frames until it leaves.
    if moving || next_hide == f64::MAX {
        ctx.request_repaint();
    } else {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(next_hide.max(0.001)));
    }
    let mut g = Glass::group(shapes, ctx.theme.thick).fuse(5.0);
    g.material.shadow_radius = 22.0;
    g.material.blur = 6.0;
    let a = ctx.prepare_glass(Id::new("toasts"), &mut g, false);
    let c = a.surface().text_color();
    let mut p = ctx.painter(layer, Rect::EVERYTHING);
    p.glass(&g);
    for (r, l, icon, t, iw) in faces {
        let k = smoothstep(0.45, 0.95, t);
        let x = r.center().x - (l.size.x + iw) * 0.5;
        if let Some(i) = icon {
            p.icon(vec2(x + 9.0, r.center().y), 19.0, i, c.fade(k));
        }
        p.text(vec2(x + iw, r.center().y - l.size.y * 0.5), &l, c.fade(k));
    }
    ctx.put_toasts(list);
}
