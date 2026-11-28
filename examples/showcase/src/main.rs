//! Every widget of rust-glass in one app.
//!
//! ```text
//! cargo run --release -p showcase
//! cargo run --release -p showcase -- --capture captures
//! cargo run --release -p showcase -- --preview captures/preview
//! ```
//!
//! `--capture DIR` renders every page to PNG without a window and prints
//! timings. `--preview DIR` renders the pictures and clips of the preview
//! folder. `--motion DIR` renders drops, presses and the album view frame by
//! frame. `--quality full|reduced|low` renders them at that quality step.
//! `--bench FILE` writes the frame budget, and `--compare BASE HEAD`
//! fails when HEAD is more than 15 percent over the budget of BASE.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod art;
mod bench;
mod capture;
mod pages;

use rust_glass::prelude::*;

use pages::Page;

/// Width of the sidebar, in points.
const SIDEBAR_WIDTH: f32 = 232.0;
/// Gap between the window edge, the sidebar and the page.
const MARGIN: f32 = 14.0;

/// Light, dark, or following the wallpaper.
#[derive(Clone, Copy, PartialEq)]
enum Appearance {
    Auto,
    Light,
    Dark,
}

pub struct Showcase {
    pub page: Page,
    /// Index into [`art::WALLPAPERS`], or `art::WALLPAPERS.len()` for an
    /// image dropped on the window.
    pub wallpaper: usize,
    wallpaper_shown: Option<usize>,
    wallpaper_image: Option<ImageId>,
    wallpaper_aspect: f32,
    dropped_image: Option<(Vec<u8>, u32, u32)>,
    appearance: usize,
    pub covers: Vec<ImageId>,
    pub pages: pages::State,
}

impl Showcase {
    fn new() -> Self {
        Self {
            page: Page::Player,
            wallpaper: 0,
            wallpaper_shown: None,
            wallpaper_image: None,
            wallpaper_aspect: 1.5,
            dropped_image: None,
            appearance: 0,
            covers: Vec::new(),
            pages: pages::State::default(),
        }
    }

    fn appearance(&self) -> Appearance {
        [Appearance::Auto, Appearance::Light, Appearance::Dark][self.appearance.min(2)]
    }

    /// Draws the wallpaper behind everything, covering the window.
    fn wallpaper(&mut self, ctx: &mut Context) {
        let new_image = if let Some((pixels, width, height)) = self.dropped_image.take() {
            self.wallpaper = art::WALLPAPERS.len();
            Some((pixels, width, height))
        } else if self.wallpaper_shown != Some(self.wallpaper) && self.wallpaper < art::WALLPAPERS.len() {
            let (width, height) = (1800, 1200);
            Some((art::WALLPAPERS[self.wallpaper].render(width, height), width, height))
        } else {
            None
        };
        if let Some((pixels, width, height)) = new_image {
            if let Some(old) = self.wallpaper_image.take() {
                ctx.remove_image(old);
            }
            self.wallpaper_image = Some(ctx.add_image(width, height, pixels));
            self.wallpaper_aspect = width as f32 / height as f32;
            self.wallpaper_shown = Some(self.wallpaper);
        }

        let wallpaper_is_dark = art::WALLPAPERS.get(self.wallpaper).is_some_and(|w| w.dark);
        ctx.theme.dark = match self.appearance() {
            Appearance::Auto => wallpaper_is_dark,
            Appearance::Light => false,
            Appearance::Dark => true,
        };

        let Some(image) = self.wallpaper_image else { return };
        // Cover the window and keep the proportions of the image.
        let size = ctx.screen.size();
        let aspect = size.x / size.y.max(1.0);
        let (u, v) = if aspect > self.wallpaper_aspect {
            (1.0, self.wallpaper_aspect / aspect)
        } else {
            (aspect / self.wallpaper_aspect, 1.0)
        };
        let uv = Rect::from_center_size(vec2(0.5, 0.5), vec2(u, v));
        // The scene is slightly larger than the window, so glass at the edge
        // has something to bend. Stretch the image into that border too.
        let border = vec2(64.0, 64.0);
        let rect = Rect { min: ctx.screen.min, max: ctx.screen.max + border };
        let grow = vec2(uv.width() * border.x / size.x.max(1.0), uv.height() * border.y / size.y.max(1.0));
        let uv = Rect { min: uv.min, max: uv.max + grow };
        ctx.background().image(rect, 0.0, image, uv, Color::WHITE);
    }

    fn sidebar(&mut self, ctx: &mut Context, rect: Rect) {
        Panel::new("sidebar", rect).corner(26.0).padding(12.0).show(ctx, |ui| {
            ui.padded(vec2(8.0, 4.0), |ui| ui.subheading("rust-glass"));
            let mut index = self.page as usize;
            if nav_list(ui, &mut index, &Page::NAVIGATION) {
                self.page = Page::ALL[index];
            }
            let footer =
                Rect { min: vec2(ui.max_rect.min.x, ui.max_rect.max.y - 190.0), max: ui.max_rect.max };
            ui.in_rect("footer", footer, |ui| {
                ui.spacing = 6.0;
                ui.separator();
                ui.caption("Glass: clear to tinted");
                let mut tint = ui.ctx.theme.glass_tint;
                if ui.slider(&mut tint, 0.0..=1.0).changed() {
                    ui.ctx.theme.glass_tint = tint;
                }
                ui.caption("Appearance");
                let width = ui.available_width();
                ui.add(Segmented::new(&mut self.appearance, &["Auto", "Light", "Dark"]).width(width));
                ui.caption("Wallpaper");
                let mut names: Vec<&str> = art::WALLPAPERS.iter().map(|w| w.name).collect();
                if self.wallpaper == art::WALLPAPERS.len() {
                    names.push("Dropped image");
                }
                ComboBox::new(&mut self.wallpaper, &names).width(width).show(ui);
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
        let sidebar =
            Rect { min: vec2(MARGIN, MARGIN), max: vec2(MARGIN + SIDEBAR_WIDTH, screen.max.y - MARGIN) };
        let page = Rect { min: vec2(sidebar.max.x + MARGIN, 0.0), max: screen.max };
        pages::show(self, ctx, info, page);
        self.sidebar(ctx, sidebar);
    }

    fn file_dropped(&mut self, ctx: &mut Context, path: &std::path::Path) {
        match image::open(path) {
            Ok(image) => {
                let image = image.resize(2400, 2400, image::imageops::FilterType::Triangle).to_rgba8();
                let (width, height) = image.dimensions();
                self.dropped_image = Some((image.into_raw(), width, height));
            }
            Err(e) => ctx.toast(Some(Icon::Warning), format!("Cannot open image: {e}")),
        }
        ctx.request_repaint();
    }
}

/// Counts heap allocations, for the frame budget.
struct Counting;

static ALLOCATIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

// SAFETY: every call goes to the system allocator unchanged.
unsafe impl std::alloc::GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // SAFETY: same contract as the caller's.
        unsafe { std::alloc::System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        // SAFETY: same contract as the caller's.
        unsafe { std::alloc::System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // SAFETY: same contract as the caller's.
        unsafe { std::alloc::System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// Heap allocations and reallocations since the start.
pub(crate) fn allocations() -> u64 {
    ALLOCATIONS.load(std::sync::atomic::Ordering::Relaxed)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let folder = |flag: &str| {
        let i = args.iter().position(|a| a == flag)?;
        Some(args.get(i + 1).cloned().unwrap_or_else(|| "captures".into()))
    };
    if let Some(q) = folder("--quality") {
        let q = match q.as_str() {
            "full" => Quality::Full,
            "reduced" => Quality::Reduced,
            "low" => Quality::Low,
            _ => Quality::Auto,
        };
        let _ = capture::QUALITY.set(q);
    }
    let compare = args.iter().position(|a| a == "--compare").map(|i| (args.get(i + 1), args.get(i + 2)));
    let result = match (folder("--capture"), folder("--preview"), folder("--motion")) {
        _ if compare.is_some() => match compare {
            Some((Some(base), Some(head))) => bench::compare(base, head),
            _ => Err("usage: --compare BASE HEAD, each a file or a list of files separated by commas".into()),
        },
        _ if folder("--bench").is_some() => bench::run(&folder("--bench").unwrap_or_default()),
        (Some(dir), _, _) => capture::pages(&dir),
        (_, Some(dir), _) => capture::preview(&dir),
        (_, _, Some(dir)) => capture::motion(&dir),
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
