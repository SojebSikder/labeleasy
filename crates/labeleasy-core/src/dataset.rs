//! A set of images being annotated and where each one's label file lives.

use crate::image_io;
use crate::label_file::{LabelFile, label_path_for};
use crate::{Error, Result};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct ImageEntry {
    pub image: PathBuf,
    pub label: PathBuf,
    /// a label file exists for this image.
    pub annotated: bool,
    /// the label file was opened directly (a `.json`), so it stays where it
    /// is when the output folder changes.
    pub pinned: bool,
}

impl ImageEntry {
    /// `image`'s entry. With an output folder, labels mirror the image's
    /// path below `root` (`out/sub/img.json`), so same-named images in
    /// different subfolders don't share one label file.
    pub fn new(image: PathBuf, output_dir: Option<&Path>, root: Option<&Path>) -> Self {
        let dir = match (output_dir, root) {
            (Some(out), Some(root)) => {
                let sub = image
                    .parent()
                    .and_then(|p| p.strip_prefix(root).ok())
                    .unwrap_or(Path::new(""));
                Some(out.join(sub))
            }
            (out, _) => out.map(Path::to_path_buf),
        };

        let label = label_path_for(&image, dir.as_deref());
        let annotated = label.is_file();

        Self {
            image,
            label,
            annotated,
            pinned: false,
        }
    }

    /// the entry for a labelme `.json` opened directly: the image it points
    /// at (which may exist only embedded in the file), labelled by the file itself.
    pub fn from_label_file(label: PathBuf) -> Self {
        let image = LabelFile::load(&label)
            .map(|f| f.resolve_image_path(&label))
            // unreadable: keep the json as the "image" so opening it reports why
            .unwrap_or_else(|_| label.clone());
        Self {
            image,
            annotated: label.is_file(),
            label,
            pinned: true,
        }
    }

    pub fn file_name(&self) -> String {
        self.image
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

fn is_label_file(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("json"))
}

#[derive(Debug, Clone, Default)]
pub struct Dataset {
    /// the opened folder, if a folder (rather than single files) was opened.
    pub root: Option<PathBuf>,
    /// where label files are written; next to the images when `None`.
    pub output_dir: Option<PathBuf>,
    pub entries: Vec<ImageEntry>,
}

impl Dataset {
    /// every supported image under `root` (recursively if asked), in natural order.
    pub fn scan(root: &Path, recursive: bool, output_dir: Option<PathBuf>) -> Result<Self> {
        let mut images = Vec::new();

        collect_images(root, recursive, &mut images)?;
        images.sort_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));

        let entries = images
            .into_iter()
            .map(|image| ImageEntry::new(image, output_dir.as_deref(), Some(root)))
            .collect();

        Ok(Self {
            root: Some(root.to_path_buf()),
            output_dir,
            entries,
        })
    }

    /// a dataset of hand-picked images and/or labelme `.json` files.
    pub fn from_files(
        files: impl IntoIterator<Item = PathBuf>,
        output_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            root: None,
            entries: files
                .into_iter()
                .map(|file| {
                    if is_label_file(&file) {
                        ImageEntry::from_label_file(file)
                    } else {
                        ImageEntry::new(file, output_dir.as_deref(), None)
                    }
                })
                .collect(),
            output_dir,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn position(&self, image: &Path) -> Option<usize> {
        self.entries.iter().position(|e| e.image == image)
    }

    pub fn annotated_count(&self) -> usize {
        self.entries.iter().filter(|e| e.annotated).count()
    }

    /// moves every (non-pinned) label path to `output_dir` and re-checks which exist.
    pub fn set_output_dir(&mut self, output_dir: Option<PathBuf>) {
        for entry in self.entries.iter_mut().filter(|e| !e.pinned) {
            *entry = ImageEntry::new(
                std::mem::take(&mut entry.image),
                output_dir.as_deref(),
                self.root.as_deref(),
            );
        }
        self.output_dir = output_dir;
    }

    /// indices of entries whose file name contains `query`.
    pub fn filter(&self, query: &str) -> Vec<usize> {
        let q = query.trim().to_lowercase();
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| q.is_empty() || e.file_name().to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect()
    }
}

fn collect_images(dir: &Path, recursive: bool, out: &mut Vec<PathBuf>) -> Result<()> {
    let read = std::fs::read_dir(dir).map_err(|e| Error::io(dir, e))?;

    for entry in read.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            if recursive {
                // unreadable subfolders shouldn't fail the whole scan
                let _ = collect_images(&path, true, out);
            }
        } else if image_io::is_supported(&path) {
            out.push(path);
        }
    }
    Ok(())
}

/// "img2" < "img10": runs of digits compare by value.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut s = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        s.push(c);
                        it.next();
                    }
                    s
                };
                let (na, nb) = (take(&mut a), take(&mut b));
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let ord = ta
                    .len()
                    .cmp(&tb.len())
                    .then_with(|| ta.cmp(tb))
                    .then(na.len().cmp(&nb.len()));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                let ord = x.to_lowercase().cmp(y.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                a.next();
                b.next();
            }
        }
    }
}
