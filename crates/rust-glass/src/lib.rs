//! A GPU rendered UI framework in the style of Apple's Liquid Glass, with an
//! immediate mode API.
//!
//! You describe the whole interface every frame in [`App::update`]. Widgets
//! return a [`Response`] that tells you whether they were clicked or changed.
//! Glass controls bend, blur and tint whatever is drawn behind them,
//! including other glass.
//!
//! ```no_run
//! use rust_glass::prelude::*;
//!
//! struct Hello {
//!     volume: f32,
//! }
//!
//! impl App for Hello {
//!     fn update(&mut self, ctx: &mut Context, _: &FrameInfo) {
//!         let area = ctx.screen.shrink(24.0);
//!         region(ctx, "main", area, |ui| {
//!             ui.heading("Hello");
//!             ui.slider(&mut self.volume, 0.0..=1.0);
//!             if ui.prominent_button("Reset").clicked() {
//!                 self.volume = 0.5;
//!             }
//!         });
//!     }
//! }
//!
//! fn main() -> Result<(), String> {
//!     run(Options::default(), Hello { volume: 0.3 })
//! }
//! ```
//!
//! The [`guide`] explains layout, widgets, theming, materials and testing.

#![warn(missing_docs)]

pub mod accessibility;
pub mod animation;
pub mod app;
pub mod color;
pub mod context;
pub mod geometry;
pub(crate) mod gpu;
pub mod headless;
pub mod icon;
pub mod input;
pub mod material;
pub mod paint;
pub mod text;
pub mod theme;
pub mod ui;
pub mod widgets;

/// How to build an app with rust-glass, with examples for every part of the
/// API.
#[doc = include_str!("../GUIDE.md")]
pub mod guide {}

pub use app::{App, FrameInfo, FrameStats, FrameTimes, Options, Quality, WindowIcon, run};
pub use color::Color;
pub use context::{Context, Id, Order, Response, Sense, Waker};
pub use geometry::{Rect, Vec2, rect, vec2};
pub use icon::Icon;
pub use material::{Corner, Glass, GlassShape, Material};
pub use paint::{ImageId, Painter};
pub use theme::{Surface, Theme};
pub use ui::{Ui, Widget};
/// The windowing library rust-glass runs on, for [`App::window_created`].
pub use winit;

/// Everything an app usually needs: `use rust_glass::prelude::*;`
pub mod prelude {
    pub use crate::animation::{Spring, Tuning};
    pub use crate::text::{TextAlign, TextStyle, Weight};
    pub use crate::ui::Align;
    pub use crate::widgets::button::{Button, ButtonGroup, ButtonKind, ButtonSize, SplitButton};
    pub use crate::widgets::checkbox::{Checkbox, Radio};
    pub use crate::widgets::collapsing::Collapsing;
    pub use crate::widgets::color_picker::ColorPicker;
    pub use crate::widgets::dialog::{alert, modal, sheet};
    pub use crate::widgets::image::Image;
    pub use crate::widgets::list::{SettingsRow, nav_list, section, table};
    pub use crate::widgets::menu::{ComboBox, MenuButton, MenuItem, context_menu};
    pub use crate::widgets::panel::{Panel, region};
    pub use crate::widgets::popup::{popup, tooltip};
    pub use crate::widgets::progress::{Gauge, ProgressBar, Spinner};
    pub use crate::widgets::scroll::ScrollArea;
    pub use crate::widgets::segmented::Segmented;
    pub use crate::widgets::slider::{DragValue, Slider, Stepper};
    pub use crate::widgets::tab_bar::TabBar;
    pub use crate::widgets::text_edit::TextEdit;
    pub use crate::widgets::toggle::Toggle;
    pub use crate::widgets::window::Window;
    pub use crate::{
        App, Color, Context, Corner, FrameInfo, Glass, GlassShape, Icon, Id, ImageId, Material, Options,
        Order, Quality, Rect, Response, Sense, Surface, Theme, Ui, Vec2, Widget, rect, run, vec2,
    };
}
