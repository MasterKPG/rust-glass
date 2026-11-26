use rust_glass::prelude::*;

use super::hint;

pub struct State {
    query: String,
    name: String,
    password: String,
    notes: String,
}

impl Default for State {
    fn default() -> Self {
        Self {
            query: String::new(),
            name: "Liquid".into(),
            password: "glasshouse".into(),
            notes:
                "Fields of several lines wrap and scroll.\nSelect with the mouse or Shift and the arrow keys."
                    .into(),
        }
    }
}

pub fn show(state: &mut State, ui: &mut Ui) {
    section(ui, "Search", |ui| {
        ui.search_field(&mut state.query, "Search");
    });

    section(ui, "One line", |ui| {
        hint(ui, "Ctrl+Z undoes a word at a time. Input methods show their text at the caret.");
        ui.add(TextEdit::new(&mut state.name).hint("Name").icon(Icon::User));
        ui.add(TextEdit::new(&mut state.password).hint("Password").password().icon(Icon::Eye));
    });

    section(ui, "Several lines", |ui| {
        ui.add(TextEdit::new(&mut state.notes).hint("Notes").rows(5));
    });

    section(ui, "Text styles", |ui| {
        ui.heading("Heading");
        ui.subheading("Subheading");
        ui.strong("Strong");
        ui.label("Body text in the interface font of the platform. It wraps and shapes any script: مرحبا, 你好, नमस्ते, 👋🏽");
        ui.secondary("Secondary");
        ui.caption("Caption");
    });
}
