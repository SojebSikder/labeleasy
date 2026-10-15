//! Background image loading plus a byte-budgeted LRU of decoded images, so
//! stepping through a folder shows neighbors instantly (they're prefetched)
//! and iced keeps their GPU textures (the handle ids stay the same).

use iced::widget::image::Handle;
use labeleasy_core::document::Document;
use labeleasy_core::image_io;
use labeleasy_core::label_file::{LabelFile, label_path_for};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// decoded RGBA kept around for quick navigation.
const BUDGET_BYTES: usize = 768 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct LoadedImage {
    pub path: PathBuf,
    pub handle: Handle,
    pub width: u32,
    pub height: u32,
}

impl LoadedImage {
    fn bytes(&self) -> usize {
        self.width as usize * self.height as usize * 4
    }
}

/// an image ready to annotate.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub image: LoadedImage,
    pub doc: Document,
}

#[derive(Debug, Default)]
pub struct ImageCache {
    entries: VecDeque<LoadedImage>,
}

impl ImageCache {
    pub fn get(&mut self, path: &Path) -> Option<LoadedImage> {
        let i = self.entries.iter().position(|e| e.path == path)?;
        let entry = self.entries.remove(i)?;
        self.entries.push_front(entry.clone());
        Some(entry)
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.entries.iter().any(|e| e.path == path)
    }

    pub fn insert(&mut self, image: LoadedImage) {
        self.entries.retain(|e| e.path != image.path);
        self.entries.push_front(image);
        let mut total: usize = self.entries.iter().map(LoadedImage::bytes).sum();
        // always keep the newest, even if it alone exceeds the budget
        while total > BUDGET_BYTES && self.entries.len() > 1 {
            if let Some(evicted) = self.entries.pop_back() {
                total -= evicted.bytes();
            }
        }
    }
}

pub fn decode(path: &Path) -> Result<LoadedImage, String> {
    let rgba = image_io::load(path).map_err(|e| e.to_string())?;
    Ok(from_rgba(path.to_path_buf(), rgba))
}

fn from_rgba(path: PathBuf, rgba: image_io::RgbaImage) -> LoadedImage {
    let pixels = std::sync::Arc::try_unwrap(rgba.pixels).unwrap_or_else(|arc| (*arc).clone());
    LoadedImage {
        path,
        handle: Handle::from_rgba(rgba.width, rgba.height, pixels),
        width: rgba.width,
        height: rgba.height,
    }
}

/// opens `path` (an image, or a labelme `.json`) for annotation. `cached`
/// skips decoding when the image is already in memory.
pub fn load(
    path: &Path,
    label_path: Option<PathBuf>,
    output_dir: Option<&Path>,
    cached: Option<LoadedImage>,
) -> Result<Loaded, String> {
    let is_json = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("json"));

    let (image_path, label_path) = if is_json {
        let file = LabelFile::load(path).map_err(|e| e.to_string())?;
        (file.resolve_image_path(path), path.to_path_buf())
    } else {
        let label = label_path.unwrap_or_else(|| label_path_for(path, output_dir));
        (path.to_path_buf(), label)
    };

    let image = match cached {
        Some(image) => image,
        None if image_path.is_file() => decode(&image_path)?,
        // labelme files can carry the image inline when the original is gone
        None => {
            let file = LabelFile::load(&label_path).map_err(|e| e.to_string())?;
            let bytes = file
                .decode_image_data()
                .ok_or_else(|| format!("{}: image not found", image_path.display()))?;
            let rgba =
                image_io::decode(&bytes).map_err(|e| format!("{}: {e}", label_path.display()))?;
            from_rgba(image_path.clone(), rgba)
        }
    };

    let doc = Document::open(image_path, label_path, image.width, image.height)
        .map_err(|e| e.to_string())?;
    Ok(Loaded { image, doc })
}

/// runs blocking work on tokio's blocking pool.
pub async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| format!("background task failed: {e}"))?
}
