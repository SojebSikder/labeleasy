//! Small modal dialogs: unsaved changes, keyboard shortcuts, about.

use crate::message::Message;
use crate::shortcuts::BINDINGS;
use iced::widget::{Space, button, column, row, scrollable, text};
use iced::{Alignment, Element, Length};
use labeleasy_ui::widgets::{card, style};

pub fn confirm_unsaved<'a>() -> Element<'a, Message> {
    let body = column![
        text("Save changes?").size(16),
        text("This image has unsaved annotations.").size(13),
        row![
            Space::new().width(Length::Fill),
            button(text("Cancel").size(13))
                .style(style::secondary_button)
                .on_press(Message::CloseOverlay),
            button(text("Discard").size(13))
                .style(button::danger)
                .on_press(Message::ConfirmDiscard),
            button(text("Save").size(13))
                .style(style::primary_button)
                .on_press(Message::ConfirmSave),
        ]
        .spacing(8),
    ]
    .spacing(12);
    card(body, 380.0)
}

pub fn shortcuts<'a>() -> Element<'a, Message> {
    let mut seen = std::collections::HashSet::new();
    let rows = column(
        BINDINGS
            .iter()
            .filter(|b| seen.insert((b.help, b.label())))
            .map(|b| {
                row![
                    text(b.help).size(13).width(Length::Fill),
                    text(b.label()).size(12).font(iced::Font::MONOSPACE),
                ]
                .align_y(Alignment::Center)
                .into()
            }),
    )
    .spacing(5);

    let mouse = column![
        style::section_header("Mouse", None),
        text("Click on first point / double-click / Enter — close polygon").size(13),
        text("Press-drag-release — draw rectangle, circle or line in one gesture").size(13),
        text("Backspace while drawing — remove last point").size(13),
        text("Drag vertex — reshape · click edge — insert vertex · Alt+click vertex — remove")
            .size(13),
        text("Ctrl+click — add to selection · double-click shape — edit label").size(13),
        text("Wheel — zoom · right/middle drag or Space+drag — pan").size(13),
    ]
    .spacing(5);

    let body = column![
        text("Keyboard shortcuts").size(16),
        scrollable(column![rows, mouse].spacing(12).padding([0, 12])).height(Length::Fixed(460.0)),
        row![
            Space::new().width(Length::Fill),
            button(text("Close").size(13))
                .style(style::primary_button)
                .on_press(Message::CloseOverlay)
        ],
    ]
    .spacing(12);
    card(body, 520.0)
}

pub fn about<'a>() -> Element<'a, Message> {
    let body = column![
        text(crate::APP_NAME).size(20),
        text(format!("Version {}", crate::APP_VERSION)).size(13),
        style::hint("Image annotation"),
        style::hint(format!("Commit {}", crate::COMMIT_SHA)),
        row![
            Space::new().width(Length::Fill),
            button(text("Close").size(13))
                .style(style::primary_button)
                .on_press(Message::CloseOverlay)
        ],
    ]
    .spacing(10);
    card(body, 380.0)
}
