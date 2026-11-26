use rust_glass::paint::ImageId;
use rust_glass::prelude::*;

use crate::Showcase;

pub struct State {
    shown: usize,
    enter: Spring,
    playing: bool,
    position: f32,
    tab: usize,
    liked: bool,
    clicks: u32,
    volume: f32,
    bright: f32,
    stepped: f32,
    big: f32,
    toggles: [bool; 5],
    count: i32,
    drag: f32,
    seg: usize,
    seg3: usize,
    seg5: usize,
    tabbar: usize,
    name: String,
    pass: String,
    query: String,
    notes: String,
    combo: usize,
    sort: usize,
    color: Color,
    checks: [bool; 3],
    radio: usize,
    progress: f32,
    alert: bool,
    sheet: bool,
    win_a: bool,
    win_b: bool,
    win_v: f32,
    win_t: bool,
    plane: bool,
    wifi: bool,
    bt: bool,
    ref_v: f32,
    ref_seg: usize,
    ref_toggle: bool,
    tasks: Vec<String>,
    boards: [Vec<String>; 2],
    pub lab: Material,
    pub lab_pos: Vec2,
    pub lab_size: f32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            shown: usize::MAX,
            enter: Spring::new(1.0),
            playing: true,
            position: 0.34,
            tab: 0,
            liked: true,
            clicks: 0,
            volume: 0.6,
            bright: 0.35,
            stepped: 3.0,
            big: 0.5,
            toggles: [true, false, true, false, true],
            count: 3,
            drag: 42.0,
            seg: 0,
            seg3: 1,
            seg5: 2,
            tabbar: 0,
            name: "Liquid".into(),
            pass: "glasshouse".into(),
            query: String::new(),
            notes: "Text fields wrap, scroll and select.\nUse the arrow keys, Ctrl+A, Ctrl+C and Ctrl+V."
                .into(),
            combo: 1,
            sort: 0,
            color: Color::INDIGO,
            checks: [true, false, true],
            radio: 0,
            progress: 0.62,
            alert: false,
            sheet: false,
            win_a: true,
            win_b: true,
            win_v: 0.5,
            win_t: true,
            plane: false,
            wifi: true,
            bt: true,
            ref_v: 0.54,
            ref_seg: 0,
            ref_toggle: true,
            tasks: ["Pick up the glass", "Sand the edges", "Polish", "Mount the frame", "Hang it"]
                .map(String::from)
                .to_vec(),
            boards: [
                ["Write the release notes", "Record the clips", "Test on Linux"].map(String::from).to_vec(),
                ["Accessibility tree".to_string()].to_vec(),
            ],
            lab: Material::CLEAR,
            lab_pos: vec2(620.0, 190.0),
            lab_size: 170.0,
        }
    }
}

pub fn show(app: &mut Showcase, ctx: &mut Context, main: Rect) {
    match app.page {
        0 => player(app, ctx, main),
        10 => lab(app, ctx, main),
        11 => reference(app, ctx, main),
        n => scrolled(app, ctx, main, n),
    }
    overlays(app, ctx, main);
}

/// Pages that are a list of sections.
fn scrolled(app: &mut Showcase, ctx: &mut Context, main: Rect, page: usize) {
    let r = Rect { min: main.min + vec2(14.0, 0.0), max: main.max - vec2(10.0, 0.0) };
    // A new page forms out of nothing: glass fades in by its optics.
    if app.s.shown != page {
        app.s.shown = page;
        app.s.enter = Spring::new(0.0);
    }
    let tune = ctx.tuning(Tuning::new(0.42, 1.0));
    if app.s.enter.step(1.0, tune, ctx.dt) {
        ctx.request_repaint();
    }
    let enter = app.s.enter.value;
    let r = r.translate(vec2(0.0, 12.0 * (1.0 - enter)));
    region(ctx, ("page", page), r, |ui| {
        ui.opacity = enter;
        ScrollArea::new().fade(36.0, 36.0).insets(34.0, 40.0).show(ui, |ui| {
            ui.max_rect.max.x -= 18.0;
            ui.spacing = 14.0;
            ui.heading(crate::PAGES[page].1);
            let s = &mut app.s;
            match page {
                1 => buttons(s, ui),
                2 => sliders(s, ui),
                3 => segments(s, ui),
                4 => fields(s, ui),
                5 => menus(s, ui),
                6 => feedback(s, ui),
                7 => windows(s, ui),
                8 => lists(s, ui),
                _ => settings(s, ui),
            }
        });
    });
}

fn title(ui: &mut Ui, t: &str) {
    ui.space(8.0);
    let c = ui.surface.text().fade(0.78);
    ui.styled_text(t, TextStyle::new(13.0).weight(Weight::Semibold), c);
}

fn buttons(s: &mut State, ui: &mut Ui) {
    title(ui, "Glass");
    ui.horizontal(|ui| {
        if ui.add(Button::new("Regular").icon(Icon::Drop)).clicked() {
            s.clicks += 1;
        }
        if ui.add(Button::new("Prominent").prominent()).clicked() {
            s.clicks += 1;
        }
        ui.add(Button::new("Clear").clear());
        ui.add(Button::new("Plain").plain());
        ui.add_enabled(false, |ui| ui.add(Button::new("Disabled")));
        let t = format!("{} clicks", s.clicks);
        ui.secondary(&t);
    });
    title(ui, "Sizes");
    ui.horizontal(|ui| {
        ui.add(Button::new("Small").small());
        ui.add(Button::new("Regular"));
        ui.add(Button::new("Large").large().icon(Icon::Star));
        ui.add(Button::new("Large").large().prominent());
        ui.add(Button::new("Rounded").large().corner(16.0));
    });
    title(ui, "Tints");
    ui.horizontal(|ui| {
        for (n, c) in [
            ("Red", Color::RED),
            ("Orange", Color::ORANGE),
            ("Green", Color::GREEN),
            ("Teal", Color::TEAL),
            ("Indigo", Color::INDIGO),
            ("Pink", Color::PINK),
        ] {
            ui.add(Button::new(n).tinted(c));
        }
    });
    title(ui, "Icon buttons");
    ui.horizontal(|ui| {
        for i in [Icon::Plus, Icon::Search, Icon::Share, Icon::Trash] {
            ui.add(Button::icon_only(i)).on_hover_text(ui, i.name());
        }
        ui.add(Button::icon_only(Icon::Play).large().clear());
        ui.add(Button::icon_only(Icon::Phone).large().tinted(Color::GREEN));
        let r = ui.add(Button::icon_only(Icon::Heart).large().selected(s.liked));
        if r.clicked() {
            s.liked = !s.liked;
        }
    });
    title(ui, "Grouped on one surface");
    ui.horizontal(|ui| {
        if let Some(i) =
            ButtonGroup::new(&[(Icon::ChevronLeft, "Back"), (Icon::ChevronRight, "Forward")]).show(ui)
        {
            s.clicks += i as u32 + 1;
        }
        ButtonGroup::new(&[
            (Icon::Share, "Share"),
            (Icon::Copy, "Copy"),
            (Icon::Pencil, "Edit"),
            (Icon::Ellipsis, "More"),
        ])
        .show(ui);
    });
    title(ui, "Fused drops: press the plus");
    if let Some(i) = SplitButton::new(
        Icon::Plus,
        &[
            (Icon::Camera, Color::ORANGE),
            (Icon::Photo, Color::GREEN),
            (Icon::Music, Color::PINK),
            (Icon::Mail, Color::BLUE),
        ],
    )
    .show(ui)
    {
        let name = ["Camera", "Photo", "Music", "Mail"][i];
        ui.ctx.toast(Some(Icon::Check), name);
    }
    title(ui, "Link");
    ui.link("Underlines on hover");
}

fn sliders(s: &mut State, ui: &mut Ui) {
    title(ui, "Slider");
    ui.horizontal(|ui| {
        ui.add(Slider::new(&mut s.volume, 0.0..=1.0).width(320.0));
        let t = format!("{:.0} %", s.volume * 100.0);
        ui.label(&t);
    });
    ui.horizontal(|ui| {
        ui.add(Slider::new(&mut s.bright, 0.0..=1.0).width(320.0).tint(Color::ORANGE));
        ui.label("Tinted");
    });
    ui.horizontal(|ui| {
        ui.add(Slider::new(&mut s.stepped, 0.0..=8.0).step(1.0).width(320.0).tint(Color::PURPLE));
        let t = format!("Step {:.0}", s.stepped);
        ui.label(&t);
    });
    ui.add(Slider::new(&mut s.big, 0.0..=1.0).scale(1.9).width(460.0).tint(Color::PINK));
    title(ui, "Switch");
    ui.horizontal(|ui| {
        for (i, c) in [Color::GREEN, Color::BLUE, Color::ORANGE, Color::PINK].iter().enumerate() {
            ui.add(Toggle::new(&mut s.toggles[i]).tint(*c));
        }
        ui.add(Toggle::new(&mut s.toggles[4]).scale(1.7));
    });
    title(ui, "Stepper and drag value");
    ui.horizontal(|ui| {
        ui.add(Stepper::new(&mut s.count, 0..=10));
        let t = s.count.to_string();
        ui.label(&t);
        ui.space(20.0);
        ui.add(DragValue::new(&mut s.drag).speed(0.5).range(0.0..=100.0).suffix(" pt"));
        ui.secondary("drag sideways");
    });
    title(ui, "On a card");
    ui.card(|ui| {
        ui.toggle(&mut s.toggles[0], "Notifications");
        ui.separator();
        ui.toggle(&mut s.toggles[1], "Focus");
        ui.separator();
        ui.add(Slider::new(&mut s.volume, 0.0..=1.0));
    });
}

fn segments(s: &mut State, ui: &mut Ui) {
    title(ui, "Segmented control: press, drag, flick");
    ui.segmented(&mut s.seg, &["For You", "Library"]);
    ui.segmented(&mut s.seg3, &["Day", "Week", "Month", "Year"]);
    ui.add(Segmented::new(&mut s.seg5, &["All", "Invitations", "Photos", "Files", "Links"]).fit());
    ui.add(Segmented::new(&mut s.seg, &["For You", "Library"]).scale(1.6));
    title(ui, "Tab bar");
    let c = vec2(ui.cursor.x + 250.0, ui.cursor.y + 60.0);
    ui.space(110.0);
    let id = Id::new("demo tabs");
    let clip = ui.clip;
    if clip.contains(c) {
        TabBar::new(
            &mut s.tabbar,
            &[
                (Icon::Home, "Home"),
                (Icon::Grid, "New"),
                (Icon::Wifi, "Radio"),
                (Icon::Music, "Library"),
                (Icon::Search, "Search"),
            ],
        )
        .show_at(ui.ctx, id, c);
    }
    title(ui, "On glass");
    let m = ui.ctx.theme.thick;
    ui.glass_panel(m, |ui| {
        ui.segmented(&mut s.seg3, &["Day", "Week", "Month", "Year"]);
        ui.add(Slider::new(&mut s.bright, 0.0..=1.0));
        ui.toggle(&mut s.toggles[2], "Switch on glass");
    });
}

fn fields(s: &mut State, ui: &mut Ui) {
    title(ui, "Search");
    ui.search_field(&mut s.query, "Search");
    title(ui, "Text");
    ui.add(TextEdit::new(&mut s.name).hint("Name").icon(Icon::User));
    ui.add(TextEdit::new(&mut s.pass).hint("Password").password().icon(Icon::Eye));
    title(ui, "Multi line");
    ui.add(TextEdit::new(&mut s.notes).hint("Notes").rows(5).width(440.0));
    title(ui, "Type scale");
    ui.card(|ui| {
        ui.heading("Heading");
        ui.subheading("Subheading");
        ui.strong("Strong body");
        ui.label("Body text in the interface font of the platform. Scales, wraps and shapes complex scripts: مرحبا, 你好, नमस्ते, 👋🏽");
        ui.secondary("Secondary");
        ui.caption("Caption");
    });
}

fn menus(s: &mut State, ui: &mut Ui) {
    title(ui, "Menu: the button becomes the menu");
    ui.horizontal(|ui| {
        let mut close = false;
        let r = MenuButton::new("Options").icon(Icon::Sliders).show(ui, |ui| {
            close |= ui.add(MenuItem::new("New Folder").icon(Icon::Folder).shortcut("Ctrl N")).clicked();
            close |= ui.add(MenuItem::new("Duplicate").icon(Icon::Copy).shortcut("Ctrl D")).clicked();
            close |= ui.add(MenuItem::new("Share").icon(Icon::Share)).clicked();
            ui.separator();
            for (i, n) in ["Sort by Name", "Sort by Date"].iter().enumerate() {
                if ui.add(MenuItem::new(n).checked(s.sort == i)).clicked() {
                    s.sort = i;
                    close = true;
                }
            }
            ui.separator();
            close |= ui.add(MenuItem::new("Delete").icon(Icon::Trash).destructive()).clicked();
        });
        if close {
            ui.ctx.set_popup(r.id, false);
        }
        let mut close = false;
        let r = MenuButton::new("").icon(Icon::Ellipsis).menu_width(180.0).show(ui, |ui| {
            for (i, n) in [(Icon::Heart, "Favorite"), (Icon::Download, "Download"), (Icon::Link, "Copy Link")]
            {
                close |= ui.add(MenuItem::new(n).icon(i)).clicked();
            }
        });
        if close {
            ui.ctx.set_popup(r.id, false);
        }
    });
    title(ui, "Combo box");
    ComboBox::new(&mut s.combo, &["Small", "Medium", "Large", "Extra Large"]).width(190.0).show(ui);
    title(ui, "Color picker");
    ui.horizontal(|ui| {
        ui.color_picker(&mut s.color);
        let t = format!("#{:06X}", s.color.to_hex());
        ui.label(&t);
        ui.add(Button::new("Tinted with it").tinted(s.color));
    });
    title(ui, "Context menu: right click the card");
    let r = ui.allocate(vec2(ui.available_width().min(420.0), 130.0));
    let id = ui.make_id("ctx card");
    let res = ui.interact(id, r, Sense::Click);
    let (a, b) = (s.color, Color::from_hsv(s.color.to_hsv().0 + 0.12, 0.7, 0.9));
    ui.painter().gradient(r, 22.0, a, b, vec2(0.7, 0.7));
    context_menu(ui.ctx, &res, 200.0, |ui| {
        ui.add(MenuItem::new("Open").icon(Icon::Window));
        ui.add(MenuItem::new("Rename").icon(Icon::Pencil));
        ui.add(MenuItem::new("Move to Trash").icon(Icon::Trash).destructive());
    });
}

fn feedback(s: &mut State, ui: &mut Ui) {
    title(ui, "Checkbox and radio");
    ui.horizontal(|ui| {
        ui.checkbox(&mut s.checks[0], "Lensing");
        ui.checkbox(&mut s.checks[1], "Dispersion");
        ui.checkbox(&mut s.checks[2], "Fresnel rim");
    });
    ui.horizontal(|ui| {
        for (i, n) in ["Regular", "Clear", "Tinted"].iter().enumerate() {
            ui.radio(&mut s.radio, i, n);
        }
    });
    title(ui, "Progress");
    ui.progress(s.progress);
    ui.horizontal(|ui| {
        let pct = format!("{:.0}", s.progress * 100.0);
        ui.add(Gauge::new(s.progress, &pct).size(84.0).tint(Color::GREEN));
        ui.add(Gauge::new(s.volume, "Vol").size(84.0).tint(Color::PINK));
        ui.add(Gauge::new(s.bright, "Sun").size(84.0).tint(Color::ORANGE));
        ui.spinner();
        if ui.button("Randomize").clicked() {
            s.progress = (s.progress + 0.37).fract();
            s.volume = (s.volume + 0.53).fract();
            s.bright = (s.bright + 0.29).fract();
        }
    });
    title(ui, "Toast and alert");
    ui.horizontal(|ui| {
        if ui.add(Button::new("Show Toast").icon(Icon::Bell)).clicked() {
            ui.ctx.toast(Some(Icon::Check), "Saved to Library");
        }
        if ui.add(Button::new("Show Alert").icon(Icon::Warning)).clicked() {
            s.alert = true;
        }
    });
    title(ui, "Collapsing section");
    ui.card(|ui| {
        ui.collapsing("Details", |ui| {
            ui.label("Content unfolds with a spring and fades in as it opens.");
            ui.checkbox(&mut s.checks[1], "Inside");
        });
        ui.collapsing("More", |ui| {
            ui.secondary("Second section.");
        });
    });
    title(ui, "Table");
    ui.card(|ui| {
        table(
            ui,
            &["Layer", "Source", "Cost"],
            &[
                vec!["Lens".into(), "Backdrop".into(), "1 to 7 samples".into()],
                vec!["Blur".into(), "Mip chain".into(), "8 samples".into()],
                vec!["Rim".into(), "Distance field".into(), "0 samples".into()],
            ],
        );
    });
}

fn windows(s: &mut State, ui: &mut Ui) {
    title(ui, "Drag the windows by their title, double click to fold");
    ui.horizontal(|ui| {
        if ui.button("Inspector").clicked() {
            s.win_a = !s.win_a;
        }
        if ui.button("Palette").clicked() {
            s.win_b = !s.win_b;
        }
        if ui.add(Button::new("Open Sheet").prominent()).clicked() {
            s.sheet = true;
        }
    });
}

const TILES: &[(&str, Color, Color)] = &[
    ("Coast", Color::TEAL, Color::BLUE),
    ("Dune", Color::ORANGE, Color::PINK),
    ("Moss", Color::GREEN, Color::TEAL),
    ("Dusk", Color::INDIGO, Color::PINK),
    ("Ember", Color::RED, Color::ORANGE),
    ("Lagoon", Color::BLUE, Color::GREEN),
    ("Iris", Color::PURPLE, Color::INDIGO),
    ("Citrus", Color::YELLOW, Color::GREEN),
];

fn lists(s: &mut State, ui: &mut Ui) {
    title(ui, "Horizontal");
    ScrollArea::horizontal().id("tiles").height(150.0).fade_sides(28.0, 28.0).show(ui, |ui| {
        for i in 0..24 {
            let (name, a, b) = TILES[i % TILES.len()];
            let r = ui.allocate(vec2(170.0, 124.0));
            let mut p = ui.painter();
            p.shadow(r.translate(vec2(0.0, 4.0)), 22.0, 14.0, Color::black(0.18));
            p.gradient(r, 22.0, a, b, vec2(0.6, 1.0));
            let st = TextStyle::new(15.0).weight(Weight::Semibold);
            let l = ui.layout_text(&format!("{name} {}", i + 1), &st);
            ui.painter().text(r.min + vec2(16.0, r.height() - 16.0 - l.size.y), &l, Color::WHITE);
        }
    });
    ui.caption("Swipe sideways or hold Shift while turning the wheel.");

    title(ui, "Two axes");
    ui.card(|ui| {
        ScrollArea::both().id("sheet").height(210.0).content_width(1290.0).show(ui, |ui| {
            ui.spacing = 0.0;
            for row in 0..40 {
                ui.horizontal(|ui| {
                    ui.spacing = 0.0;
                    for col in 0..15 {
                        let r = ui.allocate(vec2(86.0, 30.0));
                        let c = if (row + col) % 2 == 0 { ui.surface.fill() } else { Color::TRANSPARENT };
                        ui.painter().fill(r.shrink(1.0), 6.0, c);
                        let name = format!("{}{}", (b'A' + col as u8) as char, row + 1);
                        let st = TextStyle::new(13.0);
                        let l = ui.layout_text(&name, &st);
                        let tc = ui.surface.secondary();
                        ui.painter().text(r.center() - l.size * 0.5, &l, tc);
                    }
                });
            }
        });
    });

    title(ui, "100 000 rows, only the visible ones are laid out");
    ui.card(|ui| {
        ScrollArea::new().id("long").height(240.0).show_rows(ui, 32.0, 100_000, |ui, rows| {
            for i in rows {
                let w = ui.available_width() - 12.0;
                let r = ui.allocate(vec2(w, 32.0));
                if i % 2 == 0 {
                    let c = ui.surface.fill();
                    ui.painter().fill(r, 9.0, c);
                }
                let (tc, sc) = (ui.surface.text(), ui.surface.secondary());
                let a =
                    ui.layout_text(&format!("Row {}", i + 1), &TextStyle::new(14.0).weight(Weight::Medium));
                let b = ui.layout_text(&format!("#{:05X}", i * 7919 % 0xFFFFF), &TextStyle::new(13.0));
                let mut p = ui.painter();
                p.text(vec2(r.min.x + 12.0, r.center().y - a.size.y * 0.5), &a, tc);
                p.text(vec2(r.max.x - 12.0 - b.size.x, r.center().y - b.size.y * 0.5), &b, sc);
            }
        });
    });

    title(ui, "Drag to reorder");
    ui.card(|ui| {
        ui.spacing = 6.0;
        ui.reorder("tasks", &mut s.tasks, |ui, t| task_row(ui, t));
    });

    title(ui, "Drag between lists");
    let w = ((ui.available_width() - 16.0) * 0.5).max(120.0);
    let mut moved = None;
    ui.horizontal(|ui| {
        for (k, name) in ["To do", "Done"].iter().enumerate() {
            ui.vertical(w, |ui| {
                ui.strong(name);
                let items = &s.boards[k];
                let (_, got) = ui.drop_zone::<(usize, usize), _>(("board", k), vec2(w, 150.0), |ui| {
                    ui.spacing = 6.0;
                    for (i, t) in items.iter().enumerate() {
                        ui.drag_source(("card", t.as_str()), (k, i), |ui| task_row_width(ui, t, w));
                    }
                });
                if let Some((from, i)) = got
                    && from != k
                {
                    moved = Some((from, i, k));
                }
            });
        }
    });
    if let Some((from, i, to)) = moved {
        let t = s.boards[from].remove(i);
        s.boards[to].push(t);
    }
    ui.caption("Escape puts a dragged item back.");
}

fn task_row(ui: &mut Ui, t: &str) {
    let w = ui.available_width();
    task_row_width(ui, t, w);
}

fn task_row_width(ui: &mut Ui, t: &str, w: f32) {
    let r = ui.allocate(vec2(w, 40.0));
    let (fill, tc, sc) = (ui.surface.fill(), ui.surface.text(), ui.surface.tertiary());
    let l = ui.layout_text(t, &TextStyle::new(15.0));
    let mut p = ui.painter();
    p.fill(r, 12.0, fill);
    for k in 0..3 {
        let y = r.center().y - 5.0 + 5.0 * k as f32;
        p.line(vec2(r.max.x - 30.0, y), vec2(r.max.x - 16.0, y), 1.6, sc);
    }
    p.text(vec2(r.min.x + 14.0, r.center().y - l.size.y * 0.5), &l, tc);
    if let Some(n) = ui.access(ui.make_id(("task", t)), rust_glass::accessibility::Role::Label, r) {
        n.label(t);
    }
}

fn settings(s: &mut State, ui: &mut Ui) {
    ui.card(|ui| {
        ui.spacing = 4.0;
        SettingsRow::new("Airplane Mode").icon(Icon::Plane, Color::ORANGE).show(ui, |ui| {
            ui.add(Toggle::new(&mut s.plane));
        });
        ui.separator();
        SettingsRow::new("Wi-Fi").icon(Icon::Wifi, Color::BLUE).show(ui, |ui| {
            ui.add(Toggle::new(&mut s.wifi));
        });
        ui.separator();
        SettingsRow::new("Bluetooth").icon(Icon::Bluetooth, Color::BLUE).show(ui, |ui| {
            ui.add(Toggle::new(&mut s.bt));
        });
        ui.separator();
        SettingsRow::new("Sounds").icon(Icon::Volume, Color::PINK).show(ui, |ui| {
            ui.add(Slider::new(&mut s.volume, 0.0..=1.0).width(180.0).tint(Color::PINK));
        });
        ui.separator();
        SettingsRow::new("Text Size").icon(Icon::Sun, Color::INDIGO).show(ui, |ui| {
            ui.add(Stepper::new(&mut s.count, 0..=10));
        });
    });
    title(ui, "Accessibility");
    let mut t = ui.ctx.theme.clone();
    ui.card(|ui| {
        ui.spacing = 4.0;
        SettingsRow::new("Reduce Transparency").icon(Icon::Eye, Color::BLUE).show(ui, |ui| {
            ui.add(Toggle::new(&mut t.reduce_transparency));
        });
        ui.separator();
        SettingsRow::new("Reduce Motion").icon(Icon::Flash, Color::GREEN).show(ui, |ui| {
            ui.add(Toggle::new(&mut t.reduce_motion));
        });
        ui.separator();
        SettingsRow::new("Increase Contrast").icon(Icon::Moon, Color::gray(0.35)).show(ui, |ui| {
            ui.add(Toggle::new(&mut t.high_contrast));
        });
        ui.separator();
        SettingsRow::new("Adapt to Background").icon(Icon::Drop, Color::TEAL).show(ui, |ui| {
            ui.add(Toggle::new(&mut t.adaptive));
        });
    });
    ui.ctx.theme = t;
}

const SONGS: &[(&str, &str)] = &[
    ("Slow Light", "Nadir Field"),
    ("Glass Harbor", "Ana Vell"),
    ("Refraction", "Low Tide Choir"),
    ("Paper Moons", "Iris & the Static"),
    ("Ferry at Six", "Odeon"),
    ("Salt Lines", "Mira Holt"),
    ("Afterglow", "Northern Post"),
    ("Two Rivers", "Kestrel"),
    ("Clear Weather", "Sun Parlor"),
    ("Night Ferry", "Odeon"),
    ("Undertow", "Ana Vell"),
    ("Halcyon", "Mira Holt"),
];

/// Scrolling content under floating glass, the case the material is made for.
fn player(app: &mut Showcase, ctx: &mut Context, main: Rect) {
    let covers = app.covers.clone();
    let s = &mut app.s;
    let page = Rect { min: main.min, max: main.max };
    region(ctx, "player", page, |ui| {
        let bg = ui.ctx.theme.background();
        let sheet = Rect { min: vec2(main.min.x, 14.0), max: vec2(main.max.x - 14.0, main.max.y - 14.0) };
        ui.painter().fill(sheet, 26.0, bg);
        let mask = ui.painter().add_mask(sheet, 26.0, false, Default::default());
        ui.mask = mask;
        ui.clip = sheet;
        ui.in_rect("body", sheet, |ui| {
            ScrollArea::new().insets(24.0, 170.0).show(ui, |ui| {
                ui.spacing = 14.0;
                ui.padded(vec2(26.0, 0.0), |ui| {
                    ui.heading("New Releases");
                    let n = ((ui.available_width() / 190.0) as usize).clamp(2, 5);
                    let mut i = 0;
                    while i < SONGS.len() {
                        ui.grid(n, 20.0, |ui, k| {
                            let Some((t, a)) = SONGS.get(i + k) else { return };
                            ui.spacing = 3.0;
                            let w = ui.available_width();
                            let id = covers.get(i + k).copied().unwrap_or(ImageId(0));
                            ui.image(id, vec2(w, w), 14.0);
                            ui.space(4.0);
                            ui.strong(t);
                            ui.secondary(a);
                        });
                        i += n;
                    }
                    ui.heading("Made for You");
                    for (i, (t, a)) in SONGS.iter().enumerate().take(8) {
                        ui.horizontal(|ui| {
                            let id = covers.get((i + 5) % covers.len().max(1)).copied().unwrap_or(ImageId(0));
                            ui.image(id, vec2(48.0, 48.0), 9.0);
                            ui.vertical(260.0, |ui| {
                                ui.spacing = 1.0;
                                ui.strong(t);
                                ui.secondary(a);
                            });
                        });
                        ui.separator();
                    }
                });
            });
        });
    });

    if s.playing {
        s.position = (s.position + ctx.dt / 210.0).fract();
    }
    let cx = (main.min.x + main.max.x - 14.0) * 0.5;
    let bottom = main.max.y - 30.0;
    let w = 452.0;
    let bar = Rect::from_center_size(vec2(cx, bottom - 62.0 - 8.0 - 34.0), vec2(w, 60.0));
    let m = ctx.theme.regular;
    Panel::new("mini player", bar).corner(Corner::Capsule).material(m).padding(0.0).flips().show(ctx, |ui| {
        let art = Rect::from_min_size(bar.min + vec2(14.0, 11.0), vec2(38.0, 38.0));
        let cover = covers.get(2).copied().unwrap_or(ImageId(0));
        ui.painter().image(art, 9.0, cover, rect(0.0, 0.0, 1.0, 1.0), Color::WHITE);
        let t = Rect { min: vec2(art.max.x + 12.0, bar.min.y + 12.0), max: bar.max };
        ui.in_rect("t", t, |ui| {
            ui.spacing = 0.0;
            ui.strong(SONGS[2].0);
            ui.caption(SONGS[2].1);
        });
        let ctl = Rect { min: vec2(bar.max.x - 148.0, bar.min.y), max: bar.max };
        ui.row_in_rect("c", ctl, |ui| {
            ui.spacing = 4.0;
            let c = ui.surface.text();
            for (k, i) in [Icon::Backward, if s.playing { Icon::Pause } else { Icon::Play }, Icon::Forward]
                .into_iter()
                .enumerate()
            {
                let r = ui.allocate_response(vec2(42.0, 42.0), Sense::Click);
                let g = rust_glass::widgets::press::PressEffect::update(ui.ctx, &r, 1.0);
                let f = ui.surface.fill();
                let mut p = ui.painter();
                p.fill(g.rect, Corner::Capsule, f.fade(g.hover));
                p.icon(g.rect.center(), 22.0 * g.scale, i, c);
                if r.clicked() && k == 1 {
                    s.playing = !s.playing;
                }
            }
        });
    });
    TabBar::new(
        &mut s.tab,
        &[
            (Icon::Home, "Home"),
            (Icon::Grid, "New"),
            (Icon::Wifi, "Radio"),
            (Icon::Music, "Library"),
            (Icon::Search, "Search"),
        ],
    )
    .show_at(ctx, Id::new("player tabs"), vec2(cx, bottom - 31.0));
}

/// Move a lens around and edit every parameter of the material.
fn lab(app: &mut Showcase, ctx: &mut Context, main: Rect) {
    let s = &mut app.s;
    let layer = ctx.layer(Id::new("lab"), Order::Content);
    let sz = vec2(s.lab_size * 1.7, s.lab_size);
    let r = Rect::from_center_size(s.lab_pos, sz);
    let res = ctx.interact(Id::new("lens"), r, Rect::EVERYTHING, layer, Sense::Drag);
    if res.held {
        s.lab_pos += res.delta;
        s.lab_pos = s.lab_pos.max(main.min).min(main.max);
        ctx.request_repaint();
    }
    let v = ctx.animate_vec2(Id::new("lens pos"), s.lab_pos, Tuning::new(0.22, 0.55));
    let vel = v.velocity();
    let (sx, sy) = ((vel.x.abs() / 5000.0).min(0.2), (vel.y.abs() / 5000.0).min(0.2));
    let body =
        Rect::from_center_size(v.value(), vec2(sz.x * (1.0 + sx - sy * 0.6), sz.y * (1.0 + sy - sx * 0.6)));
    let mut g = Glass::new(body, Corner::Radius(s.lab_size * 0.36), s.lab);
    g.light_bend = (vel.x / 2500.0).clamp(-0.5, 0.5);
    let a = ctx.prepare_glass(Id::new("lens"), &mut g, true);
    let l = ctx.text.layout("Drag me", &TextStyle::new(17.0).weight(Weight::Semibold));
    let mut p = ctx.painter(layer, Rect::EVERYTHING);
    p.glass(&g);
    p.text_centered(body, &l, a.surface().text(), 1.0);

    let panel =
        Rect::from_min_size(vec2(main.min.x, main.max.y - 14.0 - 352.0), vec2(main.width() - 14.0, 352.0));
    Panel::new("lab panel", panel).padding(18.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.subheading("Material");
            for (n, m) in [
                ("Regular", Material::REGULAR),
                ("Clear", Material::CLEAR),
                ("Thick", Material::THICK),
                ("Lens", Material::LENS),
            ] {
                if ui.add(Button::new(n).small()).clicked() {
                    s.lab = m;
                }
            }
        });
        let m = &mut s.lab;
        let size = &mut s.lab_size;
        let mut items: Vec<(&str, &mut f32, f32, f32)> = vec![
            ("Refraction", &mut m.refraction, 0.0, 80.0),
            ("Depth", &mut m.depth, 1.0, 60.0),
            ("Outer ring", &mut m.outer_refraction, 0.0, 20.0),
            ("Dispersion", &mut m.dispersion, 0.0, 0.5),
            ("Blur", &mut m.blur, 0.0, 24.0),
            ("Magnify", &mut m.magnify, 0.6, 2.0),
            ("Saturation", &mut m.saturation, 0.0, 2.5),
            ("Light floor", &mut m.light_range.0, 0.0, 0.95),
            ("Rim", &mut m.rim, 0.0, 1.0),
            ("Rim width", &mut m.rim_width, 0.3, 4.0),
            ("Dark edge", &mut m.dark_edge, 0.0, 1.0),
            ("Light spill", &mut m.light_spill, 0.0, 1.0),
            ("Inner glow", &mut m.inner_glow, 0.0, 1.0),
            ("Inner shade", &mut m.inner_shadow, 0.0, 0.5),
            ("Shadow", &mut m.shadow, 0.0, 0.6),
            ("Shadow color", &mut m.shadow_tint, 0.0, 1.0),
            ("Shadow size", &mut m.shadow_radius, 2.0, 60.0),
            ("Size", size, 60.0, 260.0),
        ];
        ui.grid(3, 26.0, |ui, col| {
            ui.spacing = 4.0;
            for (n, v, lo, hi) in items.iter_mut().skip(col * 6).take(6) {
                ui.horizontal(|ui| {
                    let w = ui.max_rect.width();
                    ui.vertical(82.0, |ui| ui.caption(n));
                    ui.add(Slider::new(v, *lo..=*hi).width((w - 92.0).max(60.0)));
                });
            }
        });
    });
}

/// The two clips of the adoption guide, live.
fn reference(app: &mut Showcase, ctx: &mut Context, main: Rect) {
    let s = &mut app.s;
    let r = Rect { min: main.min + vec2(0.0, 14.0), max: main.max - vec2(14.0, 14.0) };
    region(ctx, "reference", r, |ui| {
        let dark = ui.ctx.theme.dark;
        let bg = if dark { Color::gray(0.13) } else { Color::hex(0xDADADA) };
        ui.painter().fill(r, 26.0, bg);
        let knob = if dark { Color::gray(0.30) } else { Color::hex(0xDDDDDD) };
        ui.padded(vec2(40.0, 34.0), |ui| {
            ui.spacing = 46.0;
            ui.secondary("Press and drag. Same geometry as the reference clips.");
            let w = ui.available_width().min(620.0);
            ui.add(
                Slider::new(&mut s.ref_v, 0.0..=1.0)
                    .scale(2.2)
                    .width(w)
                    .thumb_color(knob)
                    .tint(Color::hex(0x318DE2)),
            );
            ui.add(
                Segmented::new(&mut s.ref_seg, &["For You", "Library"]).fit().scale(2.1).thumb_color(knob),
            );
            ui.horizontal(|ui| {
                ui.add(Toggle::new(&mut s.ref_toggle).scale(2.2));
                ui.space(20.0);
                ui.add(
                    Segmented::new(&mut s.seg3, &["Day", "Week", "Month", "Year"])
                        .scale(1.4)
                        .thumb_color(knob),
                );
            });
            ui.add(
                Slider::new(&mut s.bright, 0.0..=1.0)
                    .scale(1.3)
                    .width(w)
                    .thumb_color(knob)
                    .tint(Color::ORANGE),
            );
        });
    });
}

fn overlays(app: &mut Showcase, ctx: &mut Context, main: Rect) {
    let s = &mut app.s;
    if app.page == 7 {
        Window::new("Inspector").pos(vec2(main.min.x + 60.0, 190.0)).width(300.0).open(&mut s.win_a).show(
            ctx,
            |ui| {
                ui.add(Slider::new(&mut s.win_v, 0.0..=1.0));
                ui.toggle(&mut s.win_t, "Snap to grid");
                ui.segmented(&mut s.seg3, &["Day", "Week", "Month", "Year"]);
                ui.horizontal(|ui| {
                    ui.button("Cancel");
                    ui.prominent_button("Apply");
                });
            },
        );
        Window::new("Palette").pos(vec2(main.min.x + 300.0, 330.0)).width(280.0).open(&mut s.win_b).show(
            ctx,
            |ui| {
                ui.grid(4, 10.0, |ui, i| {
                    let c = [Color::RED, Color::ORANGE, Color::GREEN, Color::BLUE][i];
                    let w = ui.available_width();
                    ui.add(Button::icon_only(Icon::Drop).tinted(c).width(w));
                });
                ui.label("Windows stack, refract each other and rise when clicked.");
                ui.progress(s.progress);
            },
        );
    }
    if let Some(i) = alert(
        ctx,
        Id::new("alert"),
        &mut s.alert,
        "Delete Album?",
        "The photos stay in your library.",
        &["Cancel", "Delete"],
    ) && i == 1
    {
        ctx.toast(Some(Icon::Trash), "Album deleted");
    }
    let (h, mut open) = (330.0, s.sheet);
    sheet(ctx, Id::new("sheet"), &mut open, 520.0_f32.min(ctx.screen.width() - 28.0), h, |ui| {
        ui.subheading("Sheet");
        ui.secondary("Pull it down to dismiss.");
        ui.add(Slider::new(&mut s.volume, 0.0..=1.0));
        ui.toggle(&mut s.toggles[0], "Notifications");
        ui.segmented(&mut s.seg, &["For You", "Library"]);
    });
    s.sheet = open;
}
