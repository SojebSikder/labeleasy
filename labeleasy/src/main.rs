#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod canvas;
mod image_cache;
mod message;
mod settings;
mod shortcuts;
mod ui;
mod updater;

use app::App;
use iced::{Event, Subscription, event, mouse, window};
use labeleasy_ui::widgets::DropdownMessage;
use message::Message;
use std::time::Duration;

pub const APP_NAME: &str = "Labeleasy";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const COMMIT_SHA: &str = env!("LABELEASY_COMMIT_SHA");
pub const APP_ICON: &[u8] = include_bytes!("../../assets/images/logo-transparent.png");

pub fn main() -> iced::Result {
    // a daemon (rather than `application`) so `app::init` opens the main
    // window itself and knows its id from the start.
    iced::daemon(app::init, app::update, view)
        .title(title)
        .theme(theme)
        .subscription(subscription)
        .run()
}

fn view(app: &App, _window: window::Id) -> iced::Element<'_, Message> {
    ui::view(app)
}

fn title(app: &App, _window: window::Id) -> String {
    app.title()
}

fn theme(app: &App, _window: window::Id) -> iced::Theme {
    app.theme.iced.clone()
}

fn subscription(app: &App) -> Subscription<Message> {
    let mut subs = vec![
        window::events().map(|(_, event)| Message::WindowEvent(event)),
        iced::system::theme_changes().map(Message::SystemThemeChanged),
    ];

    if app.overlay.is_open() {
        subs.push(shortcuts::dialog_subscription());
    } else {
        subs.push(shortcuts::subscription());
    }

    if !app.toasts.is_empty() {
        subs.push(iced::time::every(Duration::from_millis(500)).map(|_| Message::Tick));
    }

    // clicking anywhere a widget didn't handle closes an open menu
    if app.menu.open_index.is_some() {
        subs.push(event::listen_with(|event, status, _| {
            match (event, status) {
                (
                    Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                    event::Status::Ignored,
                ) => Some(Message::Menu(DropdownMessage::Close)),
                _ => None,
            }
        }));
    }

    if app.sidebar_drag.is_some() {
        subs.push(event::listen_with(|event, _, _| match event {
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                Some(Message::SidebarResizeMove(position.x))
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                Some(Message::SidebarResizeEnd)
            }
            _ => None,
        }));
    }

    Subscription::batch(subs)
}
