use rust_glass::prelude::*;

use super::{Marks, hint, section};

const TABS: [(Icon, &str); 5] = [
    (Icon::Home, "Home"),
    (Icon::Grid, "New"),
    (Icon::Wifi, "Radio"),
    (Icon::Music, "Library"),
    (Icon::Search, "Search"),
];

#[derive(Default)]
pub struct State {
    library: usize,
    period: usize,
    filter: usize,
    tab: usize,
    on_glass_period: usize,
    on_glass_level: f32,
    on_glass_switch: bool,
}

pub fn show(state: &mut State, marks: &mut Marks, ui: &mut Ui) {
    section(ui, "Segmented controls", |ui| {
        hint(ui, "Click a segment, or drag the selection. It lifts into a lens on the way.");
        let library = ui.segmented(&mut state.library, &["For You", "Library"]);
        marks.set("segmented control", library.rect);
        ui.segmented(&mut state.period, &["Day", "Week", "Month", "Year"]);
        ui.add(Segmented::new(&mut state.filter, &["All", "Invitations", "Photos", "Files", "Links"]).fit());
    });

    section(ui, "Tab bar", |ui| {
        hint(ui, "Drag across the bar. The selection lifts into a clear blob that stands out from it.");
        let space = ui.allocate(vec2(ui.available_width(), 96.0));
        let center = vec2(space.min.x + 230.0, space.center().y);
        if ui.clip.contains(center) {
            TabBar::new(&mut state.tab, &TABS).show_at(ui.ctx, Id::new("demo tab bar"), center);
            marks.set("tab bar", Rect::from_center_size(center, vec2(400.0, 62.0)));
        }
    });

    section(ui, "On a glass panel", |ui| {
        hint(ui, "Controls take their colors from the glass below them.");
        let material = ui.ctx.theme.thick;
        ui.glass_panel(material, |ui| {
            ui.segmented(&mut state.on_glass_period, &["Day", "Week", "Month", "Year"]);
            ui.add(Slider::new(&mut state.on_glass_level, 0.0..=1.0));
            ui.toggle(&mut state.on_glass_switch, "Switch on glass");
        });
    });
}
