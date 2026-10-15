//! The label vocabulary of a session and the color each label is drawn in.

use crate::Result;
use crate::text::normalize_label;
use std::path::Path;

/// 8-bit RGB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LabelColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// the PASCAL VOC colormap labelme uses (`imgviz.label_colormap`): color
/// `i` is built by spreading the bits of `i` over the three channels.
/// Index 0 is black (background), so labels start at 1.
pub fn colormap(index: usize) -> LabelColor {
    let (mut r, mut g, mut b) = (0u8, 0u8, 0u8);
    let mut id = index;
    for shift in (0..8).rev() {
        r |= ((id & 1) as u8) << shift;
        g |= (((id >> 1) & 1) as u8) << shift;
        b |= (((id >> 2) & 1) as u8) << shift;
        id >>= 3;
        if id == 0 {
            break;
        }
    }
    LabelColor { r, g, b }
}

impl<S: AsRef<str>> FromIterator<S> for LabelSet {
    fn from_iter<I: IntoIterator<Item = S>>(labels: I) -> Self {
        let mut set = Self::new();
        for label in labels {
            set.insert(label.as_ref());
        }
        set
    }
}

/// ordered, de-duplicated labels. A label's color is fixed by its position,
/// so colors stay stable while labels are added.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LabelSet {
    labels: Vec<String>,
}

impl LabelSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// reads a labelme-style `labels.txt`: one label per line, blank lines and
    /// `__ignore__` / `_background_` (labelme's reserved names) skipped.
    /// UTF-8 (with or without BOM) and UTF-16 files are accepted.
    pub fn load_txt(path: &Path) -> Result<Self> {
        let text = crate::text::read(path)?;
        Ok(Self::from_iter(text.lines().map(str::trim).filter(|l| {
            !l.is_empty() && *l != "__ignore__" && *l != "_background_"
        })))
    }

    pub fn save_txt(&self, path: &Path) -> Result<()> {
        let mut text = self.labels.join("\n");
        text.push('\n');
        std::fs::write(path, text).map_err(|e| crate::Error::io(path, e))
    }

    /// adds `label` if new; returns its index. Labels are stored normalized
    /// ([`normalize_label`]), so differently encoded Bangla is one label.
    pub fn insert(&mut self, label: &str) -> usize {
        match self.index_of(label) {
            Some(i) => i,
            None => {
                self.labels.push(normalize_label(label));
                self.labels.len() - 1
            }
        }
    }

    pub fn remove(&mut self, label: &str) {
        let label = normalize_label(label);
        self.labels.retain(|l| *l != label);
    }

    /// matches `label` in any Unicode form (shapes in label files keep the
    /// encoding they were typed in).
    pub fn index_of(&self, label: &str) -> Option<usize> {
        if let Some(i) = self.labels.iter().position(|l| l == label) {
            return Some(i);
        }
        let label = normalize_label(label);
        self.labels.iter().position(|l| *l == label)
    }

    pub fn contains(&self, label: &str) -> bool {
        self.index_of(label).is_some()
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.labels.iter().map(String::as_str)
    }

    pub fn as_slice(&self) -> &[String] {
        &self.labels
    }

    pub fn len(&self) -> usize {
        self.labels.len()
    }

    pub fn is_empty(&self) -> bool {
        self.labels.is_empty()
    }

    /// the color for `label`; unknown labels get a stable color from a hash
    /// of their name so they still look consistent.
    pub fn color(&self, label: &str) -> LabelColor {
        let index = self.index_of(label).unwrap_or_else(|| {
            let hash = label
                .bytes()
                .fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
            200 + (hash % 50) as usize
        });
        colormap(index + 1)
    }

    /// labels starting with (case-insensitively, then containing) `query`,
    /// for the label dialog's suggestion list.
    pub fn suggest(&self, query: &str) -> Vec<&str> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return self.iter().collect();
        }
        let (mut prefix, mut infix) = (Vec::new(), Vec::new());
        for label in self.iter() {
            let lower = label.to_lowercase();
            if lower.starts_with(&q) {
                prefix.push(label);
            } else if lower.contains(&q) {
                infix.push(label);
            }
        }
        prefix.extend(infix);
        prefix
    }
}
