//! The pages of the showcase. Each module draws one page and owns its state.

mod buttons;
mod controls;
mod feedback;
mod lab;
mod lists;
mod menus;
mod player;
mod selection;
mod settings;
mod text;
mod windows;

use std::collections::HashMap;

use rust_glass::prelude::*;

use crate::Showcase;

/// A page of the showcase, in sidebar order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Player,
    Buttons,
    Controls,
    Selection,
    Text,
    Menus,
    Feedback,
    Windows,
    Lists,
    Settings,
    Lab,
}

impl Page {
    pub const ALL: [Page; 11] = [
        Page::Player,
        Page::Buttons,
        Page::Controls,
        Page::Selection,
        Page::Text,
        Page::Menus,
        Page::Feedback,
        Page::Windows,
        Page::Lists,
        Page::Settings,
        Page::Lab,
    ];

    /// Icon and title of every page, for the sidebar.
    pub const NAVIGATION: [(Icon, &'static str); 11] = [
        (Icon::Music, "Player"),
        (Icon::Drop, "Buttons"),
        (Icon::Sliders, "Sliders & Switches"),
        (Icon::Layers, "Segments & Tabs"),
        (Icon::Keyboard, "Text Fields"),
        (Icon::Ellipsis, "Menus & Pickers"),
        (Icon::Bell, "Feedback"),
        (Icon::Window, "Windows & Sheets"),
        (Icon::Grid, "Lists & Drag"),
        (Icon::Gear, "Settings"),
        (Icon::Palette, "Material Lab"),
    ];

    pub fn title(self) -> &'static str {
        Self::NAVIGATION[self as usize].1
    }
}

/// Rectangles of widgets that the capture code presses and drags.
#[derive(Default)]
pub struct Marks(HashMap<&'static str, Rect>);

impl Marks {
    pub fn set(&mut self, name: &'static str, rect: Rect) {
        self.0.insert(name, rect);
    }

    /// Rectangle of a marked widget. Panics if the widget was never drawn.
    pub fn rect(&self, name: &str) -> Rect {
        *self.0.get(name).unwrap_or_else(|| panic!("nothing marked as {name}"))
    }
}

/// State of every page, so it survives switching pages.
pub struct State {
    shown: Option<Page>,
    /// Runs from 0 to 1 while a page appears.
    appear: Spring,
    pub marks: Marks,
    player: player::State,
    buttons: buttons::State,
    controls: controls::State,
    selection: selection::State,
    text: text::State,
    menus: menus::State,
    feedback: feedback::State,
    windows: windows::State,
    lists: lists::State,
    settings: settings::State,
    pub lab: lab::State,
}

impl Default for State {
    fn default() -> Self {
        Self {
            shown: None,
            appear: Spring::new(1.0),
            marks: Marks::default(),
            player: Default::default(),
            buttons: Default::default(),
            controls: Default::default(),
            selection: Default::default(),
            text: Default::default(),
            menus: Default::default(),
            feedback: Default::default(),
            windows: Default::default(),
            lists: Default::default(),
            settings: Default::default(),
            lab: Default::default(),
        }
    }
}

/// Draws the current page into `area`, the window right of the sidebar.
pub fn show(app: &mut Showcase, ctx: &mut Context, info: &FrameInfo, area: Rect) {
    let state = &mut app.pages;
    match app.page {
        Page::Player => player::show(&mut state.player, &mut state.marks, &app.covers, ctx, area),
        Page::Lab => lab::show(&mut state.lab, ctx, info, area),
        page => scrolling_page(state, ctx, area, page),
    }
    windows::overlays(&mut state.windows, ctx, area, app.page == Page::Windows);
}

/// A page that is a title and a column of sections.
fn scrolling_page(state: &mut State, ctx: &mut Context, area: Rect, page: Page) {
    // A new page forms out of nothing: it slides up a little and fades in.
    if state.shown != Some(page) {
        state.shown = Some(page);
        state.appear = Spring::new(0.0);
    }
    let tuning = ctx.tuning(Tuning::new(0.42, 1.0));
    if state.appear.step(1.0, tuning, ctx.dt) {
        ctx.request_repaint();
    }
    let appear = state.appear.value;
    let rect = Rect { min: area.min + vec2(14.0, 0.0), max: area.max - vec2(10.0, 0.0) }
        .translate(vec2(0.0, 12.0 * (1.0 - appear)));
    let marks = &mut state.marks;
    region(ctx, ("page", page as usize), rect, |ui| {
        ui.opacity = appear;
        ScrollArea::new().fade(36.0, 36.0).insets(34.0, 40.0).show(ui, |ui| {
            ui.max_rect.max.x -= 18.0;
            ui.spacing = 14.0;
            page_title(ui, page.title());
            match page {
                Page::Buttons => buttons::show(&mut state.buttons, marks, ui),
                Page::Controls => controls::show(&mut state.controls, marks, ui),
                Page::Selection => selection::show(&mut state.selection, marks, ui),
                Page::Text => text::show(&mut state.text, ui),
                Page::Menus => menus::show(&mut state.menus, marks, ui),
                Page::Feedback => feedback::show(&mut state.feedback, marks, ui),
                Page::Windows => windows::show(&mut state.windows, marks, ui),
                Page::Lists => lists::show(&mut state.lists, marks, ui),
                Page::Settings => settings::show(&mut state.settings, ui),
                Page::Player | Page::Lab => {}
            }
        });
    });
}

/// The page title, on a soft shade so it reads over any wallpaper.
fn page_title(ui: &mut Ui, title: &str) {
    let style = TextStyle::new(ui.ctx.theme.title_size + 4.0).weight(Weight::Bold);
    let layout = ui.layout_text(title, &style);
    let rect = ui.allocate(layout.size);
    let dark = ui.surface.darkness() > 0.5;
    let shade = if dark { Color::black(0.32) } else { Color::white(0.55) };
    let color = ui.surface.text_color();
    let mut painter = ui.painter();
    painter.shadow(rect.expand2(vec2(10.0, 2.0)), 14.0, 18.0, shade);
    painter.text(rect.min, &layout, color);
}

/// A short explanation under a section title.
fn hint(ui: &mut Ui, text: &str) {
    let style = TextStyle::new(ui.ctx.theme.body_size - 1.0);
    let color = ui.surface.secondary_color();
    ui.styled_text(text, style, color);
}
