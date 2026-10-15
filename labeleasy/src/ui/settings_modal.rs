//! Settings dialog. Edits a draft copy. nothing changes until "Save".

use crate::app::{App, Overlay};
use crate::message::Message;
use crate::settings::Settings;
use iced::widget::{
    button, checkbox, column, pick_list, row, scrollable, slider, text, text_editor, text_input,
};
use iced::{Alignment, Element, Length, Task};
use labeleasy_core::LabelSet;
use labeleasy_ui::theme::{Appearance, ThemeMode, ThemeSelection};
use labeleasy_ui::widgets::style;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggle {
    AutoSave,
    SaveImageData,
    KeepPrevious,
    ValidateLabels,
    RecursiveScan,
    CacheImages,
    ShowLabels,
    Crosshair,
    WheelZooms,
}

impl Toggle {
    const ALL: [(Toggle, &'static str); 9] = [
        (Toggle::AutoSave, "Save automatically when switching images"),
        (Toggle::SaveImageData, "Embed image data in label files"),
        (
            Toggle::KeepPrevious,
            "Copy previous image's shapes onto new images",
        ),
        (Toggle::ValidateLabels, "Only allow labels from the list"),
        (
            Toggle::RecursiveScan,
            "Include subfolders when opening a folder",
        ),
        (
            Toggle::CacheImages,
            "Preload neighboring images (faster, uses more memory)",
        ),
        (Toggle::ShowLabels, "Show label tags on the canvas"),
        (Toggle::Crosshair, "Crosshair while drawing"),
        (Toggle::WheelZooms, "Mouse wheel zooms (Ctrl+wheel scrolls)"),
    ];

    fn field(self, s: &mut Settings) -> &mut bool {
        match self {
            Toggle::AutoSave => &mut s.auto_save,
            Toggle::SaveImageData => &mut s.save_image_data,
            Toggle::KeepPrevious => &mut s.keep_previous,
            Toggle::ValidateLabels => &mut s.validate_labels,
            Toggle::RecursiveScan => &mut s.recursive_scan,
            Toggle::CacheImages => &mut s.cache_images,
            Toggle::ShowLabels => &mut s.show_labels,
            Toggle::Crosshair => &mut s.crosshair,
            Toggle::WheelZooms => &mut s.wheel_zooms,
        }
    }
}

pub struct SettingsDraft {
    settings: Settings,
    labels: text_editor::Content,
    image_flags: String,
    shape_flags: String,
}

#[derive(Debug, Clone)]
pub enum SettingsMessage {
    ThemeMode(ThemeMode),
    LightTheme(String),
    DarkTheme(String),
    Toggle(Toggle, bool),
    FillOpacity(f32),
    Labels(text_editor::Action),
    ImageFlags(String),
    ShapeFlags(String),
    ImportTheme,
    ThemeFilePicked(Option<PathBuf>),
    ReloadThemes,
    Save,
}

fn split_list(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in s
        .split([',', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if !out.iter().any(|o| o == item) {
            out.push(item.to_string());
        }
    }
    out
}

fn split_lines(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in s.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if !out.iter().any(|o| o == line) {
            out.push(line.to_string());
        }
    }
    out
}

impl SettingsDraft {
    pub fn new(settings: &Settings) -> Self {
        Self {
            settings: settings.clone(),
            labels: text_editor::Content::with_text(&settings.labels.join("\n")),
            image_flags: settings.image_flags.join(", "),
            shape_flags: settings.shape_flags.join(", "),
        }
    }

    fn theme_slots(&self) -> (ThemeMode, String, String) {
        match &self.settings.theme {
            ThemeSelection::Dynamic { mode, light, dark } => (*mode, light.clone(), dark.clone()),
            ThemeSelection::Static(name) => (ThemeMode::Dark, name.clone(), name.clone()),
        }
    }

    pub fn view<'a>(&'a self, app: &'a App) -> Element<'a, Message> {
        let msg = Message::Settings;
        let (mode, light, dark) = self.theme_slots();
        let light_names = app.themes.names(Some(Appearance::Light));
        let dark_names = app.themes.names(Some(Appearance::Dark));

        let theme = column![
            row![
                text("Mode").size(13).width(Length::Fixed(90.0)),
                pick_list(&ThemeMode::ALL[..], Some(mode), move |m| msg(
                    SettingsMessage::ThemeMode(m)
                ))
                .text_size(13),
            ]
            .align_y(Alignment::Center),
            row![
                text("Light theme").size(13).width(Length::Fixed(90.0)),
                pick_list(light_names, Some(light), move |n| msg(
                    SettingsMessage::LightTheme(n)
                ))
                .text_size(13),
            ]
            .align_y(Alignment::Center),
            row![
                text("Dark theme").size(13).width(Length::Fixed(90.0)),
                pick_list(dark_names, Some(dark), move |n| msg(
                    SettingsMessage::DarkTheme(n)
                ))
                .text_size(13),
            ]
            .align_y(Alignment::Center),
            row![
                button(text("Import theme…").size(12))
                    .style(style::secondary_button)
                    .on_press(msg(SettingsMessage::ImportTheme)),
                button(text("Reload themes").size(12))
                    .style(style::secondary_button)
                    .on_press(msg(SettingsMessage::ReloadThemes)),
            ]
            .spacing(8),
        ]
        .spacing(8);

        let mut draft = self.settings.clone();
        let toggles = column(Toggle::ALL.iter().map(|&(toggle, label)| {
            checkbox(*toggle.field(&mut draft))
                .label(label)
                .size(14)
                .text_size(13)
                .on_toggle(move |v| msg(SettingsMessage::Toggle(toggle, v)))
                .into()
        }))
        .spacing(8);

        let opacity = row![
            text("Fill opacity").size(13).width(Length::Fixed(90.0)),
            slider(0.0..=0.8, self.settings.fill_opacity, move |v| msg(
                SettingsMessage::FillOpacity(v)
            ))
            .step(0.05_f32),
            text(format!("{:.0}%", self.settings.fill_opacity * 100.0))
                .size(12)
                .width(Length::Fixed(40.0)),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let labels = column![
            text("Labels (one per line)").size(13),
            text_editor(&self.labels)
                .on_action(move |a| msg(SettingsMessage::Labels(a)))
                .height(Length::Fixed(120.0))
                .size(13),
            text("Image flags (comma separated)").size(13),
            text_input("e.g. occluded, blurry", &self.image_flags)
                .on_input(move |v| msg(SettingsMessage::ImageFlags(v)))
                .size(13)
                .padding(6),
            text("Shape flags (comma separated)").size(13),
            text_input("e.g. truncated, difficult", &self.shape_flags)
                .on_input(move |v| msg(SettingsMessage::ShapeFlags(v)))
                .size(13)
                .padding(6),
        ]
        .spacing(6);

        let body = column![
            text("Settings").size(18),
            scrollable(
                column![
                    style::section_header("Appearance", None),
                    theme,
                    opacity,
                    style::section_header("Behavior", None),
                    toggles,
                    style::section_header("Labels & flags", None),
                    labels,
                ]
                .spacing(10)
                .padding([0, 12])
            )
            .height(Length::Fixed(480.0)),
            row![
                iced::widget::Space::new().width(Length::Fill),
                button(text("Cancel").size(13))
                    .style(style::secondary_button)
                    .on_press(Message::CloseOverlay),
                button(text("Save").size(13))
                    .style(style::primary_button)
                    .on_press(msg(SettingsMessage::Save)),
            ]
            .spacing(8),
        ]
        .spacing(12);

        labeleasy_ui::widgets::card(body, 520.0)
    }
}

pub fn update(app: &mut App, message: SettingsMessage) -> Task<Message> {
    let Overlay::Settings(draft) = &mut app.overlay else {
        return Task::none();
    };
    match message {
        SettingsMessage::ThemeMode(mode) => draft.settings.theme.set_mode(mode, &app.themes),
        SettingsMessage::LightTheme(name) => {
            draft.settings.theme.select_for(Appearance::Light, &name)
        }
        SettingsMessage::DarkTheme(name) => {
            draft.settings.theme.select_for(Appearance::Dark, &name)
        }
        SettingsMessage::Toggle(toggle, value) => *toggle.field(&mut draft.settings) = value,
        SettingsMessage::FillOpacity(v) => draft.settings.fill_opacity = v,
        SettingsMessage::Labels(action) => draft.labels.perform(action),
        SettingsMessage::ImageFlags(v) => draft.image_flags = v,
        SettingsMessage::ShapeFlags(v) => draft.shape_flags = v,
        SettingsMessage::ImportTheme => {
            let picker = rfd::AsyncFileDialog::new()
                .set_title("Import Zed theme")
                .add_filter("Theme", &["json"])
                .pick_file();
            return Task::perform(
                async move { picker.await.map(|f| f.path().to_path_buf()) },
                |p| Message::Settings(SettingsMessage::ThemeFilePicked(p)),
            );
        }
        SettingsMessage::ThemeFilePicked(Some(path)) => {
            let Some(dir) = crate::app::user_themes_dir() else {
                return Task::none();
            };
            match labeleasy_ui::theme::loader::import_file(&path, &dir) {
                Ok(names) => {
                    app.toasts.success(format!("Imported {}", names.join(", ")));
                    crate::app::reload_themes(app);
                }
                Err(e) => {
                    app.toasts.error(e);
                }
            }
        }
        SettingsMessage::ThemeFilePicked(None) => {}
        SettingsMessage::ReloadThemes => crate::app::reload_themes(app),
        SettingsMessage::Save => {
            let Overlay::Settings(draft) = std::mem::replace(&mut app.overlay, Overlay::None)
            else {
                return Task::none();
            };
            let mut settings = draft.settings;
            // labels are one per line and may themselves contain commas
            settings.labels = split_lines(&draft.labels.text());
            settings.image_flags = split_list(&draft.image_flags);
            settings.shape_flags = split_list(&draft.shape_flags);
            // labels seen in files stay known; predefined ones come first
            let mut labels = LabelSet::from_iter(&settings.labels);
            for label in app.labels.iter() {
                labels.insert(label);
            }
            app.labels = labels;
            if let Some(doc) = &mut app.doc {
                doc.ensure_flags(settings.image_flags.iter().map(String::as_str));
            }
            // keep things changed outside the dialog meanwhile
            settings.recent_dirs = app.settings.recent_dirs.clone();
            settings.last_dir = app.settings.last_dir.clone();
            settings.sidebar_width = app.settings.sidebar_width;
            app.settings = settings;
            app.apply_cache_setting();
            app.persist_settings();
            crate::app::apply_theme(app);
        }
    }
    Task::none()
}
