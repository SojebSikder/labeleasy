//! The annotations of one image while they're being edited.

use crate::geometry::Point;
use crate::history::History;
use crate::label_file::{self, LabelFile};
use crate::shape::Shape;
use crate::{Error, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const UNDO_DEPTH: usize = 100;

/// the editable part of a document, which is what undo snapshots.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Annotations {
    pub shapes: Vec<Shape>,
    pub flags: BTreeMap<String, bool>,
}

#[derive(Debug, Clone)]
pub struct Document {
    pub image_path: PathBuf,
    pub label_path: PathBuf,
    pub width: u32,
    pub height: u32,
    state: Annotations,
    history: History<Annotations>,
    revision: u64,
    saved_revision: u64,
    /// the label file's version and unknown keys, carried over on save.
    template: Option<LabelFile>,
}

impl Document {
    /// a blank document for `image_path`, annotated to `label_path`.
    pub fn new(image_path: PathBuf, label_path: PathBuf, width: u32, height: u32) -> Self {
        Self {
            image_path,
            label_path,
            width,
            height,
            state: Annotations::default(),
            history: History::new(UNDO_DEPTH),
            revision: 0,
            saved_revision: 0,
            template: None,
        }
    }

    /// `new`, filled from `label_path` if it exists. Image dimensions are
    /// those of the decoded image, which wins over what the file claims.
    pub fn open(image_path: PathBuf, label_path: PathBuf, width: u32, height: u32) -> Result<Self> {
        let mut doc = Self::new(image_path, label_path, width, height);
        if doc.label_path.is_file() {
            let file = LabelFile::load(&doc.label_path)?;
            doc.state = Annotations {
                shapes: file.shapes.clone(),
                flags: file.flags.clone(),
            };
            doc.template = Some(file);
        }
        Ok(doc)
    }

    pub fn shapes(&self) -> &[Shape] {
        &self.state.shapes
    }

    pub fn flags(&self) -> &BTreeMap<String, bool> {
        &self.state.flags
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn is_dirty(&self) -> bool {
        self.revision != self.saved_revision
    }

    pub fn has_label_file(&self) -> bool {
        self.template.is_some()
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// applies `f` as one undoable step.
    pub fn edit<R>(&mut self, f: impl FnOnce(&mut Annotations) -> R) -> R {
        self.history.record(self.state.clone());
        self.revision += 1;
        f(&mut self.state)
    }

    pub fn undo(&mut self) -> bool {
        let changed = self.history.undo(&mut self.state);
        if changed {
            self.revision += 1;
        }
        changed
    }

    pub fn redo(&mut self) -> bool {
        let changed = self.history.redo(&mut self.state);
        if changed {
            self.revision += 1;
        }
        changed
    }

    fn bounds(&self) -> (f32, f32) {
        (self.width as f32, self.height as f32)
    }

    /// clamps `shape` into the image and normalizes it, as stored shapes are.
    fn prepare(&self, mut shape: Shape) -> Shape {
        let (w, h) = self.bounds();
        // circles may extend past the edge. clamping a mask's bbox would
        // desync it from its bitmap
        if !matches!(
            shape.shape_type,
            crate::ShapeType::Circle | crate::ShapeType::Mask
        ) {
            for p in &mut shape.points {
                *p = p.clamp_to(w, h);
            }
        }
        shape.normalize();
        shape
    }

    /// adds `shape` (clamped into the image) and returns its index.
    pub fn add_shape(&mut self, shape: Shape) -> usize {
        let shape = self.prepare(shape);
        self.edit(|s| {
            s.shapes.push(shape);
            s.shapes.len() - 1
        })
    }

    /// adds shapes (e.g. pasted from another, larger image),
    /// clamped into this image. ones left degenerate by clamping are dropped.
    pub fn add_shapes(&mut self, shapes: Vec<Shape>) -> Vec<usize> {
        let shapes: Vec<Shape> = shapes
            .into_iter()
            .map(|s| self.prepare(s))
            .filter(Shape::is_valid)
            .collect();
        if shapes.is_empty() {
            return Vec::new();
        }

        self.edit(|s| {
            let start = s.shapes.len();
            s.shapes.extend(shapes);
            (start..s.shapes.len()).collect()
        })
    }

    /// removes the shapes at `indices` (any order, duplicates ignored).
    pub fn delete(&mut self, indices: &[usize]) {
        let mut sorted: Vec<usize> = indices
            .iter()
            .copied()
            .filter(|&i| i < self.state.shapes.len())
            .collect();
        if sorted.is_empty() {
            return;
        }
        sorted.sort_unstable();
        sorted.dedup();

        self.edit(|s| {
            for i in sorted.into_iter().rev() {
                s.shapes.remove(i);
            }
        });
    }

    /// moves shapes by `delta`, limited so none leaves the image.
    pub fn translate(&mut self, indices: &[usize], delta: Point) -> Point {
        let (w, h) = self.bounds();
        let delta = indices
            .iter()
            .filter_map(|&i| self.state.shapes.get(i))
            .fold(delta, |d, shape| shape.clamp_delta(d, w, h));
        if delta == Point::ORIGIN || indices.is_empty() {
            return Point::ORIGIN;
        }

        self.edit(|s| {
            for &i in indices {
                if let Some(shape) = s.shapes.get_mut(i) {
                    shape.translate(delta);
                }
            }
        });

        delta
    }

    pub fn move_handle(&mut self, shape: usize, handle: usize, to: Point) {
        if shape >= self.state.shapes.len() {
            return;
        }
        let (w, h) = self.bounds();
        let to = to.clamp_to(w, h);
        self.edit(|s| s.shapes[shape].move_handle(handle, to));
    }

    pub fn insert_vertex(&mut self, shape: usize, index: usize, at: Point) -> bool {
        let ok = self
            .state
            .shapes
            .get(shape)
            .is_some_and(|s| s.can_insert_vertex() && index <= s.points.len());
        if ok {
            self.edit(|s| s.shapes[shape].points.insert(index, at));
        }
        ok
    }

    pub fn remove_vertex(&mut self, shape: usize, index: usize) -> bool {
        let ok = self
            .state
            .shapes
            .get(shape)
            .is_some_and(|s| s.can_remove_vertex() && index < s.points.len());
        if ok {
            self.edit(|s| {
                s.shapes[shape].points.remove(index);
            });
        }
        ok
    }

    /// replaces a shape's label/group/description/flags (not its geometry).
    pub fn update_meta(&mut self, index: usize, meta: ShapeMeta) {
        let Some(current) = self.state.shapes.get(index) else {
            return;
        };
        if meta.matches(current) {
            return;
        }
        self.edit(|s| meta.apply(&mut s.shapes[index]));
    }

    /// relabels several shapes at once.
    pub fn set_label(&mut self, indices: &[usize], label: &str) {
        let changes = indices
            .iter()
            .any(|&i| self.state.shapes.get(i).is_some_and(|s| s.label != label));
        if changes {
            self.edit(|s| {
                for &i in indices {
                    if let Some(shape) = s.shapes.get_mut(i) {
                        shape.label = label.to_string();
                    }
                }
            });
        }
    }

    /// copies of the shapes at `indices`, offset by `offset`; returns the new indices.
    pub fn duplicate(&mut self, indices: &[usize], offset: Point) -> Vec<usize> {
        let (w, h) = self.bounds();
        let copies: Vec<Shape> = indices
            .iter()
            .filter_map(|&i| self.state.shapes.get(i))
            .map(|shape| {
                let mut copy = shape.clone();
                copy.translate(shape.clamp_delta(offset, w, h));
                copy
            })
            .collect();
        self.add_shapes(copies)
    }

    /// moves a shape to the end of the list, drawing it on top of the others.
    pub fn bring_to_front(&mut self, index: usize) -> usize {
        let last = self.state.shapes.len().saturating_sub(1);
        if index < last {
            self.edit(|s| {
                let shape = s.shapes.remove(index);
                s.shapes.push(shape);
            });
        }
        last
    }

    pub fn set_flag(&mut self, name: &str, value: bool) {
        if self.state.flags.get(name) != Some(&value) {
            self.edit(|s| {
                s.flags.insert(name.to_string(), value);
            });
        }
    }

    /// makes sure every flag in `names` exists (unset), without an undo step.
    pub fn ensure_flags<'a>(&mut self, names: impl IntoIterator<Item = &'a str>) {
        for name in names {
            self.state.flags.entry(name.to_string()).or_insert(false);
        }
    }

    /// the label file this document saves as.
    pub fn to_label_file(&self, embed_image: bool) -> Result<LabelFile> {
        let mut file = self
            .template
            .clone()
            .unwrap_or_else(|| LabelFile::new(String::new(), self.width, self.height));
        file.version = label_file::FORMAT_VERSION.to_string();
        file.shapes = self.state.shapes.clone();
        file.flags = self.state.flags.clone();
        file.image_width = self.width;
        file.image_height = self.height;
        let base = self.label_path.parent().unwrap_or(Path::new(""));
        file.image_path = label_file::relative_path(&self.image_path, base);
        file.image_data = if !self.image_path.is_file() {
            // the image only exists embedded in the label file: keep it, or the image would be lost
            file.image_data.take()
        } else if embed_image {
            let bytes =
                std::fs::read(&self.image_path).map_err(|e| Error::io(&self.image_path, e))?;
            Some(LabelFile::encode_image_data(&bytes))
        } else {
            None
        };
        Ok(file)
    }

    pub fn save(&mut self, embed_image: bool) -> Result<()> {
        let file = self.to_label_file(embed_image)?;
        file.save(&self.label_path)?;
        self.template = Some(file);
        self.saved_revision = self.revision;
        Ok(())
    }

    /// deletes the label file from disk and clears the document.
    pub fn delete_label_file(&mut self) -> Result<()> {
        if self.label_path.is_file() {
            std::fs::remove_file(&self.label_path).map_err(|e| Error::io(&self.label_path, e))?;
        }
        self.template = None;
        // image flags stay offered (unset), only their values go
        self.edit(|s| {
            s.shapes.clear();
            s.flags.values_mut().for_each(|v| *v = false);
        });
        self.saved_revision = self.revision;
        Ok(())
    }

    /// marks the current state as saved without writing (e.g. after an
    /// external save, or for a freshly opened file).
    pub fn mark_saved(&mut self) {
        self.saved_revision = self.revision;
    }
}

/// a shape's non-geometric attributes, as edited in the label dialog.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShapeMeta {
    pub label: String,
    pub group_id: Option<i64>,
    pub description: String,
    pub flags: BTreeMap<String, bool>,
}

impl ShapeMeta {
    pub fn of(shape: &Shape) -> Self {
        Self {
            label: shape.label.clone(),
            group_id: shape.group_id,
            description: shape.description.clone(),
            flags: shape.flags.clone(),
        }
    }

    fn matches(&self, shape: &Shape) -> bool {
        self.label == shape.label
            && self.group_id == shape.group_id
            && self.description == shape.description
            && self.flags == shape.flags
    }

    pub fn apply(self, shape: &mut Shape) {
        shape.label = self.label;
        shape.group_id = self.group_id;
        shape.description = self.description;
        shape.flags = self.flags;
    }
}
