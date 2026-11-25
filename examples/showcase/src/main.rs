//! Every component of the framework on one screen.
//!
//! cargo run --release -p showcase
//!
//! `--capture DIR` renders all pages to PNG without a window and prints timings.
//! `--preview DIR` renders the pictures of the preview folder.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod art;
mod pages;

use rust_glass::headless::Headless;
use rust_glass::input::Event;
use rust_glass::prelude::*;

pub const PAGES: &[(Icon, &str)] = &[
    (Icon::Music, "Player"),
    (Icon::Drop, "Buttons"),
    (Icon::Sliders, "Sliders & Switches"),
    (Icon::Layers, "Segments & Tabs"),
    (Icon::Keyboard, "Text & Fields"),
    (Icon::Ellipsis, "Menus & Pickers"),
    (Icon::Bell, "Feedback"),
    (Icon::Window, "Windows & Sheets"),
    (Icon::Gear, "Settings List"),
    (Icon::Palette, "Material Lab"),
    (Icon::Eye, "Reference"),
];

pub struct Showcase {
    pub page: usize,
    pub wall: usize,
    wall_loaded: Option<usize>,
    wall_image: ImageId,
    wall_aspect: f32,
    custom: Option<(Vec<u8>, u32, u32)>,
    pub covers: Vec<ImageId>,
    pub appearance: usize,
    pub s: pages::State,
}

impl Showcase {
    fn new() -> Self {
        Self {
            page: 0,
            wall: 0,
            wall_loaded: None,
            wall_image: ImageId(0),
            wall_aspect: 1.5,
            custom: None,
            covers: Vec::new(),
            appearance: 0,
            s: pages::State::default(),
        }
    }

    fn wallpaper(&mut self, ctx: &mut Context) {
        if let Some((px, w, h)) = self.custom.take() {
            self.wall_image = ctx.add_image(w, h, px);
            self.wall_aspect = w as f32 / h as f32;
            self.wall = art::WALLPAPERS.len();
            self.wall_loaded = Some(self.wall);
        } else if self.wall_loaded != Some(self.wall) && self.wall < art::WALLPAPERS.len() {
            let (w, h) = (1800, 1200);
            self.wall_image = ctx.add_image(w, h, art::WALLPAPERS[self.wall].render(w, h));
            self.wall_aspect = 1.5;
            self.wall_loaded = Some(self.wall);
        }
        let wall_dark = art::WALLPAPERS.get(self.wall).is_some_and(|w| w.dark);
        ctx.theme.dark = match self.appearance {
            0 => wall_dark,
            1 => false,
            _ => true,
        };
        // Cover the window, keep the proportions.
        let size = ctx.screen.size();
        let aspect = size.x / size.y.max(1.0);
        let (u, v) = if aspect > self.wall_aspect {
            (1.0, self.wall_aspect / aspect)
        } else {
            (aspect / self.wall_aspect, 1.0)
        };
        let uv = Rect::from_center_size(vec2(0.5, 0.5), vec2(u, v));
        let screen = ctx.screen;
        // The scene texture is slightly larger than the window; stretch into it.
        let pad = vec2(64.0, 64.0);
        let r = Rect { min: screen.min, max: screen.max + pad };
        let grow = vec2(uv.width() * pad.x / size.x.max(1.0), uv.height() * pad.y / size.y.max(1.0));
        let uv = Rect { min: uv.min, max: uv.max + grow };
        let id = self.wall_image;
        ctx.background().image(r, 0.0, id, uv, Color::WHITE);
    }

    fn sidebar(&mut self, ctx: &mut Context, info: &FrameInfo, r: Rect) {
        Panel::new("sidebar", r).corner(26.0).padding(12.0).show(ctx, |ui| {
            ui.padded(vec2(8.0, 4.0), |ui| ui.subheading("rust-glass"));
            nav_list(ui, &mut self.page, PAGES);
            let foot = Rect { min: vec2(ui.max.min.x, ui.max.max.y - 172.0), max: ui.max.max };
            ui.in_rect("foot", foot, |ui| {
                ui.spacing = 7.0;
                ui.separator();
                ui.caption("Glass: clear to tinted");
                let mut frost = ui.ctx.theme.frost;
                if ui.slider(&mut frost, 0.0..=1.0).changed() {
                    ui.ctx.theme.frost = frost;
                }
                ui.add(
                    Segmented::new(&mut self.appearance, &["Auto", "Light", "Dark"])
                        .width(ui.available_width()),
                );
                let names: Vec<&str> = art::WALLPAPERS.iter().map(|w| w.name).collect();
                ComboBox { selected: &mut self.wall, options: &names, width: ui.available_width() }.show(ui);
                let g = info.gpu;
                let fps = if info.interval_ms > 0.1 { 1000.0 / info.interval_ms } else { 0.0 };
                let t = format!(
                    "{fps:.0} fps   ui {:.2}   gpu {:.2} ms   {} passes",
                    info.ui_ms, g.gpu_ms, g.passes
                );
                let c = ui.surface.secondary();
                ui.text_with(&t, TextStyle::new(10.5), c);
            });
        });
    }
}

impl App for Showcase {
    fn setup(&mut self, ctx: &mut Context) {
        self.covers = (0..12).map(|i| ctx.add_image(280, 280, art::cover(i, 280))).collect();
    }

    fn update(&mut self, ctx: &mut Context, info: &FrameInfo) {
        self.wallpaper(ctx);
        let screen = ctx.screen;
        let m = 14.0;
        let side = Rect { min: vec2(m, m), max: vec2(m + 232.0, screen.max.y - m) };
        let main = Rect { min: vec2(side.max.x + m, 0.0), max: screen.max };
        pages::show(self, ctx, main);
        self.sidebar(ctx, info, side);
    }

    fn file_dropped(&mut self, ctx: &mut Context, path: &std::path::Path) {
        match image::open(path) {
            Ok(img) => {
                let img = img.resize(2400, 2400, image::imageops::FilterType::Triangle).to_rgba8();
                let (w, h) = img.dimensions();
                self.custom = Some((img.into_raw(), w, h));
            }
            Err(e) => ctx.toast(Some(Icon::Warning), format!("Cannot open image: {e}")),
        }
        ctx.request_repaint();
    }
}

fn capture(dir: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let (w, h) = (1770u32, 1170u32);
    let mut hl = Headless::new(w, h, 1.5)?;
    let mut app = Showcase::new();
    app.setup(&mut hl.ctx);
    let mut report = format!("{} at {w} x {h}\n", hl.describe());
    for (i, p) in PAGES.iter().enumerate() {
        app.page = i;
        app.wall = i % 5;
        hl.settle(&mut app, 1.5);
        // Average over steady frames.
        let (mut ui, mut enc) = (0.0, 0.0);
        for _ in 0..30 {
            hl.step(&mut app, 1.0 / 60.0, &[]);
            ui += hl.info.ui_ms / 30.0;
            enc += hl.info.encode_ms / 30.0;
        }
        let g = hl.info.gpu;
        report += &format!(
            "{:<20} ui {ui:.2} ms, encode {enc:.2} ms, gpu {:.2} ms, {} passes, {} glass, {} shapes, {} kpx captured\n",
            p.1,
            g.gpu_ms,
            g.passes,
            g.glasses,
            g.shapes,
            g.captured_px / 1000
        );
        let path = format!("{dir}/page_{i:02}.png");
        image::save_buffer(&path, &hl.pixels(), w, h, image::ColorType::Rgba8).map_err(|e| e.to_string())?;
    }
    // Interaction stills.
    let shot = |hl: &mut Headless,
                app: &mut Showcase,
                name: &str,
                page: usize,
                wall: usize,
                ev: &[(Vec<Event>, usize)]| {
        app.page = page;
        app.wall = wall;
        hl.settle(app, 1.0);
        for (e, n) in ev {
            hl.step(app, 1.0 / 60.0, e);
            for _ in 0..*n {
                hl.step(app, 1.0 / 60.0, &[]);
            }
        }
        let path = format!("{dir}/{name}.png");
        let r = image::save_buffer(&path, &hl.pixels(), w, h, image::ColorType::Rgba8);
        hl.step(app, 1.0 / 60.0, &[Event::PointerUp]);
        hl.step(app, 1.0 / 60.0, &[Event::Key(rust_glass::input::Key::Escape)]);
        hl.settle(app, 1.5);
        r.map_err(|e| e.to_string())
    };
    let mv = |x: f32, y: f32| vec![Event::PointerMoved(vec2(x, y))];
    let down = || vec![Event::PointerDown];
    let up = || vec![Event::PointerUp];
    shot(&mut hl, &mut app, "shot_press", 1, 0, &[(mv(330.0, 150.0), 2), (down(), 30)])?;
    shot(&mut hl, &mut app, "shot_menu", 5, 1, &[(mv(340.0, 150.0), 2), (down(), 2), (up(), 8)])?;
    shot(&mut hl, &mut app, "shot_menu_open", 5, 1, &[(mv(340.0, 150.0), 2), (down(), 2), (up(), 60)])?;
    shot(
        &mut hl,
        &mut app,
        "shot_scroll",
        0,
        0,
        &[(mv(700.0, 300.0), 2), (vec![Event::Scroll(vec2(0.0, -260.0))], 40)],
    )?;
    let drag = |x0: f32, x1: f32, y: f32| {
        let mut v = vec![(mv(x0, y), 2), (down(), 16)];
        for i in 1..=14 {
            v.push((mv(x0 + (x1 - x0) * i as f32 / 14.0, y), 0));
        }
        v
    };
    shot(&mut hl, &mut app, "shot_split", 1, 1, &[(mv(300.0, 617.0), 2), (down(), 2), (up(), 7)])?;
    shot(&mut hl, &mut app, "shot_split_open", 1, 1, &[(mv(300.0, 617.0), 2), (down(), 2), (up(), 60)])?;
    shot(&mut hl, &mut app, "shot_tabs", 0, 0, &drag(545.0, 668.0, 722.0))?;
    shot(&mut hl, &mut app, "shot_toggle", 2, 1, &drag(312.0, 296.0, 393.0))?;
    shot(&mut hl, &mut app, "shot_alert", 6, 1, &[(mv(485.0, 417.0), 2), (down(), 2), (up(), 60)])?;
    shot(&mut hl, &mut app, "shot_toast", 6, 1, &[(mv(340.0, 417.0), 2), (down(), 2), (up(), 50)])?;
    shot(&mut hl, &mut app, "shot_sheet", 7, 2, &[(mv(531.0, 135.0), 2), (down(), 2), (up(), 70)])?;
    shot(&mut hl, &mut app, "shot_ref", 10, 0, &drag(390.0, 500.0, 264.0))?;
    // Light theme over dark content: small glass flips on its own.
    app.appearance = 1;
    shot(&mut hl, &mut app, "shot_adapt", 1, 2, &[])?;
    hl.ctx.theme.frost = 1.0;
    shot(&mut hl, &mut app, "shot_frost", 0, 0, &[])?;
    hl.ctx.theme.frost = Theme::FROST;
    hl.ctx.theme.reduce_transparency = true;
    hl.ctx.theme.high_contrast = true;
    shot(&mut hl, &mut app, "shot_access", 0, 0, &[])?;
    std::fs::write(format!("{dir}/report.txt"), &report).map_err(|e| e.to_string())?;
    println!("{report}");
    Ok(())
}

/// Pictures and short clips (as numbered frames) for the preview folder.
fn preview(dir: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let (w, h) = (2360u32, 1560u32);
    let mut hl = Headless::new(w, h, 2.0)?;
    let mut app = Showcase::new();
    app.setup(&mut hl.ctx);
    let dt = 1.0 / 60.0;
    let save = |hl: &Headless, name: &str| {
        image::save_buffer(format!("{dir}/{name}.png"), &hl.pixels(), w, h, image::ColorType::Rgba8)
            .map_err(|e| e.to_string())
    };
    let mv = |x: f32, y: f32| vec![Event::PointerMoved(vec2(x, y))];
    let down = || vec![Event::PointerDown];
    let up = || vec![Event::PointerUp];
    let go = |hl: &mut Headless, app: &mut Showcase, page: usize, wall: usize| {
        app.page = page;
        app.wall = wall;
        hl.step(app, dt, &up());
        hl.step(app, dt, &mv(-50.0, -50.0));
        hl.settle(app, 1.5);
    };
    let press = |hl: &mut Headless, app: &mut Showcase, x: f32, y: f32, wait: usize| {
        hl.step(app, dt, &mv(x, y));
        hl.step(app, dt, &down());
        for _ in 0..wait {
            hl.step(app, dt, &[]);
        }
    };
    let drag = |hl: &mut Headless, app: &mut Showcase, a: Vec2, b: Vec2, n: usize| {
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let p = a.lerp(b, t * t * (3.0 - 2.0 * t));
            hl.step(app, dt, &mv(p.x, p.y));
        }
    };

    // Material lab on every kind of glass.
    for (name, m, pos, size) in [
        ("lab-clear", Material::CLEAR, vec2(905.0, 190.0), 200.0),
        ("lab-lens", Material::LENS.scaled(3.2), vec2(880.0, 195.0), 190.0),
        ("lab-thick", Material::THICK, vec2(900.0, 190.0), 210.0),
        ("lab-regular", Material::REGULAR, vec2(560.0, 130.0), 160.0),
    ] {
        go(&mut hl, &mut app, 9, 3);
        (app.s.lab, app.s.lab_pos, app.s.lab_size) = (m, pos, size);
        hl.settle(&mut app, 2.0);
        save(&hl, name)?;
    }
    go(&mut hl, &mut app, 9, 4);
    (app.s.lab, app.s.lab_pos, app.s.lab_size) = (Material::CLEAR, vec2(905.0, 190.0), 200.0);
    hl.settle(&mut app, 2.0);
    save(&hl, "lab-grid")?;

    go(&mut hl, &mut app, 0, 0);
    save(&hl, "player")?;
    press(&mut hl, &mut app, 548.0, 722.0, 20);
    drag(&mut hl, &mut app, vec2(548.0, 722.0), vec2(702.0, 722.0), 24);
    for _ in 0..24 {
        hl.step(&mut app, dt, &[]);
    }
    save(&hl, "player-blob")?;
    go(&mut hl, &mut app, 0, 2);
    hl.step(&mut app, dt, &mv(700.0, 300.0));
    hl.step(&mut app, dt, &[Event::Scroll(vec2(0.0, -90.0))]);
    hl.settle(&mut app, 1.5);
    save(&hl, "player-dark")?;
    go(&mut hl, &mut app, 1, 1);
    save(&hl, "buttons")?;
    press(&mut hl, &mut app, 300.0, 617.0, 2);
    hl.step(&mut app, dt, &up());
    hl.settle(&mut app, 2.0);
    save(&hl, "buttons-split")?;
    go(&mut hl, &mut app, 2, 2);
    press(&mut hl, &mut app, 500.0, 286.0, 40);
    save(&hl, "sliders")?;
    go(&mut hl, &mut app, 3, 3);
    press(&mut hl, &mut app, 330.0, 289.0, 16);
    drag(&mut hl, &mut app, vec2(330.0, 289.0), vec2(392.0, 289.0), 20);
    save(&hl, "segments")?;
    go(&mut hl, &mut app, 4, 4);
    save(&hl, "fields")?;
    go(&mut hl, &mut app, 5, 1);
    press(&mut hl, &mut app, 340.0, 150.0, 2);
    hl.step(&mut app, dt, &up());
    hl.settle(&mut app, 1.5);
    save(&hl, "menu")?;
    hl.step(&mut app, dt, &[Event::Key(rust_glass::input::Key::Escape)]);
    go(&mut hl, &mut app, 6, 1);
    save(&hl, "feedback")?;
    go(&mut hl, &mut app, 7, 2);
    save(&hl, "windows")?;
    go(&mut hl, &mut app, 8, 3);
    save(&hl, "settings")?;
    go(&mut hl, &mut app, 10, 0);
    press(&mut hl, &mut app, 628.0, 147.0, 16);
    drag(&mut hl, &mut app, vec2(628.0, 147.0), vec2(470.0, 147.0), 30);
    for _ in 0..20 {
        hl.step(&mut app, dt, &[]);
    }
    save(&hl, "reference-slider")?;

    // Clips, 30 frames per second.
    let clip = |hl: &mut Headless,
                app: &mut Showcase,
                name: &str,
                events: &mut dyn FnMut(usize) -> Vec<Event>,
                n: usize| {
        let d = format!("{dir}/frames-{name}");
        std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
        for i in 0..n {
            hl.step(app, dt, &events(i));
            if i % 2 == 0 {
                let path = format!("{d}/{:04}.png", i / 2);
                image::save_buffer(path, &hl.pixels(), w, h, image::ColorType::Rgba8)
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok::<(), String>(())
    };
    let ease = |t: f32| t * t * (3.0 - 2.0 * t);
    go(&mut hl, &mut app, 0, 0);
    let keys = [548.0f32, 779.0, 625.0, 856.0, 702.0];
    clip(
        &mut hl,
        &mut app,
        "tabs",
        &mut |i| {
            let t = (i as f32 - 20.0) / 34.0;
            let k = (t.max(0.0) as usize).min(keys.len() - 2);
            let x = keys[k] + (keys[k + 1] - keys[k]) * ease((t - k as f32).clamp(0.0, 1.0));
            let mut e = mv(if t < 0.0 { keys[0] } else { x }, 722.0);
            if i == 4 {
                e.push(Event::PointerDown);
            }
            if i == 176 {
                e.push(Event::PointerUp);
            }
            e
        },
        210,
    )?;
    go(&mut hl, &mut app, 1, 1);
    // The split button was left open above; close it first.
    press(&mut hl, &mut app, 300.0, 617.0, 2);
    hl.step(&mut app, dt, &up());
    hl.settle(&mut app, 2.0);
    clip(
        &mut hl,
        &mut app,
        "split",
        &mut |i| match i {
            0 => mv(300.0, 617.0),
            12 | 86 | 140 | 158 => down(),
            18 | 92 | 144 | 162 => up(),
            _ => vec![],
        },
        210,
    )?;
    go(&mut hl, &mut app, 2, 2);
    let keys = [504.0f32, 660.0, 350.0, 600.0, 440.0, 504.0];
    clip(
        &mut hl,
        &mut app,
        "slider",
        &mut |i| {
            let t = (i as f32 - 36.0) / 32.0;
            let k = (t.max(0.0) as usize).min(keys.len() - 2);
            let x = keys[k] + (keys[k + 1] - keys[k]) * ease((t - k as f32).clamp(0.0, 1.0));
            let mut e = mv(if t < 0.0 { keys[0] } else { x }, 286.0);
            if i == 10 {
                e.push(Event::PointerDown);
            }
            if i == 222 {
                e.push(Event::PointerUp);
            }
            e
        },
        270,
    )?;
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = |flag: &str| {
        let i = args.iter().position(|a| a == flag)?;
        Some(args.get(i + 1).cloned().unwrap_or_else(|| "captures".into()))
    };
    let result = match (dir("--capture"), dir("--preview")) {
        (Some(d), _) => capture(&d),
        (_, Some(d)) => preview(&d),
        _ => run(
            Options { title: "rust-glass showcase".into(), size: (1180.0, 800.0), ..Default::default() },
            Showcase::new(),
        ),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
