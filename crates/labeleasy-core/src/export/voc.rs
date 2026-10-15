//! Pascal VOC detection: one `Annotations/<image>.xml` per image with
//! bounding boxes (every area shape contributes its bounding box).

use super::{Exporter, Item, Summary, UniqueNames, area_outline, create_dir, write};
use crate::Result;
use crate::geometry::Rect;
use crate::labels::LabelSet;
use std::fmt::Write as _;
use std::path::Path;

pub struct PascalVoc;

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

impl Exporter for PascalVoc {
    fn name(&self) -> &'static str {
        "Pascal VOC (detection)"
    }

    fn export(&self, items: &[Item], labels: &LabelSet, out_dir: &Path) -> Result<Summary> {
        let ann_dir = out_dir.join("Annotations");
        create_dir(&ann_dir)?;
        let mut summary = Summary::default();
        let mut names = UniqueNames::default();

        for item in items {
            // <filename> must match the (possibly de-duplicated) annotation name
            let name = names.name(item, &mut summary.warnings);
            let file_name = match item.image.extension() {
                Some(ext) => format!("{name}.{}", ext.to_string_lossy()),
                None => name.clone(),
            };
            let mut xml = String::new();
            let _ = write!(
                xml,
                "<annotation>\n  <folder>JPEGImages</folder>\n  <filename>{}</filename>\n  \
                 <size>\n    <width>{}</width>\n    <height>{}</height>\n    <depth>3</depth>\n  </size>\n  \
                 <segmented>0</segmented>\n",
                escape(&file_name),
                item.file.image_width,
                item.file.image_height
            );
            for shape in &item.file.shapes {
                if !labels.contains(&shape.label) {
                    summary.unknown_labels += 1;
                    continue;
                }
                let Some(b) = area_outline(shape).and_then(|o| Rect::bounding(&o)) else {
                    summary.skipped_shapes += 1;
                    continue;
                };
                let _ = write!(
                    xml,
                    "  <object>\n    <name>{}</name>\n    <pose>Unspecified</pose>\n    <truncated>0</truncated>\n    \
                     <difficult>0</difficult>\n    <bndbox>\n      <xmin>{}</xmin>\n      <ymin>{}</ymin>\n      \
                     <xmax>{}</xmax>\n      <ymax>{}</ymax>\n    </bndbox>\n  </object>\n",
                    escape(&shape.label),
                    b.min.x.round() as i64,
                    b.min.y.round() as i64,
                    b.max.x.round() as i64,
                    b.max.y.round() as i64,
                );
                summary.annotations += 1;
            }
            xml.push_str("</annotation>\n");
            write(&ann_dir.join(format!("{name}.xml")), xml)?;
            summary.images += 1;
        }

        let mut names = labels.as_slice().join("\n");
        names.push('\n');
        write(&out_dir.join("labels.txt"), names)?;
        Ok(summary)
    }
}
