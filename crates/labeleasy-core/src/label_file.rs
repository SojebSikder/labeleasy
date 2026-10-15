//! labelme's JSON annotation file, read and written byte-compatibly

use crate::shape::Shape;
use crate::{Error, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

/// the labelme format version written to new files.
pub const FORMAT_VERSION: &str = "5.5.0";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelFile {
    // older/hand-written labelme files omit or null these.
    #[serde(default)]
    pub version: String,
    #[serde(default, deserialize_with = "crate::shape::null_as_default")]
    pub flags: BTreeMap<String, bool>,
    #[serde(default)]
    pub shapes: Vec<Shape>,
    /// relative to the label file's directory.
    pub image_path: String,
    /// base64 of the image file's bytes, if embedded.
    #[serde(default)]
    pub image_data: Option<String>,
    #[serde(default)]
    pub image_height: u32,
    #[serde(default)]
    pub image_width: u32,
    /// keys written by other tools, preserved on save.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl LabelFile {
    pub fn new(image_path: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            version: FORMAT_VERSION.to_string(),
            flags: BTreeMap::new(),
            shapes: Vec::new(),
            image_path: image_path.into(),
            image_data: None,
            image_height: height,
            image_width: width,
            extra: Default::default(),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).map_err(|e| Error::io(path, e))?;
        // Windows editors may prepend a UTF-8 BOM, which serde_json rejects
        let json = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
        serde_json::from_slice(json).map_err(|source| Error::Json {
            path: path.to_path_buf(),
            source,
        })
    }

    /// writes atomically (temp file + rename) so a crash mid-save never
    /// leaves a truncated annotation behind.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(|source| Error::Json {
            path: path.to_path_buf(),
            source,
        })?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| Error::io(&tmp, e))?;
        std::fs::rename(&tmp, path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            Error::io(path, e)
        })
    }

    /// the image this file annotates, resolved against its own location.
    pub fn resolve_image_path(&self, label_path: &Path) -> PathBuf {
        let image = Path::new(&self.image_path);
        if image.is_absolute() {
            return image.to_path_buf();
        }
        // labelme on Windows writes backslashes; normalize so it resolves everywhere
        let normalized: PathBuf = self.image_path.split(['/', '\\']).collect();
        label_path
            .parent()
            .unwrap_or(Path::new(""))
            .join(normalized)
    }

    /// the embedded image bytes, if any.
    pub fn decode_image_data(&self) -> Option<Vec<u8>> {
        let data = self.image_data.as_deref()?;
        base64::engine::general_purpose::STANDARD.decode(data).ok()
    }

    pub fn encode_image_data(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }
}

/// where the annotation for `image` lives: next to it, or in `output_dir`.
pub fn label_path_for(image: &Path, output_dir: Option<&Path>) -> PathBuf {
    let file = image
        .file_stem()
        .map(|s| format!("{}.json", s.to_string_lossy()))
        .unwrap_or_else(|| "annotation.json".to_string());
    match output_dir {
        Some(dir) => dir.join(file),
        None => image.with_file_name(file),
    }
}

/// `target` relative to the directory `base`, with `/` separators. Falls back to the absolute path across drives.
pub fn relative_path(target: &Path, base: &Path) -> String {
    let target_parts: Vec<Component> = target.components().collect();
    let base_parts: Vec<Component> = base.components().collect();
    let common = target_parts
        .iter()
        .zip(&base_parts)
        .take_while(|(a, b)| a == b)
        .count();
    if common == 0 && target.is_absolute() {
        return target.to_string_lossy().into_owned();
    }
    let ups = std::iter::repeat_n("..".to_string(), base_parts.len() - common);
    let rest = target_parts[common..]
        .iter()
        .map(|c| c.as_os_str().to_string_lossy().into_owned());
    ups.chain(rest).collect::<Vec<_>>().join("/")
}
