//! Shared styles and small building blocks, all driven by the active theme.

use iced::widget::{Space, button, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme};

pub fn with_alpha(color: Color, a: f32) -> Color {
    Color { a, ..color }
}

/// a 1px horizontal rule.
pub fn divider<'a, Message: 'a>() -> Element<'a, Message> {
    container(Space::new())
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(|_theme| container::Style {
            background: Some(crate::theme::colors().border_variant.into()),
            ..Default::default()
        })
        .into()
}

/// a 1px vertical rule.
pub fn vertical_divider<'a, Message: 'a>() -> Element<'a, Message> {
    container(Space::new())
        .width(Length::Fixed(1.0))
        .height(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(crate::theme::colors().border_variant.into()),
            ..Default::default()
        })
        .into()
}

pub fn panel(_theme: &Theme) -> container::Style {
    let c = crate::theme::colors();
    container::Style {
        background: Some(c.panel_background.into()),
        text_color: Some(c.text),
        ..Default::default()
    }
}

pub fn status_bar(_theme: &Theme) -> container::Style {
    let c = crate::theme::colors();
    container::Style {
        background: Some(c.status_bar_background.into()),
        text_color: Some(c.text_muted),
        ..Default::default()
    }
}

pub fn toolbar(_theme: &Theme) -> container::Style {
    let c = crate::theme::colors();
    container::Style {
        background: Some(c.tab_bar_background.into()),
        text_color: Some(c.text),
        ..Default::default()
    }
}

/// uppercase muted caption above a panel section, with optional trailing content.
pub fn section_header<'a, Message: 'a>(
    title: &'a str,
    trailing: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut r = row![
        text(title.to_uppercase())
            .size(11)
            .color(crate::theme::colors().text_muted)
            .width(Length::Fill)
    ]
    .align_y(Alignment::Center)
    .height(Length::Fixed(26.0));
    if let Some(t) = trailing {
        r = r.push(t);
    }
    container(r).padding([0, 10]).into()
}

/// a flat list row; `selected` rows get the theme's selection background.
pub fn list_row(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let c = crate::theme::colors();
        let background = if selected {
            Some(c.element_selected.into())
        } else {
            match status {
                button::Status::Hovered | button::Status::Pressed => Some(c.element_hover.into()),
                _ => None,
            }
        };
        button::Style {
            background,
            text_color: c.text,
            border: Border {
                radius: 3.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

/// a toolbar toggle/action button; `active` renders it as pressed-in.
pub fn tool_button(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let c = crate::theme::colors();
        let accent = c.text_accent;
        let (background, text_color, border_color) = if active {
            (
                Some(with_alpha(accent, 0.18).into()),
                accent,
                with_alpha(accent, 0.5),
            )
        } else {
            match status {
                button::Status::Hovered | button::Status::Pressed => {
                    (Some(c.element_hover.into()), c.text, Color::TRANSPARENT)
                }
                button::Status::Disabled => (None, c.text_placeholder, Color::TRANSPARENT),
                _ => (None, c.text, Color::TRANSPARENT),
            }
        };
        button::Style {
            background,
            text_color,
            border: Border {
                radius: 5.0.into(),
                width: 1.0,
                color: border_color,
            },
            ..Default::default()
        }
    }
}

/// the primary call-to-action button style.
pub fn primary_button(theme: &Theme, status: button::Status) -> button::Style {
    let mut style = button::primary(theme, status);
    style.border.radius = 5.0.into();
    style
}

pub fn secondary_button(theme: &Theme, status: button::Status) -> button::Style {
    let mut style = button::secondary(theme, status);
    style.border.radius = 5.0.into();
    style
}

/// a small filled square of `color`, e.g. a label's color.
pub fn swatch<'a, Message: 'a>(color: Color, size: f32) -> Element<'a, Message> {
    container(Space::new())
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |_theme| container::Style {
            background: Some(color.into()),
            border: Border {
                radius: 2.0.into(),
                width: 1.0,
                color: with_alpha(Color::BLACK, 0.25),
            },
            ..Default::default()
        })
        .into()
}

/// muted helper text.
pub fn hint<'a>(content: impl text::IntoFragment<'a>) -> iced::widget::Text<'a> {
    text(content)
        .size(12)
        .color(crate::theme::colors().text_muted)
}
