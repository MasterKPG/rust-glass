//! Renders the showcase without a window: every page with timings
//! (`--capture`), and the pictures and clips of the preview folder
//! (`--preview`). Widgets are found through the rectangles the pages mark,
//! so the scripts keep working when the layout changes.

use rust_glass::headless::Headless;
use rust_glass::input::{Event, Key};
use rust_glass::prelude::*;

use crate::Showcase;
use crate::pages::Page;

const DT: f32 = 1.0 / 60.0;

/// Wallpapers by index into `art::WALLPAPERS`.
const DUNES: usize = 0;
const PETALS: usize = 1;
const AURORA: usize = 2;
const RIBBONS: usize = 3;
const GRID: usize = 4;

/// The wallpaper each page is captured on.
fn wallpaper_for(page: Page) -> usize {
    match page {
        Page::Player | Page::Text => DUNES,
        Page::Buttons | Page::Menus | Page::Feedback | Page::Lists => PETALS,
        Page::Controls | Page::Windows => AURORA,
        Page::Selection | Page::Settings | Page::Lab => RIBBONS,
    }
}

/// A headless showcase with helpers to drive it.
struct Rig {
    host: Headless,
    app: Showcase,
}

impl Rig {
    fn new(width: u32, height: u32, pixels_per_point: f32) -> Result<Self, String> {
        let mut host = Headless::new(width, height, pixels_per_point)?;
        let mut app = Showcase::new();
        app.setup(&mut host.ctx);
        Ok(Self { host, app })
    }

    fn step(&mut self, events: &[Event]) {
        self.host.step(&mut self.app, DT as f64, events);
    }

    fn wait(&mut self, frames: usize) {
        for _ in 0..frames {
            self.step(&[]);
        }
    }

    fn settle(&mut self, seconds: f64) {
        self.host.settle(&mut self.app, seconds);
    }

    /// Opens `page` on `wallpaper` with the pointer outside the window.
    fn open(&mut self, page: Page, wallpaper: usize) {
        self.app.page = page;
        self.app.wallpaper = wallpaper;
        self.step(&[Event::PointerUp, Event::Key(Key::Escape)]);
        self.step(&[Event::PointerMoved(vec2(-50.0, -50.0))]);
        self.settle(1.5);
    }

    fn mark(&self, name: &str) -> Vec2 {
        self.rect(name).center()
    }

    fn rect(&self, name: &str) -> Rect {
        self.app.pages.marks.rect(name)
    }

    /// Where the thumb of the marked slider sits at `value`, from 0 to 1.
    fn slider_point(&self, name: &str, value: f32) -> Vec2 {
        let rect = self.rect(name);
        // The thumb is 37 points wide and travels inside the track.
        vec2(rect.min.x + 18.5 + (rect.width() - 37.0) * value, rect.center().y)
    }

    fn move_to(&mut self, at: Vec2) {
        self.step(&[Event::PointerMoved(at)]);
    }

    fn press(&mut self, at: Vec2) {
        self.move_to(at);
        self.step(&[Event::PointerDown]);
    }

    fn click(&mut self, at: Vec2) {
        self.press(at);
        self.step(&[Event::PointerUp]);
    }

    /// Moves the held pointer from `from` to `to` in `frames` eased steps.
    fn drag(&mut self, from: Vec2, to: Vec2, frames: usize) {
        for i in 1..=frames {
            let t = i as f32 / frames as f32;
            self.move_to(from.lerp(to, t * t * (3.0 - 2.0 * t)));
        }
    }

    fn scroll(&mut self, over: Vec2, amount: f32) {
        self.move_to(over);
        self.step(&[Event::Scroll(vec2(0.0, -amount))]);
        self.settle(1.5);
    }

    fn save(&self, path: &str) -> Result<(), String> {
        let (width, height) = self.size();
        image::save_buffer(path, &self.host.pixels(), width, height, image::ColorType::Rgba8)
            .map_err(|e| e.to_string())
    }

    fn size(&self) -> (u32, u32) {
        let screen = self.host.ctx.screen.size() * self.host.ctx.pixels_per_point;
        (screen.x.round() as u32, screen.y.round() as u32)
    }
}

/// Renders every page and a set of interactions to `dir`, and writes the
/// timings to `report.txt`.
pub fn pages(dir: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut rig = Rig::new(1770, 1170, 1.5)?;
    let mut report = format!("{} at 1770 x 1170\n", rig.host.describe());
    for (i, page) in Page::ALL.iter().enumerate() {
        rig.open(*page, wallpaper_for(*page));
        // Average over steady frames.
        let (mut ui_ms, mut encode_ms) = (0.0, 0.0);
        for _ in 0..30 {
            rig.step(&[]);
            ui_ms += rig.host.info.ui_ms / 30.0;
            encode_ms += rig.host.info.encode_ms / 30.0;
        }
        let gpu = rig.host.info.gpu;
        let gpu_ms = gpu.gpu_ms.map_or("n/a".to_string(), |ms| format!("{ms:.2}"));
        report += &format!(
            "{:<20} ui {ui_ms:.2} ms, encode {encode_ms:.2} ms, gpu {gpu_ms} ms, {} passes, {} glass, {} shapes, {} kpx captured\n",
            page.title(),
            gpu.passes,
            gpu.glasses,
            gpu.shapes,
            gpu.captured_px / 1000
        );
        rig.save(&format!("{dir}/page_{i:02}.png"))?;
    }

    let shot = |rig: &mut Rig, name: &str| rig.save(&format!("{dir}/shot_{name}.png"));

    rig.open(Page::Buttons, PETALS);
    rig.press(rig.mark("regular button"));
    rig.wait(30);
    shot(&mut rig, "press")?;

    rig.open(Page::Buttons, PETALS);
    rig.click(rig.mark("split button"));
    rig.wait(7);
    shot(&mut rig, "split")?;
    rig.wait(60);
    shot(&mut rig, "split_open")?;

    rig.open(Page::Menus, PETALS);
    rig.click(rig.mark("options menu"));
    rig.wait(8);
    shot(&mut rig, "menu")?;
    rig.wait(60);
    shot(&mut rig, "menu_open")?;

    rig.open(Page::Player, DUNES);
    rig.scroll(rig.mark("player content"), 260.0);
    shot(&mut rig, "scroll")?;

    // An album cover grows from the grid into the detail view.
    rig.open(Page::Player, DUNES);
    rig.scroll(rig.mark("player content"), -1000.0);
    rig.click(rig.mark("first album"));
    rig.wait(8);
    shot(&mut rig, "album_morph")?;
    rig.wait(60);
    shot(&mut rig, "album")?;
    rig.click(rig.mark("album back"));
    rig.settle(2.0);

    rig.open(Page::Selection, RIBBONS);
    let tabs = rig.mark("tab bar");
    rig.press(tabs - vec2(154.0, 0.0));
    rig.wait(16);
    rig.drag(tabs - vec2(154.0, 0.0), tabs - vec2(30.0, 0.0), 14);
    shot(&mut rig, "tabs")?;

    rig.open(Page::Controls, AURORA);
    let switch = rig.rect("wifi switch");
    let thumb = vec2(switch.max.x - 20.0, switch.center().y);
    rig.press(thumb);
    rig.wait(16);
    rig.drag(thumb, thumb - vec2(16.0, 0.0), 14);
    shot(&mut rig, "switch")?;

    rig.open(Page::Feedback, PETALS);
    rig.click(rig.mark("alert button"));
    rig.wait(9);
    shot(&mut rig, "alert_forming")?;
    rig.wait(51);
    shot(&mut rig, "alert")?;

    rig.open(Page::Feedback, PETALS);
    rig.click(rig.mark("toast button"));
    rig.wait(50);
    shot(&mut rig, "toast")?;
    // Let the toast leave before the next shot.
    rig.settle(3.0);

    rig.open(Page::Windows, AURORA);
    rig.click(rig.mark("sheet button"));
    rig.wait(10);
    shot(&mut rig, "sheet_forming")?;
    rig.wait(60);
    shot(&mut rig, "sheet")?;

    rig.open(Page::Lists, PETALS);
    rig.scroll(rig.mark("long list") + vec2(0.0, -150.0), 720.0);
    shot(&mut rig, "lists")?;
    let tasks = rig.rect("task list");
    let first = vec2(tasks.min.x + 60.0, tasks.min.y + 20.0);
    rig.press(first);
    rig.wait(16);
    rig.drag(first, first + vec2(10.0, 140.0), 14);
    rig.wait(20);
    shot(&mut rig, "reorder")?;

    rig.open(Page::Lists, PETALS);
    rig.scroll(rig.mark("long list") + vec2(0.0, -150.0), 720.0);
    let todo = rig.rect("todo list");
    let card = vec2(todo.min.x + 60.0, todo.min.y + 20.0);
    rig.press(card);
    rig.wait(16);
    rig.drag(card, rig.mark("done list"), 14);
    rig.wait(20);
    shot(&mut rig, "drop")?;

    // Light content over a dark wallpaper: small glass switches to dark.
    rig.app.appearance = 1;
    rig.open(Page::Buttons, AURORA);
    shot(&mut rig, "adapt")?;
    rig.app.appearance = 0;

    rig.host.ctx.theme.glass_tint = 1.0;
    rig.open(Page::Player, DUNES);
    shot(&mut rig, "tinted")?;
    rig.host.ctx.theme.glass_tint = Theme::DEFAULT_GLASS_TINT;

    rig.host.ctx.theme.reduce_transparency = true;
    rig.host.ctx.theme.increase_contrast = true;
    rig.open(Page::Player, DUNES);
    shot(&mut rig, "accessibility")?;

    std::fs::write(format!("{dir}/report.txt"), &report).map_err(|e| e.to_string())?;
    println!("{report}");
    Ok(())
}

/// Renders the pictures of the preview folder to `dir`, and the clips as
/// numbered frames in `dir/frames-NAME`, 30 per second.
pub fn preview(dir: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut rig = Rig::new(2360, 1560, 2.0)?;
    let save = |rig: &Rig, name: &str| rig.save(&format!("{dir}/{name}.png"));

    // The material lab with each kind of glass.
    let lab_shots = [
        ("lab-clear", Material::CLEAR, vec2(905.0, 190.0), 200.0, RIBBONS),
        ("lab-lens", Material::LENS.scaled(3.2), vec2(880.0, 195.0), 190.0, RIBBONS),
        ("lab-thick", Material::THICK, vec2(900.0, 190.0), 210.0, RIBBONS),
        ("lab-regular", Material::REGULAR, vec2(560.0, 130.0), 160.0, RIBBONS),
        ("lab-grid", Material::CLEAR, vec2(905.0, 190.0), 200.0, GRID),
    ];
    for (name, material, position, size, wallpaper) in lab_shots {
        rig.open(Page::Lab, wallpaper);
        let lab = &mut rig.app.pages.lab;
        (lab.material, lab.position, lab.size) = (material, position, size);
        rig.settle(2.0);
        save(&rig, name)?;
    }

    rig.open(Page::Player, DUNES);
    save(&rig, "player")?;
    let tabs = rig.mark("player tab bar");
    let tab = |i: usize| vec2(tabs.x - 154.0 + 77.0 * i as f32, tabs.y);
    rig.press(tab(0));
    rig.wait(20);
    rig.drag(tab(0), tab(2), 24);
    rig.wait(24);
    save(&rig, "player-blob")?;
    rig.open(Page::Player, AURORA);
    rig.scroll(rig.mark("player content"), 90.0);
    save(&rig, "player-dark")?;

    for (name, page) in [
        ("buttons", Page::Buttons),
        ("segments", Page::Selection),
        ("fields", Page::Text),
        ("feedback", Page::Feedback),
        ("windows", Page::Windows),
        ("lists", Page::Lists),
        ("settings", Page::Settings),
    ] {
        rig.open(page, wallpaper_for(page));
        save(&rig, name)?;
    }

    rig.open(Page::Buttons, PETALS);
    rig.click(rig.mark("split button"));
    rig.settle(2.0);
    save(&rig, "buttons-split")?;

    rig.open(Page::Controls, AURORA);
    rig.press(rig.slider_point("volume slider", 0.6));
    rig.wait(40);
    save(&rig, "sliders")?;

    rig.open(Page::Menus, PETALS);
    rig.click(rig.mark("options menu"));
    rig.settle(1.5);
    save(&rig, "menu")?;

    // Clips.
    let clip = |rig: &mut Rig, name: &str, frames: usize, events: &mut dyn FnMut(usize) -> Vec<Event>| {
        let folder = format!("{dir}/frames-{name}");
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        for i in 0..frames {
            let list = events(i);
            rig.step(&list);
            if i % 2 == 0 {
                rig.save(&format!("{folder}/{:04}.png", i / 2))?;
            }
        }
        Ok::<(), String>(())
    };
    let ease = |t: f32| t * t * (3.0 - 2.0 * t);
    // Moves through `keys` one every `per` frames after `start`.
    let along = |keys: &[Vec2], i: usize, start: usize, per: f32| {
        let t = (i as f32 - start as f32) / per;
        let k = (t.max(0.0) as usize).min(keys.len() - 2);
        keys[k].lerp(keys[k + 1], ease((t - k as f32).clamp(0.0, 1.0)))
    };

    rig.open(Page::Player, DUNES);
    let tabs = rig.mark("player tab bar");
    let tab = |i: usize| vec2(tabs.x - 154.0 + 77.0 * i as f32, tabs.y);
    let keys = [tab(0), tab(3), tab(1), tab(4), tab(2)];
    clip(&mut rig, "tabs", 210, &mut |i| {
        let mut events = vec![Event::PointerMoved(along(&keys, i, 20, 34.0))];
        match i {
            4 => events.push(Event::PointerDown),
            176 => events.push(Event::PointerUp),
            _ => {}
        }
        events
    })?;

    rig.open(Page::Buttons, PETALS);
    let plus = rig.mark("split button");
    clip(&mut rig, "split", 210, &mut |i| match i {
        0 => vec![Event::PointerMoved(plus)],
        12 | 86 | 140 | 158 => vec![Event::PointerDown],
        18 | 92 | 144 | 162 => vec![Event::PointerUp],
        _ => vec![],
    })?;

    rig.open(Page::Controls, AURORA);
    let keys: Vec<Vec2> =
        [0.6, 0.95, 0.15, 0.8, 0.4, 0.6].iter().map(|v| rig.slider_point("volume slider", *v)).collect();
    clip(&mut rig, "slider", 270, &mut |i| {
        let mut events = vec![Event::PointerMoved(along(&keys, i, 36, 32.0))];
        match i {
            10 => events.push(Event::PointerDown),
            222 => events.push(Event::PointerUp),
            _ => {}
        }
        events
    })?;
    Ok(())
}
