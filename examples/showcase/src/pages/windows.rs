use rust_glass::prelude::*;

use super::{Marks, hint, section};

pub struct State {
    inspector_open: bool,
    palette_open: bool,
    sheet_open: bool,
    opacity: f32,
    snap: bool,
    period: usize,
    progress: f32,
    sheet_volume: f32,
    sheet_notifications: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            inspector_open: true,
            palette_open: true,
            sheet_open: false,
            opacity: 0.5,
            snap: true,
            period: 1,
            progress: 0.6,
            sheet_volume: 0.4,
            sheet_notifications: true,
        }
    }
}

pub fn show(state: &mut State, marks: &mut Marks, ui: &mut Ui) {
    section(ui, "Windows and sheets", |ui| {
        hint(
            ui,
            "Drag a window by its title. Double click the title to fold it. Click a window to raise it.",
        );
        ui.horizontal(|ui| {
            if ui.button("Inspector").clicked() {
                state.inspector_open = !state.inspector_open;
            }
            if ui.button("Palette").clicked() {
                state.palette_open = !state.palette_open;
            }
            let sheet = ui.add(Button::new("Open Sheet").prominent());
            marks.set("sheet button", sheet.rect);
            if sheet.clicked() {
                state.sheet_open = true;
            }
        });
    });
}

/// Windows and the sheet. They float above the page, so they are drawn
/// outside its scroll area.
pub fn overlays(state: &mut State, ctx: &mut Context, area: Rect, visible: bool) {
    if visible {
        Window::new("Inspector")
            .pos(vec2(area.min.x + 40.0, 250.0))
            .width(300.0)
            .open(&mut state.inspector_open)
            .show(ctx, |ui| {
                ui.label("Opacity");
                ui.add(Slider::new(&mut state.opacity, 0.0..=1.0));
                ui.toggle(&mut state.snap, "Snap to grid");
                ui.segmented(&mut state.period, &["Day", "Week", "Month"]);
                ui.horizontal(|ui| {
                    ui.button("Cancel");
                    ui.prominent_button("Apply");
                });
            });
        Window::new("Palette")
            .pos(vec2(area.min.x + 380.0, 290.0))
            .width(260.0)
            .open(&mut state.palette_open)
            .show(ctx, |ui| {
                ui.grid(4, 10.0, |ui, i| {
                    let color = [Color::RED, Color::ORANGE, Color::GREEN, Color::BLUE][i];
                    let width = ui.available_width();
                    ui.add(Button::icon_only(Icon::Drop).tinted(color).width(width));
                });
                ui.label("Windows bend the light of the windows below them.");
                ui.progress(state.progress);
            });
    }
    let width = 520.0_f32.min(ctx.screen.width() - 28.0);
    sheet(ctx, Id::new("sheet"), &mut state.sheet_open, width, 330.0, |ui| {
        ui.subheading("Sheet");
        ui.secondary("Drag it down by the handle, or click outside, to close it.");
        ui.add(Slider::new(&mut state.sheet_volume, 0.0..=1.0));
        ui.toggle(&mut state.sheet_notifications, "Notifications");
    });
}
