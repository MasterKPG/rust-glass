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
    flips: bool,
    order: Order,
}

impl Panel {
    pub fn new(id: impl std::hash::Hash, rect: Rect) -> Self {
        Self {
            id: Id::new(("panel", Id::new(id).0)),
            rect,
            corner: Corner::Radius(28.0),
            material: None,
            pad: 14.0,
            flips: false,
            order: Order::Bar,
        }
    }
    pub fn corner(mut self, c: impl Into<Corner>) -> Self {
        self.corner = c.into();
        self
    }
    pub fn material(mut self, m: Material) -> Self {
        self.material = Some(m);
        self
    }
    pub fn padding(mut self, p: f32) -> Self {
        self.pad = p;
        self
    }
    /// Small panels may switch between light and dark.
    pub fn flips(mut self) -> Self {
        self.flips = true;
        self
    }
    pub fn order(mut self, o: Order) -> Self {
        self.order = o;
        self
    }

    pub fn show<R>(mut self, ctx: &mut Context, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.rect = self.rect.snap(ctx.pixels_per_point);
        let layer = ctx.layer(self.id, self.order);
        ctx.interact(self.id.with("block"), self.rect, Rect::EVERYTHING, layer, Sense::Hover);
        let m = self.material.unwrap_or(ctx.theme.thick);
        let mut g = Glass::new(self.rect, self.corner, m);
        let a = ctx.prepare_glass(self.id, &mut g, self.flips);
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
