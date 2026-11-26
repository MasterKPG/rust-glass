use rust_glass::prelude::*;

use super::{Marks, hint};

pub struct State {
    notify: bool,
    sounds: bool,
    badges: bool,
    quality: usize,
    progress: f32,
    battery: f32,
    storage: f32,
    alert_open: bool,
    autoplay: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            notify: true,
            sounds: false,
            badges: true,
            quality: 0,
            progress: 0.62,
            battery: 0.8,
            storage: 0.35,
            alert_open: false,
            autoplay: true,
        }
    }
}

pub fn show(state: &mut State, marks: &mut Marks, ui: &mut Ui) {
    section(ui, "Checkboxes and radio buttons", |ui| {
        ui.horizontal(|ui| {
            ui.checkbox(&mut state.notify, "Notifications");
            ui.checkbox(&mut state.sounds, "Sounds");
            ui.checkbox(&mut state.badges, "Badges");
        });
        ui.horizontal(|ui| {
            for (i, name) in ["Automatic", "High", "Low"].iter().enumerate() {
                ui.radio(&mut state.quality, i, name);
            }
        });
    });

    section(ui, "Progress", |ui| {
        ui.progress(state.progress);
        ui.horizontal(|ui| {
            let done = format!("{:.0}%", state.progress * 100.0);
            ui.add(Gauge::new(state.progress, &done).size(76.0));
            ui.add(Gauge::new(state.battery, "Battery").size(76.0).tint(Color::ORANGE));
            ui.add(Gauge::new(state.storage, "Disk").size(76.0).tint(Color::BLUE));
            ui.spinner();
            if ui.button("Change values").clicked() {
                state.progress = (state.progress + 0.37).fract();
                state.battery = (state.battery + 0.53).fract();
                state.storage = (state.storage + 0.29).fract();
            }
        });
    });

    section(ui, "Toasts and alerts", |ui| {
        ui.horizontal(|ui| {
            let toast = ui.add(Button::new("Show Toast").icon(Icon::Bell));
            marks.set("toast button", toast.rect);
            if toast.clicked() {
                ui.ctx.toast(Some(Icon::Check), "Saved to Library");
            }
            let alert_button = ui.add(Button::new("Show Alert").icon(Icon::Warning));
            marks.set("alert button", alert_button.rect);
            if alert_button.clicked() {
                state.alert_open = true;
            }
        });
    });

    section(ui, "Collapsing sections", |ui| {
        ui.collapsing("Playback", |ui| {
            ui.label("The content unfolds on a spring and fades in as it opens.");
            ui.checkbox(&mut state.autoplay, "Play the next song");
        });
        ui.collapsing("About", |ui| {
            ui.secondary("rust-glass showcase");
        });
    });

    section(ui, "Table", |ui| {
        hint(ui, "What one glass element costs on the GPU.");
        table(
            ui,
            &["Layer", "Source", "Samples per pixel"],
            &[
                vec!["Lens".into(), "Backdrop".into(), "1 to 16".into()],
                vec!["Blur".into(), "Mip chain".into(), "8".into()],
                vec!["Rim".into(), "Distance field".into(), "0".into()],
            ],
        );
    });

    let buttons = ["Cancel", "Delete"];
    let id = Id::new("delete album alert");
    let message = "The photos stay in your library.";
    if alert(ui.ctx, id, &mut state.alert_open, "Delete Album?", message, &buttons) == Some(1) {
        ui.ctx.toast(Some(Icon::Trash), "Album deleted");
    }
}
