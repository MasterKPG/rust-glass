//! A lens to drag over the wallpaper, with a slider for every parameter of
//! its material.

use rust_glass::prelude::*;

/// Height of the parameter panel.
const PANEL_HEIGHT: f32 = 344.0;

pub struct State {
    pub material: Material,
    /// Center of the lens.
    pub position: Vec2,
    /// Height of the lens. It is 1.7 times as wide.
    pub size: f32,
}

impl Default for State {
    fn default() -> Self {
        Self { material: Material::CLEAR, position: vec2(620.0, 190.0), size: 170.0 }
    }
}

pub fn show(state: &mut State, ctx: &mut Context, info: &FrameInfo, area: Rect) {
    lens(state, ctx, area);
    let panel = Rect::from_min_size(
        vec2(area.min.x, area.max.y - 14.0 - PANEL_HEIGHT),
        vec2(area.width() - 14.0, PANEL_HEIGHT),
    );
    Panel::new("lab panel", panel).padding(18.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.subheading("Material");
            let presets = [
                ("Regular", Material::REGULAR),
                ("Clear", Material::CLEAR),
                ("Thick", Material::THICK),
                ("Lens", Material::LENS),
            ];
            for (name, material) in presets {
                if ui.add(Button::new(name).small()).clicked() {
                    state.material = material;
                }
            }
        });
        parameters(state, ui);
        let gpu = info.gpu.gpu_ms.map_or("not measured".to_string(), |ms| format!("{ms:.1} ms"));
        let stats = format!(
            "Last frame: UI {:.2} ms, GPU {gpu}, {} render passes, {} glass elements",
            info.ui_ms, info.gpu.passes, info.gpu.glasses
        );
        ui.caption(&stats);
    });
}

/// The lens follows the pointer on springs and stretches with its speed.
fn lens(state: &mut State, ctx: &mut Context, area: Rect) {
    let layer = ctx.layer(Id::new("lab"), Order::Content);
    let size = vec2(state.size * 1.7, state.size);
    let rect = Rect::from_center_size(state.position, size);
    let response = ctx.interact(Id::new("lens"), rect, Rect::EVERYTHING, layer, Sense::Drag);
    if response.held {
        state.position = (state.position + response.delta).max(area.min).min(area.max);
        ctx.request_repaint();
    }
    let motion = ctx.animate_vec2(Id::new("lens position"), state.position, Tuning::new(0.22, 0.55));
    let velocity = motion.velocity();
    let stretch = vec2((velocity.x.abs() / 5000.0).min(0.2), (velocity.y.abs() / 5000.0).min(0.2));
    let body = Rect::from_center_size(
        motion.value(),
        vec2(size.x * (1.0 + stretch.x - stretch.y * 0.6), size.y * (1.0 + stretch.y - stretch.x * 0.6)),
    );
    let mut glass = Glass::new(body, Corner::Radius(state.size * 0.36), state.material);
    glass.light_bend = (velocity.x / 2500.0).clamp(-0.5, 0.5);
    let backdrop = ctx.prepare_glass(Id::new("lens"), &mut glass, true);
    let label = ctx.text.layout("Drag me", &TextStyle::new(17.0).weight(Weight::Semibold));
    let mut painter = ctx.painter(layer, Rect::EVERYTHING);
    painter.glass(&glass);
    painter.text_centered(body, &label, backdrop.surface().text_color(), 1.0);
}

/// Three columns of sliders, one per parameter.
fn parameters(state: &mut State, ui: &mut Ui) {
    let m = &mut state.material;
    let mut sliders: Vec<(&str, &mut f32, f32, f32)> = vec![
        ("Refraction", &mut m.refraction, 0.0, 80.0),
        ("Depth", &mut m.depth, 1.0, 60.0),
        ("Outer ring", &mut m.outer_refraction, 0.0, 20.0),
        ("Dispersion", &mut m.dispersion, 0.0, 0.5),
        ("Blur", &mut m.blur, 0.0, 24.0),
        ("Magnify", &mut m.magnify, 0.6, 2.0),
        ("Saturation", &mut m.saturation, 0.0, 2.5),
        ("Light floor", &mut m.light_range.0, 0.0, 0.95),
        ("Rim", &mut m.rim, 0.0, 1.0),
        ("Rim width", &mut m.rim_width, 0.3, 4.0),
        ("Dark edge", &mut m.dark_edge, 0.0, 1.0),
        ("Light spill", &mut m.light_spill, 0.0, 1.0),
        ("Inner glow", &mut m.inner_glow, 0.0, 1.0),
        ("Inner shade", &mut m.inner_shadow, 0.0, 0.5),
        ("Shadow", &mut m.shadow, 0.0, 0.6),
        ("Shadow color", &mut m.shadow_tint, 0.0, 1.0),
        ("Shadow size", &mut m.shadow_radius, 2.0, 60.0),
        ("Lens size", &mut state.size, 60.0, 260.0),
    ];
    let rows = sliders.len().div_ceil(3);
    ui.grid(3, 26.0, |ui, column| {
        ui.spacing = 4.0;
        for (name, value, min, max) in sliders.iter_mut().skip(column * rows).take(rows) {
            ui.horizontal(|ui| {
                let width = ui.max_rect.width();
                ui.vertical(90.0, |ui| ui.caption(name));
                ui.add(Slider::new(value, *min..=*max).width((width - 100.0).max(60.0)));
            });
        }
    });
}
