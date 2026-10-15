//! Dataset exporters. Each format implements [`Exporter`]. adding a format is one new module plus an entry in [`all`].

mod coco;
mod voc;
mod yolo;

pub use coco::Coco;
pub use voc::PascalVoc;
pub use yolo::{Yolo, YoloTask};

use crate::dataset::Dataset;
use crate::geometry::Point;
use crate::label_file::LabelFile;
use crate::labels::LabelSet;
use crate::shape::{Shape, ShapeType};
use crate::{Error, Result};
use std::path::{Path, PathBuf};

/// one annotated image, as handed to exporters.
#[derive(Debug, Clone)]
pub struct Item {
    pub image: PathBuf,
    pub file: LabelFile,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Summary {
    pub images: usize,
    pub annotations: usize,
    /// shapes the format can't represent (e.g. lines in COCO).
    pub skipped_shapes: usize,
    /// shapes whose label isn't in the label set.
    pub unknown_labels: usize,
    pub warnings: Vec<String>,
}

pub trait Exporter: Send + Sync {
    fn name(&self) -> &'static str;

    /// writes `items` into `out_dir`. Class ids follow the order of `labels`.
    fn export(&self, items: &[Item], labels: &LabelSet, out_dir: &Path) -> Result<Summary>;
}

/// every built-in exporter.
pub fn all() -> Vec<Box<dyn Exporter>> {
    vec![
        Box::new(Coco),
        Box::new(Yolo {
            task: YoloTask::Detection,
            copy_images: true,
        }),
        Box::new(Yolo {
            task: YoloTask::Segmentation,
            copy_images: true,
        }),
        Box::new(PascalVoc),
    ]
}

/// loads every annotated image of `dataset`. Unreadable label files become
/// warnings rather than aborting the export.
pub fn collect(dataset: &Dataset) -> (Vec<Item>, Vec<String>) {
    let mut items = Vec::new();
    let mut warnings = Vec::new();

    for entry in dataset.entries.iter().filter(|e| e.label.is_file()) {
        match LabelFile::load(&entry.label) {
            Ok(file) => items.push(Item {
                image: entry.image.clone(),
                file,
            }),
            Err(e) => warnings.push(e.to_string()),
        }
    }
    (items, warnings)
}

/// the label set every item's labels, in first-seen order. for exporting
/// when the user hasn't defined a label list.
pub fn labels_in(items: &[Item]) -> LabelSet {
    LabelSet::from_iter(
        items
            .iter()
            .flat_map(|i| i.file.shapes.iter().map(|s| s.label.as_str())),
    )
}

/// a shape as a polygon outline, if the shape has an area.
pub(crate) fn area_outline(shape: &Shape) -> Option<Vec<Point>> {
    match shape.shape_type {
        ShapeType::Polygon if shape.points.len() >= 3 => Some(shape.points.clone()),
        ShapeType::Rectangle | ShapeType::Mask if shape.points.len() >= 2 => Some(shape.outline()),
        ShapeType::Circle if shape.points.len() >= 2 => {
            const SEGMENTS: usize = 32;
            let (c, r) = (shape.points[0], shape.circle_radius());
            Some(
                (0..SEGMENTS)
                    .map(|i| {
                        let t = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
                        Point::new(c.x + r * t.cos(), c.y + r * t.sin())
                    })
                    .collect(),
            )
        }
        _ => None,
    }
}

pub(crate) fn create_dir(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))
}

pub(crate) fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    std::fs::write(path, contents).map_err(|e| Error::io(path, e))
}

/// hands out output names unique within one export. Formats that pair
/// files by stem (YOLO, VOC) would otherwise let same-named images from
/// different subfolders overwrite each other. repeats get a suffix and a warning.
#[derive(Default)]
pub(crate) struct UniqueNames(std::collections::HashSet<String>);

impl UniqueNames {
    pub fn name(&mut self, item: &Item, warnings: &mut Vec<String>) -> String {
        let base = stem(item);
        let mut name = base.clone();
        let mut n = 1;

        while !self.0.insert(name.to_lowercase()) {
            name = format!("{base}_{n}");
            n += 1;
        }
        if name != base {
            warnings.push(format!(
                "{} exported as {name} (duplicate name)",
                item.image.display()
            ));
        }
        name
    }
}

/// writes `item`'s image to `target`: a copy of the file, or the image
/// embedded in its label file when only that exists.
pub(crate) fn copy_image(item: &Item, target: &Path) -> std::result::Result<(), String> {
    if item.image.is_file() {
        return std::fs::copy(&item.image, target)
            .map(|_| ())
            .map_err(|e| format!("{}: {e}", item.image.display()));
    }

    let bytes = item
        .file
        .decode_image_data()
        .ok_or_else(|| format!("{}: image not found", item.image.display()))?;

    std::fs::write(target, bytes).map_err(|e| format!("{}: {e}", target.display()))
}

/// image file name without extension, falling back to the label file's.
pub(crate) fn stem(item: &Item) -> String {
    item.image
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".into())
}
