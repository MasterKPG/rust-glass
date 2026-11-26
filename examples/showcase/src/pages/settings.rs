use rust_glass::prelude::*;

pub struct State {
    airplane_mode: bool,
    wifi: bool,
    bluetooth: bool,
    volume: f32,
    text_size: i32,
}

impl Default for State {
    fn default() -> Self {
        Self { airplane_mode: false, wifi: true, bluetooth: true, volume: 0.6, text_size: 3 }
    }
}

pub fn show(state: &mut State, ui: &mut Ui) {
    section(ui, "General", |ui| {
        ui.spacing = 4.0;
        SettingsRow::new("Airplane Mode").icon(Icon::Plane, Color::ORANGE).show(ui, |ui| {
            ui.add(Toggle::new(&mut state.airplane_mode));
        });
        ui.separator();
        SettingsRow::new("Wi-Fi").icon(Icon::Wifi, Color::BLUE).show(ui, |ui| {
            ui.add(Toggle::new(&mut state.wifi));
        });
        ui.separator();
        SettingsRow::new("Bluetooth").icon(Icon::Bluetooth, Color::BLUE).show(ui, |ui| {
            ui.add(Toggle::new(&mut state.bluetooth));
        });
        ui.separator();
        SettingsRow::new("Volume").icon(Icon::Volume, Color::PINK).show(ui, |ui| {
            ui.add(Slider::new(&mut state.volume, 0.0..=1.0).width(180.0).tint(Color::PINK));
        });
        ui.separator();
        SettingsRow::new("Text Size").icon(Icon::Sun, Color::INDIGO).show(ui, |ui| {
            ui.add(Stepper::new(&mut state.text_size, 0..=10));
        });
    });

    // These rows change the real theme of the showcase.
    let mut theme = ui.ctx.theme.clone();
    section(ui, "Accessibility", |ui| {
        ui.spacing = 4.0;
        SettingsRow::new("Reduce Transparency").icon(Icon::Eye, Color::BLUE).show(ui, |ui| {
            ui.add(Toggle::new(&mut theme.reduce_transparency));
        });
        ui.separator();
        SettingsRow::new("Reduce Motion").icon(Icon::Flash, Color::GREEN).show(ui, |ui| {
            ui.add(Toggle::new(&mut theme.reduce_motion));
        });
        ui.separator();
        SettingsRow::new("Increase Contrast").icon(Icon::Moon, Color::gray(0.35)).show(ui, |ui| {
            ui.add(Toggle::new(&mut theme.increase_contrast));
        });
        ui.separator();
        SettingsRow::new("Adapt Glass to Background").icon(Icon::Drop, Color::TEAL).show(ui, |ui| {
            ui.add(Toggle::new(&mut theme.adaptive));
        });
    });
    ui.ctx.theme = theme;
}
