//! COCO detection / instance segmentation (`annotations.json`). Polygons
//! become polygon shapes; an instance of several polygons shares one
//! `group_id`, as the exporter expects. RLE masks fall back to their bbox.

use super::{Importer, Parsed, Record, Source, polygon, rectangle};
use crate::geometry::Point;
use crate::{Error, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

pub struct Coco;

impl Importer for Coco {
    fn name(&self) -> &'static str {
        "COCO (annotations.json)"
    }

    fn source(&self) -> Source {
        Source::File(&["json"])
    }

    fn read(&self, source: &Path) -> Result<Parsed> {
        let text = crate::text::read(source)?;
        let doc: Value = serde_json::from_str(&text).map_err(|e| Error::Json {
            path: source.to_path_buf(),
            source: e,
        })?;
        let not_coco = || Error::Other(format!("{}: not a COCO file", source.display()));
        let images = doc["images"].as_array().ok_or_else(not_coco)?;
        let annotations = doc["annotations"].as_array().ok_or_else(not_coco)?;

        let categories: HashMap<i64, String> = doc["categories"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| Some((c["id"].as_i64()?, c["name"].as_str()?.to_string())))
            .collect();

        let mut parsed = Parsed::default();
        let mut by_id: HashMap<i64, usize> = HashMap::new();
        for image in images {
            let (Some(id), Some(name)) = (image["id"].as_i64(), image["file_name"].as_str()) else {
                continue;
            };
            by_id.insert(id, parsed.records.len());
            parsed.records.push(Record {
                names: vec![name.to_string()],
                width: image["width"].as_u64().unwrap_or(0) as u32,
                height: image["height"].as_u64().unwrap_or(0) as u32,
                ..Default::default()
            });
        }

        let (mut orphans, mut rle) = (0, 0);
        for ann in annotations {
            let Some(&record) = ann["image_id"].as_i64().and_then(|id| by_id.get(&id)) else {
                orphans += 1;
                continue;
            };
            let label = match ann["category_id"].as_i64() {
                Some(id) => categories
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| id.to_string()),
                None => String::new(),
            };

            let polygons: Vec<Vec<f32>> = ann["segmentation"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| {
                    p.as_array()?
                        .iter()
                        .map(|v| v.as_f64().map(|v| v as f32))
                        .collect::<Option<Vec<f32>>>()
                })
                .filter(|p| p.len() >= 6)
                .collect();

            let shapes = &mut parsed.records[record].shapes;
            if polygons.is_empty() {
                if ann["segmentation"].is_object() {
                    rle += 1;
                }
                let bbox: Vec<f32> = ann["bbox"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v.as_f64().map(|v| v as f32))
                    .collect();
                if let [x, y, w, h] = bbox[..] {
                    shapes.push(rectangle(
                        &label,
                        Point::new(x, y),
                        Point::new(x + w, y + h),
                    ));
                }
                continue;
            }

            // a multi-polygon instance: group its parts by the annotation id
            let group = (polygons.len() > 1).then(|| ann["id"].as_i64()).flatten();
            for p in &polygons {
                let mut shape = polygon(&label, p);
                shape.group_id = group;
                shapes.push(shape);
            }
        }

        if orphans > 0 {
            parsed
                .warnings
                .push(format!("{orphans} annotations refer to unknown images"));
        }
        if rle > 0 {
            parsed
                .warnings
                .push(format!("{rle} RLE masks imported as bounding boxes"));
        }
        Ok(parsed)
    }
}
