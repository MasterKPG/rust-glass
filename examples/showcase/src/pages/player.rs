//! A music app: content scrolls below a floating mini player and tab bar,
//! the case Liquid Glass is made for.

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
}

impl Default for State {
    fn default() -> Self {
        Self { playing: true, position: 0.34, tab: 0 }
    }
}

pub fn show(state: &mut State, marks: &mut Marks, covers: &[ImageId], ctx: &mut Context, area: Rect) {
    let cover = |i: usize| covers.get(i % covers.len().max(1)).copied().unwrap_or_default();
    let sheet = Rect { min: vec2(area.min.x, 14.0), max: vec2(area.max.x - 14.0, area.max.y - 14.0) };
    region(ctx, "player", area, |ui| {
        let background = ui.ctx.theme.background_color();
        ui.painter().fill(sheet, 26.0, background);
        ui.mask = ui.painter().add_mask(sheet, 26.0, false, Default::default());
        ui.clip = sheet;
        ui.in_rect("content", sheet, |ui| {
            // The bottom inset keeps the last rows clear of the floating bars.
            let (_, rect) = ui.scope(|ui| {
                ScrollArea::new().insets(24.0, 170.0).show(ui, |ui| {
                    ui.spacing = 14.0;
                    ui.padded(vec2(26.0, 0.0), |ui| songs(ui, &cover));
                });
            });
            marks.set("player content", rect);
        });
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

fn songs(ui: &mut Ui, cover: &dyn Fn(usize) -> ImageId) {
    ui.heading("New Releases");
    let columns = ((ui.available_width() / 190.0) as usize).clamp(2, 5);
    for start in (0..SONGS.len()).step_by(columns) {
        ui.grid(columns, 20.0, |ui, column| {
            let Some((title, artist)) = SONGS.get(start + column) else { return };
            ui.spacing = 3.0;
            let width = ui.available_width();
            ui.image(cover(start + column), vec2(width, width), 14.0);
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
