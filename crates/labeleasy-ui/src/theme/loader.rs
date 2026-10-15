//! Finds theme files and turns them into `ThemeEntry`s.

use super::schema::{self, Appearance};
use super::{ThemeEntry, ThemeRegistry, ThemeSource};
use std::path::{Path, PathBuf};

/// theme families compiled into the kit.
const BUNDLED: &[&str] = &[include_str!("../../../../assets/themes/one.json")];

/// `<data dir>/<app_name>/themes` - the user's own theme files.
pub fn user_themes_dir(app_name: &str) -> Option<PathBuf> {
    Some(dirs::data_dir()?.join(app_name).join("themes"))
}

pub fn iced_entry(theme: iced::Theme) -> ThemeEntry {
    ThemeEntry {
        name: theme.to_string(),
        appearance: if theme.extended_palette().is_dark {
            Appearance::Dark
        } else {
            Appearance::Light
        },
        author: None,
        source: ThemeSource::Iced,
        style: Default::default(),
        iced: Some(theme),
    }
}

pub fn iced_entries() -> Vec<ThemeEntry> {
    iced::Theme::ALL.iter().cloned().map(iced_entry).collect()
}

/// every theme in one Zed theme family file.
pub fn entries_from_json(json: &str, source: ThemeSource) -> Result<Vec<ThemeEntry>, String> {
    let family = schema::parse_family(json)?;
    Ok(family
        .themes
        .into_iter()
        .map(|theme| ThemeEntry {
            name: theme.name,
            appearance: theme.appearance,
            author: family.author.clone(),
            source: source.clone(),
            style: theme.style,
            iced: None,
        })
        .collect())
}

pub fn entries_from_file(path: &Path, source: ThemeSource) -> Result<Vec<ThemeEntry>, String> {
    let json = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    entries_from_json(&json, source).map_err(|e| format!("{}: {e}", path.display()))
}

/// the `*.json` files directly inside `dir`, sorted for a stable load order.
pub fn json_files_in(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("json"))
        })
        .collect();
    files.sort();
    files
}

/// builds the full registry: iced's themes, the bundled ones, `extra`
/// (theme family JSON the host app ships) and then `user_dir`'s files.
pub fn load_registry(extra: &[&str], user_dir: Option<&Path>) -> ThemeRegistry {
    let mut registry = ThemeRegistry::default();
    registry.extend(iced_entries());

    for json in BUNDLED.iter().chain(extra) {
        match entries_from_json(json, ThemeSource::Bundled) {
            Ok(entries) => registry.extend(entries),
            Err(e) => registry.errors.push(format!("bundled theme: {e}")),
        }
    }

    if let Some(dir) = user_dir {
        for path in json_files_in(dir) {
            match entries_from_file(&path, ThemeSource::User(path.clone())) {
                Ok(entries) => registry.extend(entries),
                Err(e) => registry.errors.push(e),
            }
        }
    }

    registry
}

/// copies a theme file into `user_dir` after checking it parses, returning
/// the names of the themes it adds.
pub fn import_file(source: &Path, user_dir: &Path) -> Result<Vec<String>, String> {
    let entries = entries_from_file(source, ThemeSource::User(source.to_path_buf()))?;
    if entries.is_empty() {
        return Err("the file doesn't define any themes".to_string());
    }
    std::fs::create_dir_all(user_dir).map_err(|e| e.to_string())?;
    let file_name = source.file_name().ok_or("invalid file name")?;
    std::fs::copy(source, user_dir.join(file_name)).map_err(|e| e.to_string())?;
    Ok(entries.into_iter().map(|e| e.name).collect())
}
