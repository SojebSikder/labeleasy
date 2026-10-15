use crate::app::App;
use crate::canvas::Mode;
use crate::message::{Command, Message};
use iced::widget::{Space, button, container, row, text};
use iced::{Alignment, Element, Length};
use labeleasy_ui::widgets::style;

pub fn view(app: &App) -> Element<'_, Message> {
    let colors = labeleasy_ui::theme::colors();
    let mode = match app.mode {
        Mode::Edit => "Edit".to_string(),
        Mode::Create(shape) => format!("Draw {}", shape.label().to_lowercase()),
    };

    let mut items = row![text(mode).size(12).color(colors.text_accent)]
        .spacing(16)
        .align_y(Alignment::Center);

    if let Some(path) = &app.loading {
        items = items.push(text(format!("Loading {}…", path.display())).size(12));
    } else if let (Some(doc), Some(image)) = (&app.doc, &app.image) {
        items = items
            .push(text(doc.image_path.display().to_string()).size(12))
            .push(text(format!("{} × {}", image.width, image.height)).size(12))
            .push(text(format!("{} shapes", doc.shapes().len())).size(12));
        if !app.selected.is_empty() {
            items = items.push(text(format!("{} selected", app.selected.len())).size(12));
        }
        if doc.is_dirty() {
            items = items.push(text("● unsaved").size(12).color(colors.modified));
        }
    } else {
        items = items.push(text("Open a folder (Ctrl+U) or drop images here").size(12));
    }

    items = items.push(Space::new().width(Length::Fill));
    if let (Some(i), false) = (app.current, app.dataset.is_empty()) {
        items = items.push(text(format!("{} / {}", i + 1, app.dataset.len())).size(12));
    }
    if let Some(dir) = &app.dataset.output_dir {
        items = items.push(text(format!("→ {}", dir.display())).size(12));
    }
    if app.settings.auto_save {
        items = items.push(text("auto-save").size(12).color(colors.success));
    }
    if let (Some(info), false) = (&app.update.available, app.update.downloading()) {
        items = items.push(
            button(
                text(format!("Update to v{}", info.version))
                    .size(12)
                    .color(colors.text_accent),
            )
            .style(button::text)
            .padding(0)
            .on_press(Message::Command(Command::InstallUpdate)),
        );
    }

    container(items)
        .padding([0, 10])
        .height(Length::Fixed(24.0))
        .width(Length::Fill)
        .align_y(Alignment::Center)
        .style(style::status_bar)
        .into()
}
