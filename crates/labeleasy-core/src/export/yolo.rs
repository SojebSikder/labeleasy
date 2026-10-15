//! Ultralytics YOLO layout: `images/`, `labels/*.txt` with normalized
//! coordinates, and `data.yaml` listing the classes.

use super::{Exporter, Item, Summary, UniqueNames, area_outline, copy_image, create_dir, write};
use crate::Result;
use crate::geometry::Rect;
use crate::labels::LabelSet;
use std::fmt::Write as _;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YoloTask {
    /// `class cx cy w h`
    Detection,
    /// `class x1 y1 x2 y2 ...`
    Segmentation,
}

pub struct Yolo {
    pub task: YoloTask,
    /// copy images next to the labels (YOLO expects `images/` + `labels/`).
    pub copy_images: bool,
}

impl Exporter for Yolo {
    fn name(&self) -> &'static str {
        match self.task {
            YoloTask::Detection => "YOLO (detection)",
            YoloTask::Segmentation => "YOLO (segmentation)",
        }
    }

    fn export(&self, items: &[Item], labels: &LabelSet, out_dir: &Path) -> Result<Summary> {
        let (images_dir, labels_dir) = (out_dir.join("images"), out_dir.join("labels"));
        create_dir(&labels_dir)?;
        if self.copy_images {
            create_dir(&images_dir)?;
        }
        let mut summary = Summary::default();
        let mut names = UniqueNames::default();

        for item in items {
            let (w, h) = (
                item.file.image_width.max(1) as f32,
                item.file.image_height.max(1) as f32,
            );
            let mut lines = String::new();
            for shape in &item.file.shapes {
                let Some(class) = labels.index_of(&shape.label) else {
                    summary.unknown_labels += 1;
                    continue;
                };
                let Some(outline) = area_outline(shape) else {
                    summary.skipped_shapes += 1;
                    continue;
                };
                let norm = |v: f32, size: f32| (v / size).clamp(0.0, 1.0);

                match self.task {
                    YoloTask::Detection => {
                        let Some(b) = Rect::bounding(&outline) else {
                            continue;
                        };
                        let _ = writeln!(
                            lines,
                            "{class} {:.6} {:.6} {:.6} {:.6}",
                            norm((b.min.x + b.max.x) / 2.0, w),
                            norm((b.min.y + b.max.y) / 2.0, h),
                            norm(b.width(), w),
                            norm(b.height(), h),
                        );
                    }
                    YoloTask::Segmentation => {
                        let _ = write!(lines, "{class}");
                        for p in &outline {
                            let _ = write!(lines, " {:.6} {:.6}", norm(p.x, w), norm(p.y, h));
                        }
                        lines.push('\n');
                    }
                }
                summary.annotations += 1;
            }

            let name = names.name(item, &mut summary.warnings);
            write(&labels_dir.join(format!("{name}.txt")), lines)?;
            if self.copy_images {
                let target = match item.image.extension() {
                    Some(ext) => images_dir.join(format!("{name}.{}", ext.to_string_lossy())),
                    None => images_dir.join(&name),
                };
                // one unreadable image shouldn't abort the whole export
                if let Err(e) = copy_image(item, &target) {
                    summary.warnings.push(e);
                }
            }
            summary.images += 1;
        }

        let mut yaml = format!(
            "path: {}\ntrain: images\nval: images\nnames:\n",
            out_dir.display()
        );
        for (i, label) in labels.iter().enumerate() {
            let _ = writeln!(yaml, "  {i}: {label:?}");
        }
        write(&out_dir.join("data.yaml"), yaml)?;
        Ok(summary)
    }
}
