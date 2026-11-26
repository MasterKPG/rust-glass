//! GPU rendered glass UI with an immediate mode API.
//!
//! ```no_run
//! use rust_glass::prelude::*;
//!
//! struct Hello { volume: f32 }
//!
//! impl App for Hello {
//!     fn update(&mut self, ctx: &mut Context, _: &FrameInfo) {
//!         let screen = ctx.screen.shrink(24.0);
//!         region(ctx, "main", screen, |ui| {
//!             ui.heading("Hello");
//!             ui.slider(&mut self.volume, 0.0..=1.0);
//!             if ui.prominent_button("Reset").clicked() {
//!                 self.volume = 0.5;
//!             }
//!         });
//!     }
//! }
//!
//! run(Options::default(), Hello { volume: 0.3 }).unwrap();
//! ```
//!
//! The [`guide`] covers layout, widgets, theming, materials and testing.

pub mod access;
pub mod anim;
pub mod app;
pub mod color;
pub mod context;
pub mod geom;
pub mod gpu;
pub mod headless;
pub mod icons;
pub mod input;
pub mod material;
pub mod paint;
pub mod text;
pub mod theme;
pub mod ui;
pub mod widgets;

#[doc = include_str!("../GUIDE.md")]
pub mod guide {}

pub use app::{App, FrameInfo, Options, run};
pub use color::Color;
pub use context::{Context, Id, Order, Response, Sense};
pub use geom::{Rect, Vec2, rect, vec2};
pub use icons::Icon;
pub use material::{Corner, Glass, GlassShape, Material};
pub use paint::{ImageId, Painter};
pub use theme::{Surface, Theme};
pub use ui::{Ui, Widget};

/// Everything an app usually needs.
pub mod prelude {
    pub use crate::anim::{Spring, Tuning};
    pub use crate::text::{TextAlign, TextStyle, Weight};
    pub use crate::ui::Align;
    pub use crate::widgets::bars::{Panel, nav_list, region, section, table};
    pub use crate::widgets::button::{Button, ButtonGroup, ButtonKind, ButtonSize, SplitButton};
    pub use crate::widgets::controls::{Checkbox, Collapsing, Gauge, ProgressBar, Radio, Row, Spinner};
    pub use crate::widgets::overlay::{
        ComboBox, MenuButton, MenuItem, Window, alert, context_menu, modal, popup, sheet, tooltip,
    };
    pub use crate::widgets::scroll::ScrollArea;
    pub use crate::widgets::segmented::{Segmented, TabBar, Toggle};
    pub use crate::widgets::slider::{DragValue, Slider, Stepper};
    pub use crate::widgets::text_edit::TextEdit;
    pub use crate::{
        App, Color, Context, Corner, FrameInfo, Glass, GlassShape, Icon, Id, ImageId, Material, Options,
        Order, Rect, Response, Sense, Surface, Theme, Ui, Vec2, Widget, rect, run, vec2,
    };
}
