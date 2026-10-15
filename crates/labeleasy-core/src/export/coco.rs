//! COCO instance segmentation (`annotations.json`); `group_id` merges
//! shapes into one multi-polygon instance, as labelme2coco does.

use super::{Exporter, Item, Summary, area_outline, create_dir, write};
use crate::geometry::{Rect, polygon_area};
use crate::labels::LabelSet;
use crate::{Error, Result};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;

pub struct Coco;

/// (category index, group id, shape index for ungrouped shapes)
type InstanceKey = (usize, Option<i64>, usize);
/// (category id, polygons)
type Instance = (usize, Vec<Vec<crate::Point>>);

impl Exporter for Coco {
    fn name(&self) -> &'static str {
        "COCO (instance segmentation)"
    }

    fn export(&self, items: &[Item], labels: &LabelSet, out_dir: &Path) -> Result<Summary> {
        create_dir(out_dir)?;
        let mut summary = Summary::default();
        let categories: Vec<Value> = labels
            .iter()
            .enumerate()
            .map(|(i, name)| json!({ "id": i + 1, "name": name, "supercategory": "none" }))
            .collect();

        let mut images = Vec::new();
        let mut annotations = Vec::new();

        for (image_id, item) in items.iter().enumerate() {
            let image_id = image_id + 1;
            images.push(json!({
                "id": image_id,
                "file_name": item
                    .image
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| item.file.image_path.clone()),
                "width": item.file.image_width,
                "height": item.file.image_height,
            }));
            summary.images += 1;

            // instances keyed by (label, group); ungrouped shapes are their own instance
            let mut instances: BTreeMap<InstanceKey, Instance> = BTreeMap::new();
            for (shape_index, shape) in item.file.shapes.iter().enumerate() {
                let Some(category) = labels.index_of(&shape.label) else {
                    summary.unknown_labels += 1;
                    continue;
                };
                let Some(outline) = area_outline(shape) else {
                    summary.skipped_shapes += 1;
                    continue;
                };
                let key = match shape.group_id {
                    Some(g) => (category, Some(g), 0),
                    None => (category, None, shape_index),
                };
                instances
                    .entry(key)
                    .or_insert_with(|| (category + 1, Vec::new()))
                    .1
                    .push(outline);
            }

            for (category_id, polygons) in instances.into_values() {
                let all: Vec<_> = polygons.iter().flatten().copied().collect();
                let Some(bbox) = Rect::bounding(&all) else {
                    continue;
                };
                let area: f32 = polygons.iter().map(|p| polygon_area(p).abs()).sum();
                let segmentation: Vec<Vec<f32>> = polygons
                    .iter()
                    .map(|p| p.iter().flat_map(|pt| [pt.x, pt.y]).collect())
                    .collect();
                annotations.push(json!({
                    "id": annotations.len() + 1,
                    "image_id": image_id,
                    "category_id": category_id,
                    "segmentation": segmentation,
                    "area": area,
                    "bbox": [bbox.min.x, bbox.min.y, bbox.width(), bbox.height()],
                    "iscrowd": 0,
                }));
                summary.annotations += 1;
            }
        }

        let doc = json!({
            "info": { "description": "Exported by Labeleasy" },
            "images": images,
            "annotations": annotations,
            "categories": categories,
        });
        let path = out_dir.join("annotations.json");
        let bytes =
            crate::text::to_ascii_json_pretty(&doc).map_err(|e| Error::Other(e.to_string()))?;
        write(&path, bytes)?;
        Ok(summary)
    }
}
