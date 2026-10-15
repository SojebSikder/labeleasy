//! Right-hand panels: image flags, labels, the shapes of this image, files.

use crate::app::App;
use crate::canvas::label_color;
use crate::message::Message;
use iced::widget::{
    Space, button, checkbox, column, container, mouse_area, row, scrollable, text, text_input,
};
use iced::{Alignment, Element, Length};
use labeleasy_ui::widgets::{style, virtual_list};
use std::collections::HashMap;

pub const FILE_ROW_HEIGHT: f32 = 24.0;
pub const SHAPE_ROW_HEIGHT: f32 = 26.0;

pub fn view(app: &App) -> Element<'_, Message> {
    let mut panels = column![].width(Length::Fill).height(Length::Fill);

    if let Some(doc) = &app.doc
        && !doc.flags().is_empty()
    {
        panels = panels
            .push(style::section_header("Flags", None))
            .push(flags(app))
            .push(style::divider());
    }

    panels = panels
        .push(style::section_header("Labels", None))
        .push(container(labels(app)).height(Length::FillPortion(2)))
        .push(style::divider())
        .push(style::section_header("Shapes", Some(shape_count(app))))
        .push(container(shapes(app)).height(Length::FillPortion(3)))
        .push(style::divider())
        .push(style::section_header("Files", Some(file_count(app))))
        .push(files(app));

    container(panels)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(style::panel)
        .into()
}

fn flags(app: &App) -> Element<'_, Message> {
    let Some(doc) = &app.doc else {
        return Space::new().into();
    };
    let boxes = row(doc.flags().iter().map(|(name, &value)| {
        let flag = name.clone();
        checkbox(value)
            .label(name.as_str())
            .size(14)
            .text_size(13)
            .on_toggle(move |v| Message::ImageFlagToggled(flag.clone(), v))
            .into()
    }))
    .spacing(14)
    .wrap();
    container(boxes).padding([4, 10]).into()
}

fn labels(app: &App) -> Element<'_, Message> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    if let Some(doc) = &app.doc {
        for shape in doc.shapes() {
            *counts.entry(shape.label.as_str()).or_default() += 1;
        }
    }
    if app.labels.is_empty() {
        return container(style::hint(
            "Labels appear here as you name shapes, or load a labels.txt",
        ))
        .padding([4, 10])
        .into();
    }
    let rows = column(app.labels.iter().map(|label| {
        let visible = !app.hidden_labels.contains(label);
        let count = counts.get(label).copied().unwrap_or(0);
        let toggle = label.to_string();
        let row_content = row![
            checkbox(visible)
                .size(13)
                .on_toggle(move |_| Message::ToggleLabelVisible(toggle.clone())),
            style::swatch(label_color(&app.labels, label), 12.0),
            text(label.to_string()).size(13).width(Length::Fill),
            text(count.to_string())
                .size(12)
                .color(labeleasy_ui::theme::colors().text_muted),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        button(row_content)
            .width(Length::Fill)
            .padding([3, 10])
            .style(style::list_row(false))
            .on_press(Message::LabelClicked(label.to_string()))
            .into()
    }));
    scrollable(rows).height(Length::Fill).into()
}

fn shape_count(app: &App) -> Element<'_, Message> {
    let n = app.doc.as_ref().map_or(0, |d| d.shapes().len());
    style::hint(n.to_string()).into()
}

fn shapes(app: &App) -> Element<'_, Message> {
    let Some(doc) = &app.doc else {
        return Space::new().into();
    };
    let shapes = doc.shapes();
    if shapes.is_empty() {
        return container(style::hint(
            "Pick a tool in the toolbar and draw on the image",
        ))
        .padding([4, 10])
        .into();
    }
    virtual_list(
        &app.shape_list,
        shapes.len(),
        SHAPE_ROW_HEIGHT,
        |i| {
            let shape = &shapes[i];
            let selected = app.selected.contains(&i);
            let mut name = shape.label.clone();
            if let Some(group) = shape.group_id {
                name.push_str(&format!(" ({group})"));
            }
            let content = row![
                checkbox(!app.hidden.contains(&i))
                    .size(13)
                    .on_toggle(move |_| Message::ToggleShapeVisible(i)),
                style::swatch(label_color(&app.labels, &shape.label), 12.0),
                text(name).size(13).width(Length::Fill),
                style::hint(shape.shape_type.label()),
            ]
            .spacing(8)
            .align_y(Alignment::Center);
            let body = container(content)
                .padding([0, 10])
                .height(Length::Fixed(SHAPE_ROW_HEIGHT))
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .style(move |theme| {
                    let s = style::list_row(selected)(theme, button::Status::Active);
                    iced::widget::container::Style {
                        background: s.background,
                        ..Default::default()
                    }
                });
            mouse_area(body)
                .on_press(Message::ShapeClicked(i))
                .on_double_click(Message::ShapeDoubleClicked(i))
                .into()
        },
        Message::ShapeListScrolled,
    )
}

fn file_count(app: &App) -> Element<'_, Message> {
    if app.dataset.is_empty() {
        return Space::new().into();
    }
    style::hint(format!(
        "{} / {} annotated",
        app.dataset.annotated_count(),
        app.dataset.len()
    ))
    .into()
}

fn files(app: &App) -> Element<'_, Message> {
    if app.dataset.is_empty() {
        let open = button(text("Open folder…").size(13))
            .style(style::primary_button)
            .on_press(Message::Command(crate::message::Command::OpenDir));
        return container(column![style::hint("No images open"), open].spacing(8))
            .padding([4, 10])
            .height(Length::FillPortion(4))
            .into();
    }

    let filter = text_input("Filter files…", &app.file_filter)
        .on_input(Message::FileFilterChanged)
        .size(13)
        .padding([5, 8]);

    let colors = labeleasy_ui::theme::colors();
    let list = virtual_list(
        &app.file_list,
        app.filtered.len(),
        FILE_ROW_HEIGHT,
        |row_index| {
            let index = app.filtered[row_index];
            let entry = &app.dataset.entries[index];
            let current = app.current == Some(index);
            let (mark, mark_color) = if entry.annotated {
                ("✓", colors.success)
            } else {
                ("·", colors.text_placeholder)
            };
            button(
                row![
                    text(mark)
                        .size(13)
                        .color(mark_color)
                        .width(Length::Fixed(14.0)),
                    text(entry.file_name())
                        .size(13)
                        .wrapping(text::Wrapping::None),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fixed(FILE_ROW_HEIGHT))
            .padding([2, 10])
            .style(style::list_row(current))
            .on_press(Message::SelectImage(index))
            .into()
        },
        Message::FileListScrolled,
    );

    column![container(filter).padding([2, 10]), list]
        .spacing(4)
        .height(Length::FillPortion(4))
        .into()
}
