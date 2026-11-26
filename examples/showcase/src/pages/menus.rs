use rust_glass::prelude::*;

use super::{Marks, hint};

pub struct State {
    sort_by_date: bool,
    size: usize,
    color: Color,
}

impl Default for State {
    fn default() -> Self {
        Self { sort_by_date: false, size: 1, color: Color::INDIGO }
    }
}

pub fn show(state: &mut State, marks: &mut Marks, ui: &mut Ui) {
    section(ui, "Menus", |ui| {
        hint(ui, "The button turns into the menu. Press, slide to an item and release to pick it.");
        ui.horizontal(|ui| {
            let mut close = false;
            let options = MenuButton::new("Options").icon(Icon::Sliders).show(ui, |ui| {
                close |= ui.add(MenuItem::new("New Folder").icon(Icon::Folder).shortcut("Ctrl N")).clicked();
                close |= ui.add(MenuItem::new("Duplicate").icon(Icon::Copy).shortcut("Ctrl D")).clicked();
                close |= ui.add(MenuItem::new("Share").icon(Icon::Share)).clicked();
                ui.separator();
                for (by_date, name) in [(false, "Sort by Name"), (true, "Sort by Date")] {
                    if ui.add(MenuItem::new(name).checked(state.sort_by_date == by_date)).clicked() {
                        state.sort_by_date = by_date;
                        close = true;
                    }
                }
                ui.separator();
                close |= ui.add(MenuItem::new("Delete").icon(Icon::Trash).destructive()).clicked();
            });
            marks.set("options menu", options.rect);
            if close {
                ui.ctx.set_popup(options.id, false);
            }

            let mut close = false;
            let more = MenuButton::new("").icon(Icon::Ellipsis).menu_width(180.0).show(ui, |ui| {
                for (icon, name) in
                    [(Icon::Heart, "Favorite"), (Icon::Download, "Download"), (Icon::Link, "Copy Link")]
                {
                    close |= ui.add(MenuItem::new(name).icon(icon)).clicked();
                }
            });
            if close {
                ui.ctx.set_popup(more.id, false);
            }
        });
    });

    section(ui, "Combo box", |ui| {
        ComboBox::new(&mut state.size, &["Small", "Medium", "Large", "Extra Large"]).width(190.0).show(ui);
    });

    section(ui, "Color picker", |ui| {
        ui.horizontal(|ui| {
            ui.color_picker(&mut state.color);
            let hex = format!("#{:06X}", state.color.to_hex());
            ui.label(&hex);
            ui.add(Button::new("Tinted with it").tinted(state.color));
        });
    });

    section(ui, "Context menu", |ui| {
        hint(ui, "Right click the card.");
        let rect = ui.allocate(vec2(ui.available_width().min(420.0), 120.0));
        let id = ui.make_id("context menu card");
        let response = ui.interact(id, rect, Sense::Click);
        let other = Color::from_hsv(state.color.to_hsv().0 + 0.12, 0.7, 0.9);
        ui.painter().gradient(rect, 22.0, state.color, other, vec2(0.7, 0.7));
        context_menu(ui.ctx, &response, 200.0, |ui| {
            ui.add(MenuItem::new("Open").icon(Icon::Window));
            ui.add(MenuItem::new("Rename").icon(Icon::Pencil));
            ui.add(MenuItem::new("Move to Trash").icon(Icon::Trash).destructive());
        });
    });
}
