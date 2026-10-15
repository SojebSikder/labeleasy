//! Labeleasy annotation engine
//!
//! - [`geometry`]: points, rectangles and the hit-testing math shapes use.
//! - [`shape`]: one annotation (polygon, rectangle, circle, ...).
//! - [`label_file`]: the on-disk format - byte-compatible with labelme's JSON.
//! - [`document`]: an image's annotations being edited, with undo/redo.
//! - [`dataset`]: a folder of images and where their annotations live.
//! - [`image_io`]: decoding images (EXIF-oriented) into RGBA buffers.
//! - [`labels`]: the label vocabulary and its stable colors.
//! - [`export`]: COCO / YOLO / Pascal VOC exporters behind one trait.
//! - [`text`]: Unicode normalization, text decoding and escaping for non-Latin labels.

pub mod dataset;
pub mod document;
pub mod export;
pub mod geometry;
pub mod history;
pub mod image_io;
pub mod label_file;
pub mod labels;
pub mod shape;
pub mod text;

pub use dataset::{Dataset, ImageEntry};
pub use document::Document;
pub use geometry::{Point, Rect};
pub use label_file::LabelFile;
pub use labels::{LabelColor, LabelSet};
pub use shape::{Shape, ShapeType};

/// errors surfaced by the core crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path}: invalid label file: {source}")]
    Json {
        path: std::path::PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("{0}: unsupported or corrupt image: {1}")]
    Image(std::path::PathBuf, String),
    #[error("{0}")]
    Other(String),
}

impl Error {
    pub(crate) fn io(path: impl Into<std::path::PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
