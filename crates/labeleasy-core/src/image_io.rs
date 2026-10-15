//! Image decoding into RGBA8, honoring EXIF orientation.

use crate::{Error, Result};
use image::{DynamicImage, ImageDecoder, ImageReader};
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

/// extensions (lowercase) of images that can be opened.
pub const SUPPORTED_EXTENSIONS: &[&str] =
    &["png", "jpg", "jpeg", "bmp", "tif", "tiff", "webp", "gif"];

pub fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| SUPPORTED_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// a decoded image, cheap to clone.
#[derive(Debug, Clone)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Arc<Vec<u8>>,
}

impl RgbaImage {
    fn from_dynamic(image: DynamicImage) -> Self {
        let rgba = image.into_rgba8();
        let (width, height) = rgba.dimensions();
        Self {
            width,
            height,
            pixels: Arc::new(rgba.into_raw()),
        }
    }
}

/// decodes `path`.
pub fn load(path: &Path) -> Result<RgbaImage> {
    let bytes = std::fs::read(path).map_err(|e| Error::io(path, e))?;
    decode(&bytes).map_err(|e| Error::Image(path.to_path_buf(), e))
}

/// decodes encoded image bytes (any supported format, sniffed from content).
pub fn decode(bytes: &[u8]) -> Result<RgbaImage, String> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let orientation = decoder.orientation().ok();
    let mut image = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    if let Some(orientation) = orientation {
        image.apply_orientation(orientation);
    }
    Ok(RgbaImage::from_dynamic(image))
}

/// reads only the header to get `(width, height)` - for exporters that need
/// sizes of images without decoding them.
pub fn dimensions(path: &Path) -> Result<(u32, u32)> {
    image::image_dimensions(path).map_err(|e| Error::Image(path.to_path_buf(), e.to_string()))
}
