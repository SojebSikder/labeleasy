//! Modal dialogs: a dimmed backdrop that closes on click, and a card that
//! keeps clicks inside it from reaching the backdrop.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Shell, overlay, renderer};
use iced::widget::{center, container, mouse_area, opaque, stack};
use iced::{
    Border, Color, Element, Event, Length, Rectangle, Renderer, Shadow, Size, Theme, Vector, mouse,
};

/// a raised card of fixed `width`, for modal content.
pub fn card<'a, Message: 'a>(
    body: impl Into<Element<'a, Message>>,
    width: f32,
) -> Element<'a, Message> {
    let styled = container(body)
        .width(Length::Fixed(width))
        .padding(18)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                text_color: Some(palette.background.base.text),
                background: Some(crate::theme::colors().elevated_surface_background.into()),
                border: Border {
                    color: palette.background.strong.color,
                    width: 1.0,
                    radius: 10.0.into(),
                },
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
                    offset: Vector::new(0.0, 6.0),
                    blur_radius: 24.0,
                },
                ..Default::default()
            }
        });

    Element::new(ClickSwallow {
        content: styled.into(),
    })
}

/// shows `content` centered over `base`, dimming it. Clicking the backdrop
/// sends `on_blur`, if given.
pub fn modal<'a, Message: Clone + 'a>(
    base: impl Into<Element<'a, Message>>,
    content: impl Into<Element<'a, Message>>,
    on_blur: Option<Message>,
) -> Element<'a, Message> {
    let backdrop = center(content).style(|_theme| container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.45).into()),
        ..Default::default()
    });
    let mut area = mouse_area(backdrop);
    if let Some(message) = on_blur {
        area = area.on_press(message);
    }
    stack![base.into(), opaque(area)].into()
}

/// captures left clicks within its bounds after its content saw them, so
/// they don't fall through to whatever is underneath (the backdrop).
struct ClickSwallow<'a, Message> {
    content: Element<'a, Message>,
}

impl<'a, Message> iced::advanced::Widget<Message, Theme, Renderer> for ClickSwallow<'a, Message> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        let is_click = matches!(
            event,
            Event::Mouse(
                mouse::Event::ButtonPressed(mouse::Button::Left)
                    | mouse::Event::ButtonReleased(mouse::Button::Left)
            )
        );
        if is_click && cursor.is_over(layout.bounds()) {
            shell.capture_event();
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
