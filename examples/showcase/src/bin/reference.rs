//! Renders the slider and segmented control at the scale of the reference
//! footage and plays the same gestures. Writes PNG frames.
//!
//! cargo run --release -p showcase --bin reference -- captures

use rust_glass::headless::Headless;
use rust_glass::input::Event;
use rust_glass::prelude::*;

const PPP: f32 = 3.33;

struct SliderScene {
    v: f32,
}

impl App for SliderScene {
    fn update(&mut self, ctx: &mut Context, _: &FrameInfo) {
        ctx.clear = Color::hex(0xD8D8D8);
        let c = ctx.center();
        let w = 953.0 / PPP;
        let r = Rect::from_center_size(c + vec2(-2.0, 2.0), vec2(w, 36.0));
        region(ctx, "s", r, |ui| {
            ui.add(
                Slider::new(&mut self.v, 0.0..=1.0)
                    .width(w)
                    .tint(Color::hex(0x318DE2))
                    .track(Color::hex(0xC4C4C4))
                    .knob(Color::hex(0xD9D9D9)),
            );
        });
    }
}

struct SegmentScene {
    sel: usize,
}

impl App for SegmentScene {
    fn update(&mut self, ctx: &mut Context, _: &FrameInfo) {
        ctx.clear = Color::hex(0xDCDCDC);
        let w = 717.0 / PPP;
        let h = 150.0 / PPP;
        let k = h / 34.0;
        let r = Rect::from_min_size(vec2(142.0, 88.0) / PPP, vec2(w, h));
        region(ctx, "s", r, |ui| {
            ui.add(
                Segmented::new(&mut self.sel, &["For You", "Library"])
                    .fit()
                    .width(w)
                    .scale(k)
                    .track(Color::hex(0xD0D0D0))
                    .knob(Color::hex(0xDDDDDD)),
            );
        });
    }
}

struct TabScene {
    sel: usize,
}

impl App for TabScene {
    fn update(&mut self, ctx: &mut Context, _: &FrameInfo) {
        ctx.clear = Color::hex(0xEFEFEF);
        let s = ctx.screen;
        let w = s.width();
        let mut p = ctx.background();
        let card = Rect { min: vec2(w * 0.052, -300.0), max: vec2(w * 0.948, 300.0) };
        p.gradient(s.expand(64.0), 0.0, Color::gray(0.92), Color::gray(0.55), vec2(0.0, 1.0));
        p.gradient(card, 0.0, Color::hex(0x09090B), Color::hex(0x4B4247), vec2(1.0, 0.0));
        p.shadow(
            Rect::from_center_size(vec2(w * 0.72, 40.0), vec2(90.0, 60.0)),
            Corner::Capsule,
            30.0,
            Color::white(0.45),
        );
        let leaf = Rect::from_center_size(vec2(w * 0.585, -8.0), vec2(80.0, 74.0));
        p.fill(leaf, Corner::Capsule, Color::hex(0xF4F0EA));
        p.arc(vec2(w * 0.2, 76.0), 78.0, 3.0, 3.5, 2.6, Color::hex(0xFF9A6A), Color::hex(0x4FC3FF));
        p.fill(Rect { min: vec2(w * 0.5, 74.0), max: vec2(w * 0.96, 300.0) }, 0.0, Color::gray(0.42));
        TabBar {
            selected: &mut self.sel,
            tabs: &[
                (Icon::Home, "Today"),
                (Icon::Plane, "Games"),
                (Icon::Layers, "Apps"),
                (Icon::Grid, "Arcade"),
                (Icon::Search, "Search"),
            ],
        }
        .show_at(ctx, Id::new("tabs"), vec2(w * 0.5, 106.0 / 2.135));
    }
}

/// Tab bar at the scale of the third clip, at one transparency setting.
fn tabs(dir: &str, name: &str, frost: f32) {
    let (w, hh, ppp) = (960, 230, 2.135);
    let mut h = Headless::new(w, hh, ppp).expect("gpu");
    h.ctx.theme.frost = frost;
    h.ctx.theme.adaptive = false;
    h.ctx.theme.accent = Color::hex(0x0068F0);
    let mut app = TabScene { sel: 4 };
    let dt = 1.0 / 60.0;
    let at = |x: f32| vec2(x, 106.0) / ppp;
    h.settle(&mut app, 1.0);
    save(&h, w, hh, &format!("{dir}/tab_{name}_rest.png"));
    h.step(&mut app, dt, &[Event::PointerMoved(at(808.0))]);
    h.step(&mut app, dt, &[Event::PointerDown]);
    for _ in 0..40 {
        h.step(&mut app, dt, &[]);
    }
    save(&h, w, hh, &format!("{dir}/tab_{name}_lift.png"));
    for i in 0..36 {
        let x = 808.0 - 253.0 * (i as f32 / 35.0);
        h.step(&mut app, dt, &[Event::PointerMoved(at(x))]);
    }
    for _ in 0..30 {
        h.step(&mut app, dt, &[]);
    }
    save(&h, w, hh, &format!("{dir}/tab_{name}_between.png"));
    for i in 0..12 {
        let x = 555.0 + 92.0 * (i as f32 / 11.0);
        h.step(&mut app, dt, &[Event::PointerMoved(at(x))]);
    }
    for _ in 0..40 {
        h.step(&mut app, dt, &[]);
    }
    save(&h, w, hh, &format!("{dir}/tab_{name}_on.png"));
}

fn save(h: &Headless, w: u32, hgt: u32, path: &str) {
    let px = h.pixels();
    image::save_buffer(path, &px, w, hgt, image::ColorType::Rgba8).expect("write png");
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "captures".into());
    std::fs::create_dir_all(&dir).expect("create output directory");

    let (w, hh) = (1080, 318);
    let mut h = Headless::new(w, hh, PPP).expect("gpu");
    println!("{}", h.describe());
    let mut app = SliderScene { v: 0.54 };
    h.settle(&mut app, 1.0);
    save(&h, w, hh, &format!("{dir}/slider_rest.png"));
    let dt = 1.0 / 60.0;
    let at = |x: f32| vec2(x, 160.0) / PPP;
    h.step(&mut app, dt, &[Event::PointerMoved(at(590.0))]);
    h.step(&mut app, dt, &[Event::PointerDown]);
    for i in 0..40 {
        h.step(&mut app, dt, &[]);
        if i == 4 {
            save(&h, w, hh, &format!("{dir}/slider_mid.png"));
        }
    }
    save(&h, w, hh, &format!("{dir}/slider_lift.png"));
    for i in 0..12 {
        h.step(&mut app, dt, &[Event::PointerMoved(at(590.0 - 30.0 * i as f32))]);
    }
    save(&h, w, hh, &format!("{dir}/slider_fast.png"));
    for i in 0..10 {
        h.step(&mut app, dt, &[]);
        if i == 7 {
            save(&h, w, hh, &format!("{dir}/slider_stop.png"));
        }
    }

    let (w, hh) = (1080, 334);
    let mut h = Headless::new(w, hh, PPP).expect("gpu");
    let mut app = SegmentScene { sel: 0 };
    h.settle(&mut app, 1.0);
    save(&h, w, hh, &format!("{dir}/seg_rest.png"));
    h.step(&mut app, dt, &[Event::PointerMoved(at(320.0))]);
    h.step(&mut app, dt, &[Event::PointerDown]);
    for _ in 0..40 {
        h.step(&mut app, dt, &[]);
    }
    save(&h, w, hh, &format!("{dir}/seg_lift.png"));
    for i in 0..30 {
        h.step(&mut app, dt, &[Event::PointerMoved(at(320.0 + 5.0 * i as f32))]);
    }
    save(&h, w, hh, &format!("{dir}/seg_drag.png"));
    for i in 0..12 {
        h.step(&mut app, dt, &[Event::PointerMoved(at(470.0 + 25.0 * i as f32))]);
    }
    save(&h, w, hh, &format!("{dir}/seg_fast.png"));
    h.step(&mut app, dt, &[Event::PointerUp]);
    h.settle(&mut app, 2.0);
    save(&h, w, hh, &format!("{dir}/seg_done.png"));
    println!("selected {}", app.sel);

    for (name, frost) in [("clear", 0.0), ("default", Theme::FROST), ("tinted", 1.0)] {
        tabs(&dir, name, frost);
    }
}
