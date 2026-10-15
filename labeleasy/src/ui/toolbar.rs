//! The tool strip above the canvas.

use crate::app::App;
use crate::canvas::Mode;
use crate::message::{Command, Message};
use crate::shortcuts::hint;
use iced::widget::{Space, button, container, row, text, tooltip};
use iced::{Alignment, Element, Length};
use labeleasy_core::ShapeType;
use labeleasy_ui::widgets::style;

fn glyph(shape: ShapeType) -> &'static str {
    match shape {
        ShapeType::Polygon => "⬠",
        ShapeType::Rectangle => "▭",
        ShapeType::Circle => "◯",
        ShapeType::Line => "╱",
        ShapeType::Point => "•",
        ShapeType::LineStrip => "〰",
        ShapeType::Points => "⁘",
        ShapeType::Mask => "▦",
    }
}

fn tip<'a>(
    content: Element<'a, Message>,
    label: &str,
    command: Option<Command>,
) -> Element<'a, Message> {
    let text_label = match command.and_then(hint) {
        Some(h) => format!("{label}  ({h})"),
        None => label.to_string(),
    };
    tooltip(
        content,
        container(text(text_label).size(12))
            .padding([4, 8])
            .style(|_t| {
                let c = labeleasy_ui::theme::colors();
                iced::widget::container::Style {
                    background: Some(c.elevated_surface_background.into()),
                    text_color: Some(c.text),
                    border: iced::Border {
                        color: c.border,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                }
            }),
        tooltip::Position::Bottom,
    )
    .into()
}

fn tool<'a>(
    label: String,
    active: bool,
    command: Command,
    enabled: bool,
    help: &str,
) -> Element<'a, Message> {
    let b = button(text(label).size(13))
        .padding([5, 10])
        .style(style::tool_button(active))
        .on_press_maybe(enabled.then_some(Message::Command(command)));
    tip(b.into(), help, Some(command))
}

pub fn view(app: &App) -> Element<'_, Message> {
    let has_doc = app.doc.is_some();
    let has_dataset = !app.dataset.is_empty();
    let has_selection = !app.selected.is_empty();
    let doc = app.doc.as_ref();

    let mut bar = row![
        tool(
            "Open".into(),
            false,
            Command::OpenFiles,
            true,
            "Open images or label files"
        ),
        tool(
            "Folder".into(),
            false,
            Command::OpenDir,
            true,
            "Open folder"
        ),
        tool(
            "‹ Prev".into(),
            false,
            Command::PrevImage,
            has_dataset,
            "Previous image"
        ),
        tool(
            "Next ›".into(),
            false,
            Command::NextImage,
            has_dataset,
            "Next image"
        ),
        tool("Save".into(), false, Command::Save, has_doc, "Save"),
        style::vertical_divider(),
        tool(
            "↖ Edit".into(),
            app.mode == Mode::Edit,
            Command::SetMode(Mode::Edit),
            has_doc,
            "Select, move and reshape"
        ),
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    for shape in ShapeType::DRAWABLE {
        let mode = Mode::Create(shape);
        bar = bar.push(tool(
            format!("{} {}", glyph(shape), shape.label()),
            app.mode == mode,
            Command::SetMode(mode),
            has_doc,
            &format!("Draw {}", shape.label().to_lowercase()),
        ));
    }

    bar = bar.push(style::vertical_divider());
    bar = bar.push(tool(
        "↶".into(),
        false,
        Command::Undo,
        doc.is_some_and(|d| d.can_undo()),
        "Undo",
    ));
    bar = bar.push(tool(
        "↷".into(),
        false,
        Command::Redo,
        doc.is_some_and(|d| d.can_redo()),
        "Redo",
    ));
    bar = bar.push(tool(
        "⧉".into(),
        false,
        Command::DuplicateSelected,
        has_selection,
        "Duplicate",
    ));
    bar = bar.push(tool(
        "🗑".into(),
        false,
        Command::DeleteSelected,
        has_selection,
        "Delete",
    ));
    bar = bar.push(Space::new().width(Length::Fill));

    let zoom = if has_doc {
        let scale = app.view.scale(app.canvas_size, crate::app::image_size(app));
        format!("{:.0}%", scale * 100.0)
    } else {
        "—".into()
    };
    bar = bar
        .push(tool(
            "−".into(),
            false,
            Command::ZoomOut,
            has_doc,
            "Zoom out",
        ))
        .push(
            container(text(zoom).size(12))
                .width(Length::Fixed(48.0))
                .align_x(Alignment::Center),
        )
        .push(tool("+".into(), false, Command::ZoomIn, has_doc, "Zoom in"))
        .push(tool(
            "Fit".into(),
            false,
            Command::FitWindow,
            has_doc,
            "Fit window",
        ))
        .push(tool(
            "1:1".into(),
            false,
            Command::ZoomOriginal,
            has_doc,
            "Original size",
        ));

    container(bar.height(Length::Fixed(30.0)))
        .padding([4, 8])
        .width(Length::Fill)
        .style(style::toolbar)
        .into()
}
