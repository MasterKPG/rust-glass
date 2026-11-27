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
        ctx.clear_color = Color::hex(0xD8D8D8);
        let c = ctx.screen.center();
        let w = 953.0 / PPP;
        let r = Rect::from_center_size(c + vec2(-2.0, 2.0), vec2(w, 36.0));
        region(ctx, "s", r, |ui| {
            ui.add(
                Slider::new(&mut self.v, 0.0..=1.0)
                    .width(w)
                    .tint(Color::hex(0x318DE2))
                    .track_color(Color::hex(0xC4C4C4))
                    .thumb_color(Color::hex(0xD9D9D9)),
            );
        });
    }
}

struct SegmentScene {
    sel: usize,
}

impl App for SegmentScene {
    fn update(&mut self, ctx: &mut Context, _: &FrameInfo) {
        ctx.clear_color = Color::hex(0xDCDCDC);
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
                    .track_color(Color::hex(0xD0D0D0))
                    .thumb_color(Color::hex(0xDDDDDD)),
            );
        });
    }
}

struct TabScene {
    sel: usize,
}

impl App for TabScene {
    fn update(&mut self, ctx: &mut Context, _: &FrameInfo) {
        ctx.clear_color = Color::hex(0xEFEFEF);
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
        TabBar::new(
            &mut self.sel,
            &[
                (Icon::Home, "Today"),
                (Icon::Plane, "Games"),
                (Icon::Layers, "Apps"),
                (Icon::Grid, "Arcade"),
                (Icon::Search, "Search"),
            ],
        )
        .show_at(ctx, Id::new("tabs"), vec2(w * 0.5, 106.0 / 2.135));
    }
}

/// A tab bar over App Store like content, at the scale of the iOS 27 clips
/// in `references/Liquid Glass iOS27 reference`.
struct StoreScene {
    sel: usize,
    dark: bool,
}

impl App for StoreScene {
    fn update(&mut self, ctx: &mut Context, _: &FrameInfo) {
        ctx.theme.dark = self.dark;
        let (bg, fg) = if self.dark { (Color::BLACK, Color::WHITE) } else { (Color::WHITE, Color::BLACK) };
        ctx.clear_color = bg;
        let w = ctx.screen.width();
        let mut p = ctx.background();
        p.fill(
            Rect::from_min_size(vec2(24.0, -10.0), vec2(70.0, 44.0)),
            Corner::Capsule,
            Color::hex(0xF5A31B),
        );
        p.fill(Rect::from_min_size(vec2(40.0, -4.0), vec2(36.0, 30.0)), Corner::Capsule, bg);
        let big = ctx.text.layout("This week's favourites", &TextStyle::new(19.0).weight(Weight::Bold));
        let top = ctx.text.layout("with friends", &TextStyle::new(12.0).weight(Weight::Semibold));
        let small = ctx.text.layout("Share the moment", &TextStyle::new(8.0).weight(Weight::Semibold));
        let mut p = ctx.background();
        p.text(vec2(24.0, 88.0), &big, fg);
        p.text(vec2(104.0, -4.0), &top, fg);
        p.text(vec2(104.0, 14.0), &small, Color::gray(0.55));
        TabBar::new(
            &mut self.sel,
            &[
                (Icon::Home, "Today"),
                (Icon::Plane, "Games"),
                (Icon::Layers, "Apps"),
                (Icon::Grid, "Arcade"),
                (Icon::Search, "Search"),
            ],
        )
        .show_at(ctx, Id::new("tabs"), vec2(w * 0.5, STORE_BAR_Y));
    }
}

const STORE_PPP: f32 = 2.6;
const STORE_BAR_Y: f32 = 66.0;

/// Plays a press, a hold, a drag across two tabs and a release, and writes
/// every other frame (30 per second) to `dir/store_NAME/`.
fn store(dir: &str, dark: bool) {
    let (w, hh) = (1180, 330);
    let mut h = Headless::new(w, hh, STORE_PPP).expect("gpu");
    h.ctx.theme.adaptive = false;
    h.ctx.theme.accent = Color::hex(0x0A84FF);
    let mut app = StoreScene { sel: 2, dark };
    let name = if dark { "dark" } else { "light" };
    let folder = format!("{dir}/store_{name}");
    std::fs::create_dir_all(&folder).expect("create output directory");
    let dt = 1.0 / 60.0;
    let cx = w as f32 / STORE_PPP * 0.5;
    let tab = |i: f32| vec2(cx + (i - 2.0) * 77.0, STORE_BAR_Y);
    h.settle(&mut app, 1.0);
    let mut frame = 0;
    let mut run = |h: &mut Headless, app: &mut StoreScene, events: Vec<Event>| {
        h.step(app, dt, &events);
        if frame % 2 == 0 {
            save(h, w, hh, &format!("{folder}/{:04}.png", frame / 2));
        }
        frame += 1;
    };
    let ease = |t: f32| t * t * (3.0 - 2.0 * t);
    run(&mut h, &mut app, vec![Event::PointerMoved(tab(2.0))]);
    for _ in 0..9 {
        run(&mut h, &mut app, vec![]);
    }
    run(&mut h, &mut app, vec![Event::PointerDown]);
    for _ in 0..23 {
        run(&mut h, &mut app, vec![]);
    }
    // Two tabs to the left in about half a second, as in the clip.
    for i in 1..=32 {
        let t = ease(i as f32 / 32.0);
        run(&mut h, &mut app, vec![Event::PointerMoved(tab(2.0 - 2.0 * t))]);
    }
    for _ in 0..30 {
        run(&mut h, &mut app, vec![]);
    }
    for i in 1..=16 {
        let t = ease(i as f32 / 16.0);
        run(&mut h, &mut app, vec![Event::PointerMoved(tab(1.0 * t))]);
    }
    run(&mut h, &mut app, vec![Event::PointerUp]);
    for _ in 0..50 {
        run(&mut h, &mut app, vec![]);
    }
    // A tap on another tab.
    run(&mut h, &mut app, vec![Event::PointerMoved(tab(3.0))]);
    run(&mut h, &mut app, vec![Event::PointerDown]);
    for _ in 0..5 {
        run(&mut h, &mut app, vec![]);
    }
    run(&mut h, &mut app, vec![Event::PointerUp]);
    for _ in 0..60 {
        run(&mut h, &mut app, vec![]);
    }
}

/// Tab bar at the scale of the third clip, at one transparency setting.
fn tabs(dir: &str, name: &str, frost: f32) {
    let (w, hh, ppp) = (960, 230, 2.135);
    let mut h = Headless::new(w, hh, ppp).expect("gpu");
    h.ctx.theme.glass_tint = frost;
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
    if std::env::args().nth(2).as_deref() == Some("store") {
        store(&dir, false);
        store(&dir, true);
        return;
    }

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

    for (name, frost) in [("clear", 0.0), ("default", Theme::DEFAULT_GLASS_TINT), ("tinted", 1.0)] {
        tabs(&dir, name, frost);
    }
}
