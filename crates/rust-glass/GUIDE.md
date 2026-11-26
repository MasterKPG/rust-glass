# Guide

How to build an app with rust-glass. Every Rust block on this page is compiled by `cargo test`.

- [Set up a project](#set-up-a-project)
- [The app loop](#the-app-loop)
- [Put things on screen](#put-things-on-screen)
- [Layout](#layout)
- [Scrolling and long lists](#scrolling-and-long-lists)
- [Drag and drop](#drag-and-drop)
- [Widgets](#widgets)
- [Menus, dialogs and windows](#menus-dialogs-and-windows)
- [Images and wallpaper](#images-and-wallpaper)
- [Theme](#theme)
- [Materials](#materials)
- [Custom painting and your own widgets](#custom-painting-and-your-own-widgets)
- [Animation and repainting](#animation-and-repainting)
- [Keyboard, clipboard and links](#keyboard-clipboard-and-links)
- [Accessibility](#accessibility)
- [Testing without a window](#testing-without-a-window)
- [Common problems](#common-problems)

## Set up a project

You need Rust 1.90 or newer and a GPU with Vulkan, DirectX 12 or Metal.

```bash
cargo new hello-glass
cd hello-glass
cargo add rust-glass
```

Replace `src/main.rs` with:

```rust,no_run
use rust_glass::prelude::*;

struct Hello {
    volume: f32,
    wifi: bool,
}

impl App for Hello {
    fn update(&mut self, ctx: &mut Context, _: &FrameInfo) {
        // Something colorful for the glass to bend.
        let screen = ctx.screen;
        ctx.background().gradient(screen.expand(64.0), 0.0, Color::INDIGO, Color::ORANGE, vec2(0.5, 0.5));

        let panel = Rect::from_center_size(screen.center(), vec2(360.0, 220.0));
        Panel::new("main", panel).padding(22.0).show(ctx, |ui| {
            ui.heading("Hello");
            ui.slider(&mut self.volume, 0.0..=1.0);
            ui.toggle(&mut self.wifi, "Wi-Fi");
            if ui.prominent_button("Reset").clicked() {
                self.volume = 0.5;
            }
        });
    }
}

fn main() -> Result<(), String> {
    let options = Options { title: "Hello".into(), size: (640.0, 420.0), ..Default::default() };
    run(options, Hello { volume: 0.3, wifi: true })
}
```

Run it with `cargo run --release`. For debug builds, let Cargo optimize the dependencies. Loading the system fonts and shaping text is slow without it:

```toml
[profile.dev.package."*"]
opt-level = 3
```

## The app loop

Implement `App` for your state and pass it to `run`.

- `setup(&mut self, ctx)` runs once before the first frame. Upload images and change the theme here.
- `update(&mut self, ctx, info)` runs for every frame. It reads input, changes your state and describes the whole screen. When the size of something had to be guessed, it runs again for the same frame without input.
- `file_dropped(&mut self, ctx, path)` runs when a file is dropped on the window.

The API is immediate mode: widgets take a reference to your data, draw it, and return a `Response`. There are no widget objects to keep around and no callbacks.

```rust
use rust_glass::prelude::*;

fn counter(ui: &mut Ui, n: &mut i32) {
    ui.label(&format!("Count: {n}"));
    if ui.button("Add one").clicked() {
        *n += 1;
    }
}
```

The fields of `Options` are `title`, `size` and `min_size` in points, `vsync`, and `low_power`, which prefers the integrated GPU on machines with two.

`FrameInfo` holds timings of recent frames (`ui_ms`, `encode_ms`, `gpu`, `interval_ms`) in case you want to show them. `gpu.gpu_ms` is `None` where the GPU cannot measure time.

All lengths are in points. `ctx.screen` is the window, `ctx.pixels_per_point` the number of pixels per point.

## Put things on screen

`ctx` is the window. A `Ui` is a region that hands out space to widgets. You get a `Ui` from one of these:

- `region(ctx, id, rect, |ui| ...)`: a part of the window without a surface of its own. Use it for page content.
- `Panel::new(id, rect).show(ctx, |ui| ...)`: a floating glass panel. Use it for sidebars and toolbars.
- `Window::new(title).show(ctx, |ui| ...)`: a glass window the user can drag and fold.
- `popup`, `modal`, `sheet` and `context_menu`, covered below.

`TabBar::new(..).show_at(ctx, id, center)` places a floating tab bar directly.

Layers are stacked by `Order`: background, content, bars, windows, modals, popups, toasts, tooltips. Within one order, the layer that was clicked last is on top.

Glass shows whatever is drawn below it, so put content in a region and float controls above it. A typical screen is a scrolling page with bars on top:

```rust
use rust_glass::prelude::*;

#[derive(Default)]
struct Mail {
    tab: usize,
    folder: usize,
}

impl App for Mail {
    fn update(&mut self, ctx: &mut Context, _: &FrameInfo) {
        let screen = ctx.screen;
        let side = rect(14.0, 14.0, 220.0, screen.height() - 28.0);
        let main = Rect { min: vec2(side.max.x + 14.0, 0.0), max: screen.max };

        region(ctx, "page", main.shrink2(vec2(20.0, 0.0)), |ui| {
            // Content scrolls below the tab bar and ends above it.
            ScrollArea::new().insets(24.0, 110.0).show(ui, |ui| {
                ui.heading("Inbox");
                for i in 1..=30 {
                    ui.label(&format!("Message {i}"));
                    ui.separator();
                }
            });
        });

        Panel::new("side", side).show(ctx, |ui| {
            nav_list(ui, &mut self.folder, &[(Icon::Mail, "Inbox"), (Icon::Trash, "Trash")]);
        });

        let tabs = [(Icon::Mail, "Mail"), (Icon::Calendar, "Calendar"), (Icon::Search, "Search")];
        let center = vec2(main.center().x, screen.max.y - 60.0);
        TabBar::new(&mut self.tab, &tabs).show_at(ctx, Id::new("tabs"), center);
    }
}
```

A button placed on a glass panel draws as a plain fill, because glass on glass muddies both. For blocks of text or groups of controls inside a page, use `ui.card` instead of glass.

## Layout

A `Ui` places widgets from top to bottom. These change that:

```rust
use rust_glass::prelude::*;

fn layout(ui: &mut Ui) {
    // Side by side, centered vertically.
    ui.horizontal(|ui| {
        ui.button("One");
        ui.button("Two");
    });
    // A row in the middle, and one at the right edge.
    ui.horizontal_centered(|ui| ui.button("Middle"));
    ui.horizontal_right(|ui| ui.button("Right"));

    // Three columns of the same width.
    ui.grid(3, 16.0, |ui, column| {
        ui.label(&format!("Column {column}"));
    });

    // A column of fixed width inside a row.
    ui.horizontal(|ui| {
        ui.vertical(180.0, |ui| {
            ui.strong("Title");
            ui.secondary("Subtitle");
        });
        ui.button("Open");
    });

    ui.padded(vec2(20.0, 10.0), |ui| ui.label("Inset"));
    ui.card(|ui| ui.label("On a card"));
    section(ui, "Title", |ui| {
        ui.label("On a card under a title");
    });
    ui.space(12.0);
    ui.separator();

    // Gap between widgets, from here on in this region.
    ui.spacing = 6.0;
}
```

`ui.available_width()` and `ui.available_height()` tell how much room is left. `ui.in_rect(id, rect, |ui| ...)` ignores the flow and lays out in a rectangle you choose. `ui.align = Align::Center` centers the widgets of a vertical layout.

`ui.add_enabled(condition, |ui| ...)` dims everything inside and blocks input while the condition is false.

Most widgets get their identity from their position in the layout. If widgets come and go, give the changing part its own `in_rect` with a stable id, or the state of the widgets after it moves along (a running animation, keyboard focus, an open menu).

## Scrolling and long lists

`ScrollArea::new()` scrolls vertically, `ScrollArea::horizontal()` sideways and `ScrollArea::both()` on two axes. An area fills the remaining height unless you call `.height(h)`. Two scroll areas in one `Ui` need different ids.

```rust
use rust_glass::prelude::*;

fn scrolling(ui: &mut Ui, photos: &[String], rows: &[String]) {
    // A row of cards. Children sit side by side.
    ScrollArea::horizontal().id("photos").height(140.0).fade_sides(24.0, 24.0).show(ui, |ui| {
        for p in photos {
            ui.button(p);
        }
    });

    // A table wider and taller than the view.
    ScrollArea::both().id("table").height(300.0).content_width(1600.0).show(ui, |ui| {
        ui.label("Wide content");
    });

    // A long list: only the rows in view are laid out.
    ScrollArea::new().id("rows").height(400.0).show_rows(ui, 28.0, rows.len(), |ui, range| {
        for i in range {
            ui.label(&rows[i]);
        }
    });
}
```

The wheel scrolls the innermost area under the pointer that scrolls on that axis. A horizontal row inside a page leaves the vertical wheel to the page and takes sideways swipes. Shift with the wheel scrolls sideways.

`show_rows` takes the height of one row and the number of rows, and calls your closure with the range that is visible, plus one row on each side. Each row has to be exactly that tall. The cost of a frame then depends on the height of the view, not on the length of the list, so a hundred thousand rows scroll as smoothly as ten.

`.fade(top, bottom)` and `.fade_sides(left, right)` dissolve content at the edges, `.insets(top, bottom)` keeps room for floating bars, and `scroll_offset` and `set_scroll_offset` in `widgets::scroll` save and restore the position.

## Drag and drop

`ui.drag_source(id, payload, |ui| ...)` makes its content draggable. Once the pointer moves a few points with the button down, the content lifts onto a glass plate and follows the pointer, and its place in the layout fades. `ui.drop_zone::<P, _>(id, min_size, |ui| ...)` lights up while a payload of type `P` is over it and returns the payload in the frame it is dropped. Escape cancels a drag.

```rust
use rust_glass::prelude::*;

struct Board {
    todo: Vec<String>,
    done: Vec<String>,
}

fn board(ui: &mut Ui, b: &mut Board) {
    ui.horizontal(|ui| {
        ui.vertical(240.0, |ui| {
            for (i, task) in b.todo.iter().enumerate() {
                // The id must follow the item, not its position.
                ui.drag_source(("task", task.as_str()), i, |ui| {
                    ui.button(task);
                });
            }
        });
        let (_, dropped) = ui.drop_zone::<usize, _>("done", vec2(240.0, 200.0), |ui| {
            for task in &b.done {
                ui.label(task);
            }
        });
        if let Some(i) = dropped {
            b.done.push(b.todo.remove(i));
        }
    });
}

fn tasks(ui: &mut Ui, list: &mut Vec<String>) {
    // Drag items up and down to change the order. The others make room.
    ui.reorder("tasks", list, |ui, task| {
        ui.label(task);
    });
}
```

The payload can be any type that is `Clone` and `'static`. Zones only react to their own type, so a list of files and a list of colors on one screen do not mix. For your own drop targets, `ctx.dragged::<P>()`, `ctx.drag_over::<P>(rect)` and `ctx.take_drop::<P>(rect)` are the parts the zone is built from.

Files dropped on the window from outside arrive in `App::file_dropped`.

## Widgets

Most widgets have a short form on `Ui` and a builder with more options, added with `ui.add`.

```rust
use rust_glass::prelude::*;

#[derive(Default)]
struct Form {
    name: String,
    password: String,
    notes: String,
    query: String,
    volume: f32,
    steps: f32,
    size: f32,
    count: i32,
    wifi: bool,
    agree: bool,
    choice: usize,
    range: usize,
    quality: usize,
    tags: usize,
}

fn form(ui: &mut Ui, f: &mut Form, color: &mut Color) {
    // Text
    ui.heading("Heading");
    ui.subheading("Subheading");
    ui.label("Body text. Wraps at the width of the region.");
    ui.secondary("Less important");
    ui.caption("Small");

    // Buttons
    ui.horizontal(|ui| {
        ui.button("Regular");
        ui.prominent_button("Primary");
        ui.add(Button::new("Clear").clear());
        ui.add(Button::new("Plain").plain());
        ui.add(Button::new("Delete").icon(Icon::Trash).tinted(Color::RED));
        ui.add(Button::new("Large").large());
        ui.icon_button(Icon::Plus).on_hover_text(ui, "New item");
    });
    if let Some(i) = ButtonGroup::new(&[(Icon::Share, "Share"), (Icon::Copy, "Copy")]).show(ui) {
        println!("clicked {i}");
    }
    let drops = [(Icon::Camera, Color::ORANGE), (Icon::Photo, Color::GREEN)];
    if let Some(i) = SplitButton::new(Icon::Plus, &drops).show(ui) {
        println!("picked {i}");
    }

    // Values
    ui.slider(&mut f.volume, 0.0..=1.0);
    ui.add(Slider::new(&mut f.steps, 0.0..=8.0).step(1.0).width(300.0).tint(Color::PURPLE));
    ui.add(Stepper::new(&mut f.count, 0..=10));
    ui.add(DragValue::new(&mut f.size).range(0.0..=100.0).speed(0.5).suffix(" pt"));
    ui.color_picker(color);

    // Choices
    ui.toggle(&mut f.wifi, "Wi-Fi");
    ui.checkbox(&mut f.agree, "Agree");
    ui.radio(&mut f.choice, 0, "First");
    ui.radio(&mut f.choice, 1, "Second");
    ui.segmented(&mut f.range, &["Day", "Week", "Month"]);
    ui.add(Segmented::new(&mut f.tags, &["All", "Photos", "Links"]).fit());
    ComboBox::new(&mut f.quality, &["Low", "High"]).width(160.0).show(ui);

    // Text input
    ui.text_edit(&mut f.name, "Name");
    ui.search_field(&mut f.query, "Search");
    ui.add(TextEdit::new(&mut f.password).hint("Password").password());
    ui.add(TextEdit::new(&mut f.notes).hint("Notes").rows(4).width(400.0));

    // Status
    ui.progress(0.4);
    ui.add(Gauge::new(0.7, "70").size(80.0).tint(Color::GREEN));
    ui.spinner();

    // Grouping
    section(ui, "Network", |ui| {
        SettingsRow::new("Wi-Fi").icon(Icon::Wifi, Color::BLUE).show(ui, |ui| {
            ui.add(Toggle::new(&mut f.wifi));
        });
    });
    ui.collapsing("Details", |ui| {
        ui.label("Hidden until opened");
    });
    table(ui, &["Name", "Size"], &[vec!["readme".into(), "4 KB".into()]]);
}
```

A `Response` tells what happened: `clicked()`, `changed()`, `hovered()`, and fields such as `held`, `double_clicked`, `secondary_clicked`, `delta` and `rect`.

`Icon::ALL` lists the built in icons. They are vectors and stay sharp at any size.

## Menus, dialogs and windows

`MenuButton` turns into its menu when clicked. Items do not close it on their own, so close it when one was chosen:

```rust
use rust_glass::prelude::*;

fn options(ui: &mut Ui, sort: &mut usize) {
    let mut close = false;
    let menu = MenuButton::new("Options").icon(Icon::Sliders).show(ui, |ui| {
        for (i, name) in ["Sort by name", "Sort by date"].iter().enumerate() {
            if ui.add(MenuItem::new(name).checked(*sort == i)).clicked() {
                *sort = i;
                close = true;
            }
        }
        ui.separator();
        close |= ui.add(MenuItem::new("Delete").icon(Icon::Trash).destructive()).clicked();
    });
    if close {
        ui.ctx.set_popup(menu.id, false);
    }
}
```

The other overlays take a `&mut bool` that says whether they are open. Call them every frame, also while closed, so they can animate out:

```rust
use rust_glass::prelude::*;

#[derive(Default)]
struct Overlays {
    confirm: bool,
    details: bool,
    inspector: bool,
}

fn overlays(ctx: &mut Context, o: &mut Overlays) {
    // The last button is the primary one.
    let buttons = ["Cancel", "Delete"];
    let picked = alert(ctx, Id::new("confirm"), &mut o.confirm, "Delete file?", "It is gone for good.", &buttons);
    if picked == Some(1) {
        ctx.toast(Some(Icon::Check), "Deleted");
    }

    // Rises from the bottom edge and can be flicked away.
    sheet(ctx, Id::new("details"), &mut o.details, 460.0, 320.0, |ui| {
        ui.heading("Details");
    });

    Window::new("Inspector").pos(vec2(300.0, 120.0)).width(260.0).open(&mut o.inspector).show(ctx, |ui| {
        ui.label("Drag me by the title. Double click to fold.");
    });
}
```

`modal` is an `alert` with your own content. Escape closes menus, alerts, modals and sheets.

`context_menu(ui.ctx, &response, width, |ui| ...)` opens a menu on a right click on the widget that returned `response`. `response.on_hover_text(ui, "text")` shows a tooltip.

## Images and wallpaper

`ctx.add_image(width, height, rgba)` uploads 8 bit RGBA pixels and returns an `ImageId`. Upload each image once, for example in `setup`, and free it with `ctx.remove_image(id)` when it is no longer needed. rust-glass does not decode files. The `image` crate does.

```rust
use rust_glass::prelude::*;

struct Gallery {
    photo: ImageId,
}

impl App for Gallery {
    fn setup(&mut self, ctx: &mut Context) {
        let (w, h) = (64, 64);
        let pixels = (0..w * h).flat_map(|i| [(i % w * 4) as u8, (i / w * 4) as u8, 160, 255]).collect();
        self.photo = ctx.add_image(w, h, pixels);
    }

    fn update(&mut self, ctx: &mut Context, _: &FrameInfo) {
        // Wallpaper. The scene is a bit larger than the window, so cover that too.
        let whole = rect(0.0, 0.0, 1.0, 1.0);
        let screen = ctx.screen.expand(64.0);
        ctx.background().image(screen, 0.0, self.photo, whole, Color::WHITE);

        region(ctx, "page", ctx.screen.shrink(40.0), |ui| {
            ui.image(self.photo, vec2(160.0, 160.0), 18.0);
        });
    }
}
```

## Theme

`ctx.theme` is a plain struct. Change it in `setup` or at any time in `update`.

```rust
use rust_glass::prelude::*;

fn configure(ctx: &mut Context) {
    ctx.theme.dark = true;
    ctx.theme.accent = Color::PINK;
    ctx.theme.glass_tint = 0.2;
    ctx.theme.body_size = 15.0;
}
```

| Field | Default | Meaning |
| --- | --- | --- |
| `dark` | `false` | Dark content. Small glass elements decide for themselves from what is behind them. |
| `accent` | `Color::BLUE` | Prominent buttons, links, focus rings. |
| `glass_tint` | `0.35` | 0 is the clearest glass, 1 is fully tinted. Also sets how clear the lifted lenses are. |
| `adaptive` | `true` | Small glass switches between light and dark with its backdrop. |
| `reduce_transparency` | `false` | Denser surfaces. |
| `reduce_motion` | `false` | No overshoot. |
| `increase_contrast` | `false` | Stronger edges. |
| `regular`, `clear`, `thick`, `lens`, `blob`, `blob_labels` | presets | Materials used by the widgets. |
| `control_height`, `spacing` | `36`, `10` | Sizes in points. |
| `body_size`, `caption_size`, `title_size` | `14`, `12`, `24` | Font sizes. |

Text uses the interface font of the platform. `ctx.text.add_font(bytes)` registers a font file, and `ctx.text.sans = "Family".into()` switches to it.

## Materials

A `Material` holds every parameter of the glass. Presets: `REGULAR` for controls and bars, `CLEAR` for controls over photos and video, `THICK` for menus and panels, `LENS` for lifted thumbs, `BLOB` for the tab bar selection.

Start from a preset and change what you need. The Material Lab page of the showcase edits most of this live.

```rust
use rust_glass::prelude::*;

const ICE: Material = Material { blur: 1.5, refraction: 30.0, depth: 22.0, ..Material::CLEAR };

fn toolbar(ctx: &mut Context) {
    let r = rect(40.0, 40.0, 300.0, 60.0);
    Panel::new("bar", r).corner(Corner::Capsule).material(ICE).adaptive().show(ctx, |ui| {
        ui.label("Over a photo");
    });

    // Colored glass. The alpha is the strength.
    let tint = Material::REGULAR.tinted(Color::TEAL.alpha(0.6));
    Panel::new("teal", rect(40.0, 120.0, 300.0, 60.0)).material(tint).show(ctx, |ui| {
        ui.label("Teal");
    });

    // Used by every widget from now on.
    ctx.theme.regular.dispersion = 0.2;
}
```

Parameters that change the look the most:

- `refraction` and `depth`: how far the edge bends light, and how wide the bent zone is.
- `dispersion`: color fringes, as a fraction of the bend.
- `blur`: blur radius of the backdrop.
- `light_range` and `dark_range`: the brightness range the backdrop is squeezed into. A narrow range near white keeps dark text legible over anything.
- `rim`, `dark_edge`, `inner_glow`: light on the edge.
- `shadow`, `shadow_radius`, `shadow_tint`: the shadow, and how much color it carries.
- `glass_tint_response`: how much the `glass_tint` setting applies. Set it to 0 for a lens that sits above its own label, or the label turns milky.

`.adaptive()` lets a panel switch between light and dark with its backdrop. Keep that for small panels.

## Custom painting and your own widgets

`ui.painter()` draws into the current region. A widget is a struct with a `ui` method:

```rust
use rust_glass::prelude::*;

struct Stars<'a> {
    rating: &'a mut usize,
}

impl Widget for Stars<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let mut r = ui.allocate_response(vec2(150.0, 30.0), Sense::Click);
        if r.clicked()
            && let Some(p) = r.pointer
        {
            *self.rating = (((p.x - r.rect.min.x) / 30.0) as usize + 1).min(5);
            r.changed = true;
        }
        let off = ui.surface.strong_fill_color();
        let mut p = ui.painter();
        for i in 0..5 {
            let center = vec2(r.rect.min.x + 15.0 + 30.0 * i as f32, r.rect.center().y);
            let color = if i < *self.rating { Color::YELLOW } else { off };
            p.icon(center, 24.0, Icon::Star, color);
        }
        r
    }
}

fn rate(ui: &mut Ui, rating: &mut usize) {
    if ui.add(Stars { rating }).changed() {
        ui.ctx.toast(None, format!("{rating} stars"));
    }
}
```

The painter has `fill`, `gradient`, `stroke`, `shadow`, `line`, `arc`, `image`, `icon`, `text`, `text_centered` and `glass`. `ui.surface` gives the text and fill colors that fit what the widget is drawn on.

For a glass element of your own, `ui.glass` measures the backdrop and returns the surface to pick the label color from:

```rust
use rust_glass::prelude::*;

fn badge(ui: &mut Ui, text: &str) {
    let layout = ui.layout_text(text, &TextStyle::new(13.0).weight(Weight::Semibold));
    let r = ui.allocate(layout.size + vec2(28.0, 14.0));
    let id = ui.next_id();
    let material = ui.ctx.theme.regular;
    let surface = ui.glass(id, r, Corner::Capsule, material, true);
    ui.painter().text_centered(r, &layout, surface.text_color(), 1.0);
}

// Two shapes in one element fuse like drops.
fn drops(ui: &mut Ui, gap: f32) {
    let r = ui.allocate(vec2(200.0, 60.0));
    let left = GlassShape::new(rect(r.min.x, r.min.y, 60.0, 60.0), Corner::Capsule);
    let right = GlassShape::new(rect(r.min.x + 60.0 + gap, r.min.y, 60.0, 60.0), Corner::Capsule);
    let mut glass = Glass::group(vec![left, right], ui.ctx.theme.regular).fuse(20.0);
    let id = ui.next_id();
    ui.ctx.prepare_glass(id, &mut glass, true);
    ui.painter().glass(&glass);
}
```

`ctx.get`, `ctx.try_get` and `ctx.set` store small state for a widget under its id and type. State of a widget that is no longer drawn is dropped after a while, so keep anything that matters in your own struct.

## Animation and repainting

The window draws only when there is input or something asked for another frame. `ctx.animate` stores a spring under an id and asks for frames until it settles:

```rust
use rust_glass::prelude::*;

fn bar(ui: &mut Ui, open: bool) {
    let id = ui.make_id("bar");
    let width = ui.ctx.animate(id, if open { 240.0 } else { 80.0 }, Tuning::PRESS);
    let r = ui.allocate(vec2(240.0, 40.0));
    let r = Rect::from_min_size(r.min, vec2(width, r.height()));
    let accent = ui.ctx.theme.accent;
    ui.painter().fill(r, Corner::Capsule, accent);
}
```

Tunings: `SNAP` (no overshoot), `SMOOTH`, `PRESS` and `BOUNCY`, or `Tuning::new(response, damping)`, with the same meaning as a SwiftUI spring.

Call `ctx.request_repaint()` in every frame while something else moves, such as a video or a timer. `ctx.dt` is the time since the last frame in seconds and `ctx.time` the time since start. Nothing can wake an idle window from another thread yet. While a background job runs, request a repaint and poll its channel in `update`.

## Keyboard, clipboard and links

Tab and Shift+Tab move the focus, and scroll areas scroll to show the focused widget. Space and Enter press, arrow keys move sliders, segments and tabs.

```rust
use rust_glass::input::Key;
use rust_glass::prelude::*;

fn shortcuts(ctx: &mut Context, saved: &mut bool) {
    // Control on Windows and Linux, Command on macOS.
    if ctx.input.modifiers.command && ctx.input.take_key(Key::Char('s')) {
        *saved = true;
        ctx.toast(Some(Icon::Check), "Saved");
    }
}

fn links(ui: &mut Ui) {
    ui.hyperlink("Source code", "https://github.com/MasterKPG/rust-glass");
    if ui.button("Copy").clicked() {
        ui.ctx.copy_text("text for the clipboard");
    }
}

fn quit(ctx: &mut Context) {
    // Handled by the window after this frame.
    ctx.host_commands.push(rust_glass::context::HostCommand::Close);
}
```

`take_key` consumes the press, so a widget drawn later does not see it. `ctx.input` also has the pointer position, buttons, scroll and typed text.

Text fields undo with Ctrl+Z and redo with Ctrl+Shift+Z or Ctrl+Y (Command on macOS). Typing undoes a word at a time, while paste, cut and deleting a selection are steps of their own. When the app changes the text itself, the history of that field starts over.

Input methods for Chinese, Japanese, Korean and other scripts work in every text field. The text being composed shows underlined at the caret, and the candidate window of the system opens next to it.

## Accessibility

Screen readers see every built in widget with its role, name, value and state, and can operate it: press buttons, flip switches, move sliders, pick tabs and segments, and set the text of fields. The tree goes to the platform through [AccessKit](https://accesskit.dev). It is built only while a screen reader listens, so an app without one pays nothing.

It has been checked with Windows UI Automation. VoiceOver on macOS and Orca on Linux have not been tried yet.

Widgets without text of their own, such as a slider, need a name:

```rust
use rust_glass::prelude::*;

fn volume(ui: &mut Ui, v: &mut f32) {
    let r = ui.slider(v, 0.0..=1.0);
    ui.ctx.access_label(r.id, "Volume");
}
```

`SettingsRow` names the controls at its end after its title, so a settings list needs nothing extra. Your own widgets describe themselves with `ui.access`, which returns `None` while no screen reader listens:

```rust
use rust_glass::accessibility::Role;
use rust_glass::prelude::*;

fn star(ui: &mut Ui, on: &mut bool) {
    let r = ui.allocate_response(vec2(28.0, 28.0), Sense::Click);
    if r.clicked {
        *on = !*on;
    }
    if let Some(n) = ui.access(r.id, Role::CheckBox, r.rect) {
        n.label("Favorite").toggled(*on);
    }
    ui.ctx.focusable(r.id);
    ui.painter().icon(r.rect.center(), 20.0, Icon::Star, Color::YELLOW.fade(if *on { 1.0 } else { 0.3 }));
}
```

A click from a screen reader arrives through `Response::clicked` like any other. Steps on sliders arrive as arrow keys. New text for a field arrives through `ctx.take_access_text(id)`, which `TextEdit` handles. In tests, `ctx.set_accessibility(true)` turns the tree on, `ctx.access_nodes()` lists what was described and `Event::Access` sends a request.

## Testing without a window

`Headless` runs an app with scripted input and fixed time steps, and returns pixels. It needs a GPU but no window:

```rust,no_run
use rust_glass::headless::Headless;
use rust_glass::input::Event;
use rust_glass::prelude::*;

fn screenshot(app: &mut impl App) -> Result<Vec<u8>, String> {
    let mut h = Headless::new(1280, 800, 1.0)?;
    app.setup(&mut h.ctx);
    h.settle(app, 2.0);
    h.step(app, 1.0 / 60.0, &[Event::PointerMoved(vec2(100.0, 100.0))]);
    h.step(app, 1.0 / 60.0, &[Event::PointerDown]);
    h.step(app, 1.0 / 60.0, &[Event::PointerUp]);
    h.settle(app, 2.0);
    Ok(h.pixels()) // RGBA, 1280 * 800 * 4 bytes
}
```

For logic only, drive a `Context` with `begin_frame` and `end_frame`. That needs no GPU. [tests/interaction.rs](https://github.com/MasterKPG/rust-glass/blob/main/crates/rust-glass/tests/interaction.rs) does it for the built in widgets.

## Common problems

**The glass looks like a flat gray sheet.** There is nothing behind it. Put an image, a gradient or scrolling content below.

**Glass near the window border looks wrong.** The scene is larger than the window, so glass at the border has something to bend. Extend the wallpaper with `ctx.screen.expand(64.0)`.

**A menu stays open after a click on an item.** Close it with `ui.ctx.set_popup(menu.id, false)`.

**Two panels share their state.** Their ids are equal. Ids of panels, regions and overlays have to be unique in the window. Windows use their title.

**A dialog disappears without an animation.** It is called only while open. Call it every frame and let the `bool` decide.

**An animation stops halfway.** Nothing asks for the next frame. Use `ctx.animate` or call `ctx.request_repaint()`.

**A screen reader reads a control without a name.** It has no text of its own. Name it with `ui.ctx.access_label(response.id, "Name")`.

**A dragged item jumps back to another one.** Its `drag_source` id comes from its position. Use something that belongs to the item, such as its name or key.

**`run` returns an error about the adapter or the surface.** The driver offers none of Vulkan, DirectX 12 and Metal, or the window cannot be drawn to. Update the GPU driver.
