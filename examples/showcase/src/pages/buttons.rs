use rust_glass::prelude::*;

use super::{Marks, hint};

pub struct State {
    liked: bool,
}

impl Default for State {
    fn default() -> Self {
        Self { liked: true }
    }
}

pub fn show(state: &mut State, marks: &mut Marks, ui: &mut Ui) {
    section(ui, "Styles", |ui| {
        hint(ui, "Glass buttons swell and stretch towards the pointer while pressed.");
        ui.horizontal(|ui| {
            let regular = ui.add(Button::new("Regular").icon(Icon::Drop));
            marks.set("regular button", regular.rect);
            ui.add(Button::new("Prominent").prominent());
            ui.add(Button::new("Clear").clear());
            ui.add(Button::new("Plain").plain());
            ui.add_enabled(false, |ui| ui.add(Button::new("Disabled")));
        });
    });

    section(ui, "Sizes", |ui| {
        ui.horizontal(|ui| {
            ui.add(Button::new("Small").small());
            ui.add(Button::new("Regular"));
            ui.add(Button::new("Large").large().icon(Icon::Star));
        });
    });

    section(ui, "Tinted", |ui| {
        ui.horizontal(|ui| {
            let colors = [
                ("Red", Color::RED),
                ("Orange", Color::ORANGE),
                ("Green", Color::GREEN),
                ("Teal", Color::TEAL),
                ("Indigo", Color::INDIGO),
                ("Pink", Color::PINK),
            ];
            for (name, color) in colors {
                ui.add(Button::new(name).tinted(color));
            }
        });
    });

    section(ui, "Icon buttons", |ui| {
        hint(ui, "Hover a button to see its name. The heart stays on when clicked.");
        ui.horizontal(|ui| {
            for icon in [Icon::Plus, Icon::Search, Icon::Share, Icon::Trash] {
                ui.add(Button::icon_only(icon)).on_hover_text(ui, icon.name());
            }
            ui.add(Button::icon_only(Icon::Phone).large().tinted(Color::GREEN));
            if ui.add(Button::icon_only(Icon::Heart).large().selected(state.liked)).clicked() {
                state.liked = !state.liked;
            }
        });
    });

    section(ui, "Button groups", |ui| {
        hint(ui, "Related actions share one piece of glass.");
        ui.horizontal(|ui| {
            ButtonGroup::new(&[(Icon::ChevronLeft, "Back"), (Icon::ChevronRight, "Forward")]).show(ui);
            ButtonGroup::new(&[
                (Icon::Share, "Share"),
                (Icon::Copy, "Copy"),
                (Icon::Pencil, "Edit"),
                (Icon::Ellipsis, "More"),
            ])
            .show(ui);
        });
    });

    section(ui, "Split button", |ui| {
        hint(ui, "Press the plus. It splits into drops that pull apart and fuse again when it closes.");
        let items = [
            (Icon::Camera, Color::ORANGE),
            (Icon::Photo, Color::GREEN),
            (Icon::Music, Color::PINK),
            (Icon::Mail, Color::BLUE),
        ];
        let (picked, rect) = ui.scope(|ui| SplitButton::new(Icon::Plus, &items).show(ui));
        marks.set("split button", Rect::from_min_size(rect.min, vec2(rect.height(), rect.height())));
        if let Some(i) = picked {
            ui.ctx.toast(Some(Icon::Check), ["Camera", "Photo", "Music", "Mail"][i]);
        }
    });

    section(ui, "Links", |ui| {
        ui.hyperlink("rust-glass on GitHub", "https://github.com/MasterKPG/rust-glass");
    });
}
