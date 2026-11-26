use rust_glass::prelude::*;

use super::{Marks, hint};

pub struct State {
    volume: f32,
    brightness: f32,
    rating: f32,
    zoom: f32,
    wifi: bool,
    bluetooth: bool,
    low_power: bool,
    large_switch: bool,
    copies: i32,
    line_height: f32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            volume: 0.6,
            brightness: 0.35,
            rating: 3.0,
            zoom: 0.5,
            wifi: true,
            bluetooth: false,
            low_power: true,
            large_switch: true,
            copies: 3,
            line_height: 42.0,
        }
    }
}

/// A slider with a fixed width label before it.
fn labeled_slider(ui: &mut Ui, label: &str, slider: Slider) -> Response {
    ui.horizontal(|ui| {
        ui.vertical(110.0, |ui| ui.label(label));
        ui.add(slider)
    })
}

pub fn show(state: &mut State, marks: &mut Marks, ui: &mut Ui) {
    section(ui, "Sliders", |ui| {
        hint(ui, "While you drag, the thumb lifts into a clear lens and stretches with the motion.");
        let width = (ui.available_width() - 200.0).clamp(160.0, 360.0);
        let volume = labeled_slider(ui, "Volume", Slider::new(&mut state.volume, 0.0..=1.0).width(width));
        marks.set("volume slider", volume.rect);
        labeled_slider(
            ui,
            "Brightness",
            Slider::new(&mut state.brightness, 0.0..=1.0).width(width).tint(Color::ORANGE),
        );
        let rating = format!("Rating: {:.0}", state.rating);
        labeled_slider(
            ui,
            &rating,
            Slider::new(&mut state.rating, 0.0..=8.0).step(1.0).width(width).tint(Color::PURPLE),
        );
        labeled_slider(
            ui,
            "Large",
            Slider::new(&mut state.zoom, 0.0..=1.0).scale(1.8).width(width).tint(Color::PINK),
        );
    });

    section(ui, "Switches", |ui| {
        hint(ui, "Click or drag the thumb.");
        let wifi = ui.add(Toggle::new(&mut state.wifi).label("Wi-Fi"));
        marks.set("wifi switch", wifi.rect);
        ui.add(Toggle::new(&mut state.bluetooth).label("Bluetooth").tint(Color::BLUE));
        ui.add(Toggle::new(&mut state.low_power).label("Low Power Mode").tint(Color::ORANGE));
        ui.add(Toggle::new(&mut state.large_switch).label("Large").scale(1.6));
    });

    section(ui, "Numbers", |ui| {
        SettingsRow::new("Copies").show(ui, |ui| {
            let copies = state.copies.to_string();
            ui.label(&copies);
            ui.add(Stepper::new(&mut state.copies, 1..=10));
        });
        ui.separator();
        SettingsRow::new("Line height (drag sideways)").show(ui, |ui| {
            ui.add(DragValue::new(&mut state.line_height).speed(0.5).range(0.0..=100.0).suffix(" pt"));
        });
    });
}
