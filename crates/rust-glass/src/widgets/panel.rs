//! Glass panels and plain regions placed at a fixed rectangle.

use crate::context::{Context, Id, Order, Sense};
use crate::geometry::Rect;
use crate::material::{Corner, Glass, Material};
use crate::ui::Ui;

/// Glass panel at a fixed place above the content.
pub struct Panel {
    id: Id,
    rect: Rect,
    corner: Corner,
    material: Option<Material>,
    pad: f32,
    adaptive: bool,
    order: Order,
}

impl Panel {
    /// Creates a panel at `rect`.
    pub fn new(id: impl std::hash::Hash, rect: Rect) -> Self {
        Self {
            id: Id::new(("panel", Id::new(id).0)),
            rect,
            corner: Corner::Radius(28.0),
            material: None,
            pad: 14.0,
            adaptive: false,
            order: Order::Bar,
        }
    }
    /// Sets the corners. The default is a radius of 28.
    pub fn corner(mut self, corner: impl Into<Corner>) -> Self {
        self.corner = corner.into();
        self
    }
    /// Sets the material. The default is the theme's thick glass.
    pub fn material(mut self, material: Material) -> Self {
        self.material = Some(material);
        self
    }
    /// Sets the space between the edge and the content. The default is 14.
    pub fn padding(mut self, padding: f32) -> Self {
        self.pad = padding;
        self
    }
    /// Lets the panel switch between light and dark with what is behind it.
    /// Use it for small panels.
    pub fn adaptive(mut self) -> Self {
        self.adaptive = true;
        self
    }
    /// Sets the layer order. The default is [`Order::Bar`].
    pub fn order(mut self, order: Order) -> Self {
        self.order = order;
        self
    }

    /// Shows the panel with the content drawn by `f`.
    pub fn show<R>(mut self, ctx: &mut Context, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.rect = self.rect.snap(ctx.pixels_per_point);
        let layer = ctx.layer(self.id, self.order);
        ctx.interact(self.id.with("block"), self.rect, Rect::EVERYTHING, layer, Sense::Hover);
        let m = self.material.unwrap_or(ctx.theme.thick);
        let mut g = Glass::new(self.rect, self.corner, m);
        let a = ctx.prepare_glass(self.id, &mut g, self.adaptive);
        ctx.painter(layer, Rect::EVERYTHING).glass(&g);
        let mut ui = Ui::new(ctx, layer, self.id, self.rect.shrink(self.pad));
        ui.surface = a.surface();
        f(&mut ui)
    }
}

/// Plain region of the window, without a surface of its own.
pub fn region<R>(ctx: &mut Context, id: impl std::hash::Hash, rect: Rect, f: impl FnOnce(&mut Ui) -> R) -> R {
    let id = Id::new(("region", Id::new(id).0));
    let layer = ctx.layer(id, Order::Content);
    let mut ui = Ui::new(ctx, layer, id, rect);
    f(&mut ui)
}
