//! Image widget.

use crate::color::Color;
use crate::context::{Response, Sense};
use crate::geometry::{Vec2, rect};
use crate::material::Corner;
use crate::paint::ImageId;
use crate::ui::{Ui, Widget};

/// An image added with [`Context::add_image`].
pub struct Image {
    id: ImageId,
    size: Vec2,
    corner: Corner,
}

impl Image {
    /// Creates an image widget of `size` points.
    pub fn new(id: ImageId, size: Vec2) -> Self {
        Self { id, size, corner: Corner::Radius(0.0) }
    }
    /// Rounds the corners.
    pub fn corner(mut self, corner: impl Into<Corner>) -> Self {
        self.corner = corner.into();
        self
    }
}

impl Widget for Image {
    fn ui(self, ui: &mut Ui) -> Response {
        let r = ui.allocate_response(self.size, Sense::Hover);
        ui.painter().image(r.rect, self.corner, self.id, rect(0.0, 0.0, 1.0, 1.0), Color::WHITE);
        r
    }
}

impl Ui<'_> {
    pub fn image(&mut self, id: ImageId, size: Vec2, corner: impl Into<Corner>) -> Response {
        self.add(Image::new(id, size).corner(corner))
    }
}
