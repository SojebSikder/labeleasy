//! Dataset importers, the reverse of [`crate::export`]. Each format reads
//! its annotations into [`Record`]s; [`apply`] matches them to the images of
//! a [`Dataset`] and writes labelme files. Adding a format is one new module
//! plus an entry in [`all`].

mod coco;
mod voc;
mod yolo;

pub use coco::Coco;
pub use voc::PascalVoc;
pub use yolo::Yolo;

use crate::dataset::Dataset;
use crate::geometry::Point;
use crate::label_file::{LabelFile, relative_path};
use crate::labels::LabelSet;
use crate::shape::{Shape, ShapeType};
use crate::text::normalize_label;
use crate::{Error, Result, image_io};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// what an importer reads from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// a single file with one of these extensions.
    File(&'static [&'static str]),
    /// a dataset folder.
    Folder,
}

/// the annotations of one image, as read from a dataset.
#[derive(Debug, Clone, Default)]
pub struct Record {
    /// names the source gives the image, best first. May include folders
    /// (`train/img.jpg`) or lack an extension (YOLO pairs files by stem).
    pub names: Vec<String>,
    /// 0 when the source doesn't say; read from the image then.
    pub width: u32,
    pub height: u32,
    /// coordinates are fractions of the image size (YOLO).
    pub normalized: bool,
    pub shapes: Vec<Shape>,
}

#[derive(Debug, Clone, Default)]
pub struct Parsed {
    pub records: Vec<Record>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Summary {
    /// label files written.
    pub images: usize,
    pub annotations: usize,
    /// images left alone because they already have a label file.
    pub skipped_existing: usize,
    /// records naming an image that isn't in the dataset.
    pub unmatched: usize,
    /// shapes dropped as degenerate (e.g. a zero-size box).
    pub skipped_shapes: usize,
    /// every label imported, in first-seen order.
    pub labels: Vec<String>,
    pub warnings: Vec<String>,
}

pub trait Importer: Send + Sync {
    fn name(&self) -> &'static str;

    fn source(&self) -> Source;

    /// reads the annotations at `source`.
    fn read(&self, source: &Path) -> Result<Parsed>;
}

/// every built-in importer.
pub fn all() -> Vec<Box<dyn Importer>> {
    vec![Box::new(Coco), Box::new(Yolo), Box::new(PascalVoc)]
}

/// writes `parsed` as labelme files for the matching images of `dataset`.
/// Images that already have a label file are skipped, never overwritten.
pub fn apply(parsed: Parsed, dataset: &Dataset) -> Result<Summary> {
    let mut summary = Summary {
        warnings: parsed.warnings,
        ..Default::default()
    };
    let index = NameIndex::new(dataset);

    // several records may name one image (e.g. a COCO file listing it twice)
    let mut matched: BTreeMap<usize, Record> = BTreeMap::new();
    for record in parsed.records {
        let Some(i) = index.find(&record.names) else {
            summary.unmatched += 1;
            continue;
        };
        match matched.get_mut(&i) {
            Some(existing) if existing.normalized == record.normalized => {
                existing.shapes.extend(record.shapes)
            }
            Some(_) => summary.warnings.push(format!(
                "{}: annotated twice in different units, kept the first",
                dataset.entries[i].image.display()
            )),
            None => {
                matched.insert(i, record);
            }
        }
    }

    let mut labels = LabelSet::new();
    for (i, record) in matched {
        let entry = &dataset.entries[i];
        if entry.label.is_file() {
            summary.skipped_existing += 1;
            continue;
        }
        let (width, height) = match (record.width, record.height) {
            (w, h) if w > 0 && h > 0 => (w, h),
            _ => match image_io::dimensions(&entry.image) {
                Ok(size) => size,
                Err(e) => {
                    summary.warnings.push(e.to_string());
                    continue;
                }
            },
        };
        let (w, h) = (width as f32, height as f32);
        let scale = if record.normalized {
            Point::new(w, h)
        } else {
            Point::new(1.0, 1.0)
        };

        let mut shapes = Vec::with_capacity(record.shapes.len());
        for mut shape in record.shapes {
            for p in &mut shape.points {
                *p = Point::new(p.x * scale.x, p.y * scale.y).clamp_to(w, h);
            }
            shape.label = normalize_label(&shape.label);
            shape.normalize();
            if !shape.is_valid() {
                summary.skipped_shapes += 1;
                continue;
            }
            labels.insert(&shape.label);
            shapes.push(shape);
        }

        let base = entry.label.parent().unwrap_or(Path::new(""));
        let mut file = LabelFile::new(relative_path(&entry.image, base), width, height);
        summary.annotations += shapes.len();
        file.shapes = shapes;
        file.save(&entry.label)?;
        summary.images += 1;
    }

    summary.labels = labels.as_slice().to_vec();
    Ok(summary)
}

/// the dataset folder `source` belongs to, for opening its images: the
/// picked folder, or the one above a `labels/` / `annotations/` folder
/// (or the file's folder, for COCO).
pub fn dataset_root(source: &Path) -> PathBuf {
    let dir = if source.is_file() {
        source.parent().unwrap_or(source)
    } else {
        source
    };
    let is_annotations = dir
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("labels") || n.eq_ignore_ascii_case("annotations"));
    match dir.parent() {
        Some(parent) if is_annotations => parent.to_path_buf(),
        _ => dir.to_path_buf(),
    }
}

/// looks up dataset entries by file name, then by stem (case-insensitive).
struct NameIndex<'a> {
    dataset: &'a Dataset,
    by_name: HashMap<String, Vec<usize>>,
    by_stem: HashMap<String, Vec<usize>>,
}

impl<'a> NameIndex<'a> {
    fn new(dataset: &'a Dataset) -> Self {
        let (mut by_name, mut by_stem) = (HashMap::new(), HashMap::new());
        for (i, entry) in dataset.entries.iter().enumerate() {
            if let Some(name) = entry.image.file_name() {
                let key = normalize_label(&name.to_string_lossy()).to_lowercase();
                by_name.entry(key).or_insert_with(Vec::new).push(i);
            }
            if let Some(stem) = entry.image.file_stem() {
                let key = normalize_label(&stem.to_string_lossy()).to_lowercase();
                by_stem.entry(key).or_insert_with(Vec::new).push(i);
            }
        }
        Self {
            dataset,
            by_name,
            by_stem,
        }
    }

    fn find(&self, names: &[String]) -> Option<usize> {
        for name in names {
            // sources written on Windows may use backslashes
            let parts: Vec<&str> = name.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
            let Some(file) = parts.last() else {
                continue;
            };
            let file = normalize_label(file).to_lowercase();
            let stem = Path::new(&file)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let candidates = self
                .by_name
                .get(&file)
                .or_else(|| self.by_stem.get(&file))
                .or_else(|| self.by_stem.get(&stem));
            if let Some(candidates) = candidates {
                return Some(self.best(candidates, &parts));
            }
        }
        None
    }

    /// among same-named images, the one whose folders match the source's path.
    fn best(&self, candidates: &[usize], parts: &[&str]) -> usize {
        let folders = &parts[..parts.len().saturating_sub(1)];
        let score = |&i: &usize| {
            let path = &self.dataset.entries[i].image;
            let dirs: Vec<String> = path
                .parent()
                .into_iter()
                .flat_map(Path::components)
                .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
                .collect();
            dirs.iter()
                .rev()
                .zip(folders.iter().rev())
                .take_while(|(a, b)| **a == b.to_lowercase())
                .count()
        };
        // max_by_key keeps the last max; prefer the first image in dataset order
        candidates
            .iter()
            .rev()
            .max_by_key(|i| score(i))
            .copied()
            .unwrap_or(candidates[0])
    }
}

pub(crate) fn rectangle(label: &str, min: Point, max: Point) -> Shape {
    Shape::new(ShapeType::Rectangle, vec![min, max]).with_label(label)
}

pub(crate) fn polygon(label: &str, coords: &[f32]) -> Shape {
    let points = coords
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&[x, y]| Point::new(x, y))
        .collect();
    Shape::new(ShapeType::Polygon, points).with_label(label)
}

/// every file below `dir` whose extension is one of `extensions`, sorted.
pub(crate) fn files_with(dir: &Path, extensions: &[&str]) -> Result<Vec<PathBuf>> {
    fn walk(dir: &Path, extensions: &[&str], out: &mut Vec<PathBuf>) -> Result<()> {
        let read = std::fs::read_dir(dir).map_err(|e| Error::io(dir, e))?;
        for entry in read.flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                let _ = walk(&path, extensions, out);
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| extensions.iter().any(|x| e.eq_ignore_ascii_case(x)))
            {
                out.push(path);
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, extensions, &mut out)?;
    out.sort();
    Ok(out)
}

/// `dir/name` if that's a folder (any case), else `dir`.
pub(crate) fn subdir_or_self(dir: &Path, name: &str) -> PathBuf {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .find(|e| {
            e.file_type().is_ok_and(|t| t.is_dir())
                && e.file_name().to_string_lossy().eq_ignore_ascii_case(name)
        })
        .map(|e| e.path())
        .unwrap_or_else(|| dir.to_path_buf())
}
