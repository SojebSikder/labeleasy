//! Draws the image being annotated, positioned by the same [`ViewMode`] as
//! the annotation canvas stacked on top of it.

use super::ViewMode;
use iced::advanced::image::{FilterMethod, Handle, Image, Renderer as _};
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::{self, Renderer as _};
use iced::advanced::widget::{Tree, Widget};
use iced::{Color, Element, Length, Rectangle, Renderer, Size, Theme, mouse};

pub struct ImageLayer {
    handle: Handle,
    image_size: Size,
    view: ViewMode,
    background: Color,
}

impl ImageLayer {
    pub fn new(handle: Handle, image_size: Size, view: ViewMode, background: Color) -> Self {
        Self {
            handle,
            image_size,
            view,
            background,
        }
    }
}

impl<Message> Widget<Message, Theme, Renderer> for ImageLayer {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.max())
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                ..Default::default()
            },
            self.background,
        );

        let t = self.view.resolve(bounds.size(), self.image_size);
        let target = Rectangle {
            x: bounds.x + t.offset.x,
            y: bounds.y + t.offset.y,
            width: self.image_size.width * t.scale,
            height: self.image_size.height * t.scale,
        };
        // nearest-neighbour when zoomed in so individual pixels can be annotated precisely
        let filter = if t.scale >= 2.0 {
            FilterMethod::Nearest
        } else {
            FilterMethod::Linear
        };
        renderer.draw_image(
            Image::new(self.handle.clone()).filter_method(filter),
            target,
            bounds,
        );
    }
}

impl<'a, Message: 'a> From<ImageLayer> for Element<'a, Message> {
    fn from(layer: ImageLayer) -> Self {
        Element::new(layer)
    }
}
