//! Appearance and accessibility settings shared by every widget.

use crate::color::Color;
use crate::material::Material;

/// Appearance and accessibility settings. Change them through
/// [`Context::theme`](crate::Context::theme) at any time.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// Dark appearance for content. Small glass elements still switch between
    /// light and dark on their own, depending on what is behind them.
    pub dark: bool,
    /// Color of prominent buttons, selections and focus rings.
    pub accent: Color,
    /// How tinted glass is, from 0 (clearest) to 1 (fully tinted). This is
    /// the system wide transparency setting of iOS 27. The material presets
    /// are tuned for [`Theme::DEFAULT_GLASS_TINT`].
    pub glass_tint: f32,
    /// Lets small glass elements switch between light and dark with what is
    /// behind them.
    pub adaptive: bool,
    /// Makes glass nearly opaque.
    pub reduce_transparency: bool,
    /// Replaces bouncy springs with short, calm ones and turns off stretching.
    pub reduce_motion: bool,
    /// Strengthens outlines and edges.
    pub increase_contrast: bool,

    /// Material of controls and bars.
    pub regular: Material,
    /// Material of clear buttons over media.
    pub clear: Material,
    /// Material of menus, sheets, windows and panels.
    pub thick: Material,
    /// Material of a lifted slider, switch or segment thumb.
    pub lens: Material,
    /// Material of the lifted tab bar selection.
    pub blob: Material,
    /// Material that bends the tab labels under the lifted selection.
    pub blob_labels: Material,

    /// Height of buttons and fields, in points.
    pub control_height: f32,
    /// Default gap between widgets, in points.
    pub spacing: f32,
    /// Font size of body text.
    pub body_size: f32,
    /// Font size of captions.
    pub caption_size: f32,
    /// Font size of headings.
    pub title_size: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            dark: false,
            accent: Color::BLUE,
            glass_tint: Theme::DEFAULT_GLASS_TINT,
            adaptive: true,
            reduce_transparency: false,
            reduce_motion: false,
            increase_contrast: false,
            regular: Material::REGULAR,
            clear: Material::CLEAR,
            thick: Material::THICK,
            lens: Material::LENS,
            blob: Material::BLOB,
            blob_labels: Material::BLOB_LABELS,
            control_height: 36.0,
            spacing: 10.0,
            body_size: 14.0,
            caption_size: 12.0,
            title_size: 24.0,
        }
    }
}

impl Theme {
    /// The default of [`Theme::glass_tint`], between clear and tinted.
    pub const DEFAULT_GLASS_TINT: f32 = 0.35;

    /// How far the tint setting is past the default, from 0 to 1.
    pub(crate) fn tint_above_default(&self) -> f32 {
        if self.reduce_transparency {
            1.0
        } else {
            ((self.glass_tint - Self::DEFAULT_GLASS_TINT) / (1.0 - Self::DEFAULT_GLASS_TINT)).clamp(0.0, 1.0)
        }
    }

    /// How milky the lifted tab bar selection is, from 0 to 1.
    pub(crate) fn tab_blob_veil(&self) -> f32 {
        if self.reduce_transparency { 1.0 } else { self.glass_tint.clamp(0.0, 1.0) * 0.75 }
    }
    /// Returns the color of text on the window background.
    pub fn text_color(&self) -> Color {
        if self.dark { Color::gray(0.97) } else { Color::gray(0.04) }
    }
    /// Returns the window background color.
    pub fn background_color(&self) -> Color {
        if self.dark { Color::gray(0.07) } else { Color::gray(0.95) }
    }
}

/// What a widget is drawn on. It decides the colors of labels and fills.
///
/// Read it from [`Ui::surface`](crate::Ui::surface) when you draw your own
/// widget, so it looks right on content and on glass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Surface {
    /// Directly on app content.
    Content {
        /// The content has the dark appearance.
        dark: bool,
    },
    /// On a glass element.
    Glass {
        /// How dark the glass is, from 0 (light) to 1 (dark).
        dark: f32,
    },
}

impl Surface {
    /// Returns how dark the surface is, from 0 (light) to 1 (dark).
    pub fn darkness(&self) -> f32 {
        match self {
            Surface::Content { dark } => *dark as u32 as f32,
            Surface::Glass { dark } => *dark,
        }
    }
    /// Returns `true` on glass.
    pub fn is_glass(&self) -> bool {
        matches!(self, Surface::Glass { .. })
    }
    /// Returns the color of primary text.
    pub fn text_color(&self) -> Color {
        Color::gray(0.04).lerp(Color::gray(0.99), self.darkness())
    }
    /// Returns the color of less important text.
    pub fn secondary_color(&self) -> Color {
        Color::black(0.52).lerp(Color::white(0.62), self.darkness())
    }
    /// Returns the color of hints and disabled text.
    pub fn tertiary_color(&self) -> Color {
        Color::black(0.30).lerp(Color::white(0.36), self.darkness())
    }
    /// Returns the fill of tracks, wells and hovered rows.
    pub fn fill_color(&self) -> Color {
        Color::black(0.085).lerp(Color::white(0.13), self.darkness())
    }
    /// Returns a stronger fill, for selections and switches that are off.
    pub fn strong_fill_color(&self) -> Color {
        Color::black(0.15).lerp(Color::white(0.22), self.darkness())
    }
    /// Returns the color of separator lines.
    pub fn separator_color(&self) -> Color {
        Color::black(0.10).lerp(Color::white(0.14), self.darkness())
    }
    /// Returns the color of a thumb at rest.
    pub fn thumb_color(&self) -> Color {
        Color::WHITE.lerp(Color::gray(0.96), self.darkness())
    }
}
