//! A music app: content scrolls below a floating mini player and tab bar,
//! the case Liquid Glass is made for.

use rust_glass::context::Cursor;
use rust_glass::input::Key;
use rust_glass::prelude::*;
use rust_glass::widgets::press::PressEffect;

use super::Marks;

const SONGS: [(&str, &str); 12] = [
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

const TABS: [(Icon, &str); 5] = [
    (Icon::Home, "Home"),
    (Icon::Grid, "New"),
    (Icon::Wifi, "Radio"),
    (Icon::Music, "Library"),
    (Icon::Search, "Search"),
];

/// The song in the mini player.
const NOW_PLAYING: usize = 2;

pub struct State {
    playing: bool,
    /// Position in the song, 0 to 1.
    position: f32,
    tab: usize,
    /// The album shown in the detail view, if one is open.
    album: Option<usize>,
    /// Runs from 0 to 1 while the list or the detail view appears.
    appear: Spring,
}

impl Default for State {
    fn default() -> Self {
        Self { playing: true, position: 0.34, tab: 0, album: None, appear: Spring::new(1.0) }
    }
}

/// The view that shows an album cover, for [`Context::morph`].
fn cover_view(detail: bool) -> Id {
    Id::new(if detail { "album detail" } else { "album grid" })
}

pub fn show(state: &mut State, marks: &mut Marks, covers: &[ImageId], ctx: &mut Context, area: Rect) {
    let cover = |i: usize| covers.get(i % covers.len().max(1)).copied().unwrap_or_default();
    let sheet = Rect { min: vec2(area.min.x, 14.0), max: vec2(area.max.x - 14.0, area.max.y - 14.0) };
    region(ctx, "player", area, |ui| {
        let background = ui.ctx.theme.background_color();
        ui.painter().fill(sheet, 26.0, background);
        ui.mask = ui.painter().add_mask(sheet, 26.0, false, Default::default());
        ui.clip = sheet;
        let tuning = ui.ctx.tuning(Tuning::new(0.34, 1.0));
        if state.appear.step(1.0, tuning, ui.ctx.dt) {
            ui.ctx.request_repaint();
        }
        let appear = state.appear.value;
        let before = state.album;
        ui.in_rect("content", sheet, |ui| {
            // The bottom inset keeps the last rows clear of the floating bars.
            let (_, rect) = ui.scope(|ui| {
                ScrollArea::new()
                    .id(if before.is_some() { "album" } else { "songs" })
                    .insets(24.0, 170.0)
                    .show(ui, |ui| {
                        ui.spacing = 14.0;
                        ui.padded(vec2(26.0, 0.0), |ui| match before {
                            Some(album) => album_detail(ui, &mut state.album, album, &cover, marks, appear),
                            None => songs(ui, &mut state.album, &cover, marks, appear),
                        });
                    });
            });
            marks.set("player content", rect);
        });
        // The list and the album page come into focus as they appear. The
        // flying cover is on a layer of its own and stays sharp.
        ui.painter().blur(sheet.shrink(1.0), 25.0, 8.0 * (1.0 - appear));
        if state.album != before {
            // The click came in a frame that still drew the old view. Ask for
            // the next one now, or the window waits for the pointer to move.
            state.appear = Spring::new(0.0);
            ui.ctx.request_repaint();
        }
    });

    if state.playing {
        state.position = (state.position + ctx.dt / 210.0).fract();
    }
    let center_x = sheet.center().x;
    let bottom = area.max.y - 30.0;
    mini_player(
        state,
        ctx,
        &cover,
        Rect::from_center_size(vec2(center_x, bottom - 104.0), vec2(452.0, 60.0)),
    );
    let tab_center = vec2(center_x, bottom - 31.0);
    TabBar::new(&mut state.tab, &TABS).show_at(ctx, Id::new("player tabs"), tab_center);
    marks.set("player tab bar", Rect::from_center_size(tab_center, vec2(400.0, 62.0)));
}

/// Draws an album cover that morphs between the grid and the detail view.
/// Returns the response of its place in the layout.
fn album_cover(ui: &mut Ui, album: usize, image: ImageId, size: f32, detail: bool) -> Response {
    let response = ui.allocate_response(vec2(size, size), Sense::Click);
    let key = Id::new(("album cover", album));
    let shown = ui.ctx.morph(key, cover_view(detail), response.rect);
    let corner = if detail { 22.0 } else { 14.0 };
    if ui.ctx.is_morphing(key) {
        // On its way, the cover flies above everything else on the page. Its
        // shadow sinks back as it lands, so arriving has no visible step.
        let rest = response.rect;
        let away = (shown.center() - rest.center()).length() + (shown.size() - rest.size()).length();
        let lifted = (away / 60.0).min(1.0);
        let layer = ui.ctx.layer(Id::new("flying cover"), Order::Content);
        let mut painter = ui.ctx.painter(layer, ui.clip);
        painter.mask = ui.mask;
        painter.shadow(shown.translate(vec2(0.0, 8.0 * lifted)), corner, 18.0, Color::black(0.25 * lifted));
        painter.image(shown, corner, image, rect(0.0, 0.0, 1.0, 1.0), Color::WHITE);
    } else {
        ui.painter().image(shown, corner, image, rect(0.0, 0.0, 1.0, 1.0), Color::WHITE);
    }
    if response.hovered() {
        ui.ctx.cursor = Cursor::Pointer;
    }
    response
}

fn songs(
    ui: &mut Ui,
    open: &mut Option<usize>,
    cover: &dyn Fn(usize) -> ImageId,
    marks: &mut Marks,
    appear: f32,
) {
    ui.opacity = appear;
    ui.heading("New Releases");
    let columns = ((ui.available_width() / 190.0) as usize).clamp(2, 5);
    for start in (0..SONGS.len()).step_by(columns) {
        ui.grid(columns, 20.0, |ui, column| {
            let album = start + column;
            let Some((title, artist)) = SONGS.get(album) else { return };
            ui.spacing = 3.0;
            let width = ui.available_width();
            let opacity = std::mem::replace(&mut ui.opacity, 1.0);
            let response = album_cover(ui, album, cover(album), width, false);
            ui.opacity = opacity;
            if album == 0 {
                marks.set("first album", response.rect);
            }
            if response.clicked() {
                *open = Some(album);
            }
            ui.space(4.0);
            ui.strong(title);
            ui.secondary(artist);
        });
    }
    ui.heading("Made for You");
    for (i, (title, artist)) in SONGS.iter().enumerate().take(8) {
        ui.horizontal(|ui| {
            ui.image(cover(i + 5), vec2(48.0, 48.0), 9.0);
            ui.vertical(260.0, |ui| {
                ui.spacing = 1.0;
                ui.strong(title);
                ui.secondary(artist);
            });
        });
        ui.separator();
    }
}

fn album_detail(
    ui: &mut Ui,
    open: &mut Option<usize>,
    album: usize,
    cover: &dyn Fn(usize) -> ImageId,
    marks: &mut Marks,
    appear: f32,
) {
    let (title, artist) = SONGS[album];
    ui.opacity = appear;
    let back = ui.add(Button::new("New Releases").icon(Icon::ChevronLeft).plain());
    marks.set("album back", back.rect);
    if back.clicked() || ui.ctx.input.take_key(Key::Escape) {
        *open = None;
    }
    ui.horizontal(|ui| {
        ui.opacity = 1.0;
        album_cover(ui, album, cover(album), 260.0, true);
        ui.opacity = appear;
        ui.vertical(320.0, |ui| {
            ui.heading(title);
            ui.subheading(artist);
            ui.secondary("Album, 2026");
            ui.space(8.0);
            ui.horizontal(|ui| {
                ui.add(Button::new("Play").icon(Icon::Play).prominent());
                ui.add(Button::new("Shuffle"));
            });
        });
    });
    for (i, (song, _)) in SONGS.iter().cycle().skip(album).take(6).enumerate() {
        ui.horizontal(|ui| {
            ui.vertical(32.0, |ui| ui.secondary(&(i + 1).to_string()));
            ui.label(song);
        });
        ui.separator();
    }
}

fn mini_player(state: &mut State, ctx: &mut Context, cover: &dyn Fn(usize) -> ImageId, bar: Rect) {
    let material = ctx.theme.regular;
    Panel::new("mini player", bar).corner(Corner::Capsule).material(material).padding(0.0).adaptive().show(
        ctx,
        |ui| {
            let art = Rect::from_min_size(bar.min + vec2(14.0, 11.0), vec2(38.0, 38.0));
            ui.painter().image(art, 9.0, cover(NOW_PLAYING), rect(0.0, 0.0, 1.0, 1.0), Color::WHITE);
            let text = Rect { min: vec2(art.max.x + 12.0, bar.min.y + 12.0), max: bar.max };
            ui.in_rect("song", text, |ui| {
                ui.spacing = 0.0;
                ui.strong(SONGS[NOW_PLAYING].0);
                ui.caption(SONGS[NOW_PLAYING].1);
            });
            let controls = Rect { min: vec2(bar.max.x - 148.0, bar.min.y), max: bar.max };
            ui.row_in_rect("controls", controls, |ui| {
                ui.spacing = 4.0;
                let play_pause = if state.playing { Icon::Pause } else { Icon::Play };
                for icon in [Icon::Backward, play_pause, Icon::Forward] {
                    let response = ui.allocate_response(vec2(42.0, 42.0), Sense::Click);
                    let press = PressEffect::update(ui.ctx, &response, 1.0);
                    let (fill, color) = (ui.surface.fill_color(), ui.surface.text_color());
                    let mut painter = ui.painter();
                    painter.fill(press.rect, Corner::Capsule, fill.fade(press.hover));
                    painter.icon(press.rect.center(), 22.0 * press.scale, icon, color);
                    if response.clicked() && icon == play_pause {
                        state.playing = !state.playing;
                    }
                }
            });
        },
    );
}
