use super::App;
use crate::message::Message;
use crate::settings::Settings;
use iced::Task;
use labeleasy_ui::theme::{self, ActiveTheme, Appearance, ThemeRegistry};

pub fn user_themes_dir() -> Option<std::path::PathBuf> {
    theme::loader::user_themes_dir(crate::APP_NAME)
}

pub fn load_registry() -> ThemeRegistry {
    theme::loader::load_registry(&[], user_themes_dir().as_deref())
}

pub fn resolve(registry: &ThemeRegistry, settings: &Settings, system: Appearance) -> ActiveTheme {
    let (name, fallback) = settings.theme.resolve(system);
    let entry = registry.get_or_default(name, fallback);
    entry.build(settings.theme_overrides.get(&entry.name))
}

/// re-resolves the active theme after a settings or system change.
pub fn apply(app: &mut App) {
    app.theme = resolve(&app.themes, &app.settings, app.system_appearance);
    theme::publish(&app.theme);
    app.touch_canvas();
}

pub fn reload(app: &mut App) {
    app.themes = load_registry();
    for error in std::mem::take(&mut app.themes.errors) {
        app.toasts.error(error);
    }
    apply(app);
}

pub fn system_changed(app: &mut App, mode: iced::theme::Mode) -> Task<Message> {
    app.system_appearance = match mode {
        iced::theme::Mode::Light => Appearance::Light,
        _ => Appearance::Dark,
    };
    apply(app);
    Task::none()
}
