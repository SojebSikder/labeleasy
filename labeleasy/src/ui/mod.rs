//! Views. `view` assembles the window: titlebar + menu, toolbar, canvas,
//! sidebar, status bar, then dialogs and toasts on top.

pub mod dialogs;
pub mod label_dialog;
pub mod menu;
pub mod settings_modal;
pub mod sidebar;
pub mod status_bar;
pub mod toolbar;

use crate::app::{App, Overlay};
use crate::canvas::image_layer::ImageLayer;
use crate::canvas::{AnnotationCanvas, Options};
use crate::message::Message;
use iced::widget::{Canvas, center, column, container, row, stack, text};
use iced::{Element, Length, Size};
use labeleasy_ui::widgets::{
    DividerOrientation, TitlebarActions, menu as menubar, modal, resize_handle, style, titlebar,
    window_border, window_resize_handles,
};

pub fn view(app: &App) -> Element<'_, Message> {
    let groups = menu::groups(app);
    let menu_bar = menubar::render_menu_bar(&app.menu, &groups).map(Message::Menu);
    let title = titlebar(
        menu_bar,
        app.title(),
        app.maximized,
        TitlebarActions {
            drag: Message::TitleBarDrag,
            minimize: Message::Minimize,
            toggle_maximize: Message::ToggleMaximize,
            close: Message::Command(crate::message::Command::Quit),
        },
    );

    let sidebar = container(sidebar::view(app)).width(Length::Fixed(app.settings.sidebar_width));
    let body = row![
        canvas(app),
        resize_handle(DividerOrientation::Vertical, Message::SidebarResizeStart),
        sidebar,
    ]
    .height(Length::Fill);

    let base: Element<'_, Message> = column![
        title,
        toolbar::view(app),
        style::divider(),
        body,
        style::divider(),
        status_bar::view(app)
    ]
    .into();

    let mut layers = vec![base];

    if let Some(dropdown) = menubar::render_menu_overlay(&app.menu, &groups) {
        layers.push(dropdown.map(Message::Menu));
    }

    let dialog: Option<(Element<'_, Message>, Option<Message>)> = match &app.overlay {
        Overlay::None => None,
        Overlay::LabelDialog(d) => Some((d.view(app), None)),
        Overlay::Settings(d) => Some((d.view(app), Some(Message::CloseOverlay))),
        Overlay::Shortcuts => Some((dialogs::shortcuts(), Some(Message::CloseOverlay))),
        Overlay::About => Some((dialogs::about(), Some(Message::CloseOverlay))),
        Overlay::ConfirmUnsaved { .. } => Some((dialogs::confirm_unsaved(), None)),
    };

    let mut root: Element<'_, Message> = stack(layers).into();
    if let Some((content, on_blur)) = dialog {
        root = modal(root, content, on_blur);
    }

    let mut top = vec![root, app.toasts.view(Message::DismissToast, Message::ToastAction)];
    if !app.maximized {
        top.push(window_resize_handles(Message::WindowResizeDrag));
        top.push(window_border());
    }
    stack(top).width(Length::Fill).height(Length::Fill).into()
}

fn canvas(app: &App) -> Element<'_, Message> {
    let (Some(doc), Some(image)) = (&app.doc, &app.image) else {
        let message = if app.loading.is_some() {
            "Loading…"
        } else {
            "Open a folder (Ctrl+U), images (Ctrl+O), or drop files here"
        };
        return center(
            text(message)
                .size(15)
                .color(labeleasy_ui::theme::colors().text_muted),
        )
        .style(|_t| container::Style {
            background: Some(labeleasy_ui::theme::colors().editor_background.into()),
            ..Default::default()
        })
        .into();
    };

    let image_size = Size::new(image.width as f32, image.height as f32);
    let program = AnnotationCanvas {
        image_size,
        shapes: doc.shapes(),
        selected: &app.selected,
        hidden: &app.hidden,
        hidden_labels: &app.hidden_labels,
        labels: &app.labels,
        view: app.view,
        mode: app.mode,
        options: Options {
            fill_opacity: app.settings.fill_opacity,
            show_labels: app.settings.show_labels,
            crosshair: app.settings.crosshair,
            wheel_zooms: app.settings.wheel_zooms,
        },
        doc_id: app.doc_id,
        revision: doc.revision(),
        canvas_rev: app.canvas_rev,
    };
    let background = labeleasy_ui::theme::colors().editor_background;
    let annotations: Element<'_, Message> = Element::from(
        Canvas::new(program)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .map(Message::Canvas);
    stack![
        ImageLayer::new(image.handle.clone(), image_size, app.view, background),
        annotations
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
