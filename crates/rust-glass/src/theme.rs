//! Look and behavior settings shared by every widget.

use crate::color::Color;
use crate::material::Material;

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// Content appearance. Glass adapts to its backdrop on its own.
    pub dark: bool,
    pub accent: Color,
    /// 0 is the clearest glass, 1 the most frosted. The system wide
    /// transparency setting of iOS 27.
    pub frost: f32,
    /// Small elements flip between light and dark with the backdrop.
    pub adaptive: bool,
    pub reduce_transparency: bool,
    pub reduce_motion: bool,
    pub high_contrast: bool,

    pub regular: Material,
    pub clear: Material,
    pub thick: Material,
    pub lens: Material,

    pub control_height: f32,
    pub spacing: f32,
    pub body: f32,
    pub caption: f32,
    pub title: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            dark: false,
            accent: Color::BLUE,
            frost: 0.0,
            adaptive: true,
            reduce_transparency: false,
            reduce_motion: false,
            high_contrast: false,
            regular: Material::REGULAR,
            clear: Material::CLEAR,
            thick: Material::THICK,
            lens: Material::LENS,
            control_height: 36.0,
            spacing: 10.0,
            body: 14.0,
            caption: 12.0,
            title: 24.0,
        }
    }
}

impl Theme {
    pub fn text(&self) -> Color {
        if self.dark { Color::gray(0.97) } else { Color::gray(0.04) }
    }
    pub fn background(&self) -> Color {
        if self.dark { Color::gray(0.07) } else { Color::gray(0.95) }
    }
}

/// What a widget is drawn on. Decides label and fill colors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Surface {
    /// Directly on app content.
    Content { dark: bool },
    /// On a glass element with the given darkness, 0 to 1.
    Glass { dark: f32 },
}

impl Surface {
    pub fn dark(&self) -> f32 {
        match self {
            Surface::Content { dark } => *dark as u32 as f32,
            Surface::Glass { dark } => *dark,
        }
    }
    pub fn is_glass(&self) -> bool {
        matches!(self, Surface::Glass { .. })
    }
    pub fn text(&self) -> Color {
        Color::gray(0.04).lerp(Color::gray(0.99), self.dark())
    }
    pub fn secondary(&self) -> Color {
        Color::black(0.52).lerp(Color::white(0.62), self.dark())
    }
    pub fn tertiary(&self) -> Color {
        Color::black(0.30).lerp(Color::white(0.36), self.dark())
    }
    /// Track and well fill.
    pub fn fill(&self) -> Color {
        Color::black(0.085).lerp(Color::white(0.13), self.dark())
    }
    pub fn fill_strong(&self) -> Color {
        Color::black(0.15).lerp(Color::white(0.22), self.dark())
    }
    pub fn separator(&self) -> Color {
        Color::black(0.10).lerp(Color::white(0.14), self.dark())
    }
    /// Opaque knob color at rest.
    pub fn knob(&self) -> Color {
        Color::WHITE.lerp(Color::gray(0.96), self.dark())
    }
}
