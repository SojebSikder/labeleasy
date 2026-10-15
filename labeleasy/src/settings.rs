//! User settings, persisted as `<data dir>/Labeleasy/settings.json`.

use labeleasy_ui::theme::{ThemeOverrides, ThemeSelection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const MAX_RECENT: usize = 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// `"Name"` or `{ "mode": "system", "light": "..", "dark": ".." }`.
    pub theme: ThemeSelection,
    #[serde(skip_serializing_if = "ThemeOverrides::is_empty")]
    pub theme_overrides: ThemeOverrides,

    /// save automatically when leaving an image.
    pub auto_save: bool,
    /// embed the image (base64) in label files, like labelme's default.
    pub save_image_data: bool,
    /// copy the previous image's shapes onto unannotated images.
    pub keep_previous: bool,
    /// only allow labels from `labels`.
    pub validate_labels: bool,
    /// include subfolders when opening a folder.
    pub recursive_scan: bool,

    pub fill_opacity: f32,
    pub show_labels: bool,
    pub crosshair: bool,
    pub wheel_zooms: bool,

    /// predefined labels (labelme's `--labels`).
    pub labels: Vec<String>,
    /// image-level flags (labelme's `--flags`).
    pub image_flags: Vec<String>,
    /// per-shape flags (labelme's `--labelflags`, applied to every label).
    pub shape_flags: Vec<String>,

    pub recent_dirs: Vec<PathBuf>,
    pub last_dir: Option<PathBuf>,

    pub sidebar_width: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeSelection::default(),
            theme_overrides: ThemeOverrides::default(),
            auto_save: false,
            save_image_data: false,
            keep_previous: false,
            validate_labels: false,
            recursive_scan: false,
            fill_opacity: 0.25,
            show_labels: true,
            crosshair: true,
            wheel_zooms: true,
            labels: Vec::new(),
            image_flags: Vec::new(),
            shape_flags: Vec::new(),
            recent_dirs: Vec::new(),
            last_dir: None,
            sidebar_width: 280.0,
        }
    }
}

impl Settings {
    pub fn push_recent(&mut self, dir: PathBuf) {
        self.recent_dirs.retain(|d| d != &dir);
        self.recent_dirs.insert(0, dir.clone());
        self.recent_dirs.truncate(MAX_RECENT);
        self.last_dir = Some(dir);
    }
}

pub fn data_dir() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join(crate::APP_NAME))
}

fn path() -> Option<PathBuf> {
    Some(data_dir()?.join("settings.json"))
}

pub fn load() -> Settings {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// writes settings off the UI thread's critical path; failures are not fatal.
pub fn save(settings: &Settings) -> Result<(), String> {
    let path = path().ok_or("no data directory on this platform")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}
