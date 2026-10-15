//! Ultralytics YOLO: `labels/*.txt` with normalized coordinates, class names
//! from `data.yaml` (or `classes.txt`). `class cx cy w h` lines become
//! rectangles, `class x1 y1 x2 y2 ...` lines polygons.

use super::{Importer, Parsed, Record, Source, files_with, polygon, rectangle, subdir_or_self};
use crate::Result;
use crate::geometry::Point;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct Yolo;

/// `.txt` files in a YOLO folder that aren't annotations.
const NOT_LABELS: &[&str] = &[
    "classes.txt",
    "labels.txt",
    "train.txt",
    "val.txt",
    "test.txt",
];

impl Importer for Yolo {
    fn name(&self) -> &'static str {
        "YOLO (detection / segmentation)"
    }

    fn source(&self) -> Source {
        Source::Folder
    }

    fn read(&self, source: &Path) -> Result<Parsed> {
        let mut parsed = Parsed::default();
        let names = class_names(source, &mut parsed.warnings);
        let mut unnamed = false;

        for path in files_with(&subdir_or_self(source, "labels"), &["txt"])? {
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if NOT_LABELS.contains(&file_name.as_str()) {
                continue;
            }
            let text = match crate::text::read(&path) {
                Ok(text) => text,
                Err(e) => {
                    parsed.warnings.push(e.to_string());
                    continue;
                }
            };

            let mut record = Record {
                names: path
                    .file_stem()
                    .map(|s| vec![s.to_string_lossy().into_owned()])
                    .unwrap_or_default(),
                normalized: true,
                ..Default::default()
            };
            for (n, line) in text.lines().enumerate() {
                let mut tokens = line.split_whitespace();
                let Some(class) = tokens.next() else {
                    continue;
                };
                let values: Option<Vec<f32>> = tokens.map(|t| t.parse().ok()).collect();
                let (Ok(class), Some(values)) = (class.parse::<usize>(), values) else {
                    parsed
                        .warnings
                        .push(format!("{}:{}: not a YOLO line", path.display(), n + 1));
                    continue;
                };
                let label = match names.get(&class) {
                    Some(name) => name.clone(),
                    None => {
                        unnamed = true;
                        class.to_string()
                    }
                };
                match values[..] {
                    [cx, cy, w, h] => record.shapes.push(rectangle(
                        &label,
                        Point::new(cx - w / 2.0, cy - h / 2.0),
                        Point::new(cx + w / 2.0, cy + h / 2.0),
                    )),
                    _ if values.len() >= 6 && values.len() % 2 == 0 => {
                        record.shapes.push(polygon(&label, &values))
                    }
                    _ => parsed.warnings.push(format!(
                        "{}:{}: unsupported number of values",
                        path.display(),
                        n + 1
                    )),
                }
            }
            parsed.records.push(record);
        }

        if unnamed {
            parsed
                .warnings
                .push("some class ids have no name; imported as numbers".into());
        }
        Ok(parsed)
    }
}

/// class names from `data.yaml` (or any yaml with `names:`) or `classes.txt`,
/// looked for in `dir` and its parent (when `labels/` itself was picked).
fn class_names(dir: &Path, warnings: &mut Vec<String>) -> BTreeMap<usize, String> {
    let dirs: Vec<&Path> = std::iter::once(dir).chain(dir.parent()).collect();
    for dir in &dirs {
        let mut yamls: Vec<PathBuf> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.extension().is_some_and(|e| {
                        e.eq_ignore_ascii_case("yaml") || e.eq_ignore_ascii_case("yml")
                    })
            })
            .collect();
        // data.yaml first
        yamls.sort_by_key(|p| {
            !p.file_stem()
                .is_some_and(|s| s.eq_ignore_ascii_case("data"))
        });
        for yaml in yamls {
            if let Ok(text) = crate::text::read(&yaml)
                && let Some(names) = yaml_names(&text)
            {
                return names;
            }
        }
        for txt in ["classes.txt", "obj.names"] {
            if let Ok(text) = crate::text::read(&dir.join(txt)) {
                return text
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(str::to_string)
                    .enumerate()
                    .collect();
            }
        }
    }
    warnings.push("no data.yaml or classes.txt found; classes imported as numbers".into());
    BTreeMap::new()
}

/// the `names:` entry of a YOLO data.yaml, in any of its usual forms:
/// a `0: name` mapping, a `- name` list, or inline `[a, b]` / `{0: a}`.
fn yaml_names(text: &str) -> Option<BTreeMap<usize, String>> {
    let mut lines = text.lines();
    let rest = lines.find_map(|l| l.strip_prefix("names:"))?;
    let rest = strip_comment(rest).trim();

    let items: Vec<String> = if rest.is_empty() {
        lines
            .take_while(|l| l.trim().is_empty() || l.starts_with([' ', '\t', '-']))
            .map(|l| strip_comment(l).trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()
    } else {
        let inner = rest
            .strip_prefix(['[', '{'])
            .and_then(|r| r.strip_suffix([']', '}']))?;
        split_unquoted(inner, ',')
    };

    let mut names = BTreeMap::new();
    for (i, item) in items.iter().enumerate() {
        let item = item.trim();
        let (key, value) = match item.strip_prefix('-') {
            Some(value) => (i, value.to_string()),
            None => {
                let parts = split_unquoted(item, ':');
                match parts.first().and_then(|k| k.trim().parse().ok()) {
                    Some(key) if parts.len() > 1 => (key, parts[1..].join(":")),
                    _ => (i, item.to_string()),
                }
            }
        };
        names.insert(key, unquote(value.trim()));
    }
    Some(names)
}

/// `s` up to an unquoted ` #`.
fn strip_comment(s: &str) -> &str {
    let mut quote = None;
    let (mut escaped, mut prev) = (false, ' ');
    for (i, c) in s.char_indices() {
        match (quote, c) {
            _ if escaped => escaped = false,
            (Some('"'), '\\') => escaped = true,
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, '#') if prev.is_whitespace() => return &s[..i],
            _ => {}
        }
        prev = c;
    }
    s
}

/// splits on `sep` outside quotes.
fn split_unquoted(s: &str, sep: char) -> Vec<String> {
    let (mut parts, mut current) = (Vec::new(), String::new());
    let mut quote = None;
    let mut escaped = false;
    for c in s.chars() {
        match (quote, c) {
            _ if escaped => escaped = false,
            (Some('"'), '\\') => escaped = true,
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, c) if c == sep => {
                parts.push(std::mem::take(&mut current));
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts
}

/// a YAML scalar: double-quoted (with escapes), single-quoted or plain.
fn unquote(s: &str) -> String {
    if let Some(inner) = s.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        return inner.replace("''", "'");
    }
    let Some(inner) = s.strip_prefix('"').and_then(|s| s.strip_suffix('"')) else {
        return s.to_string();
    };
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(e @ ('u' | 'x' | 'U')) => {
                let len = match e {
                    'x' => 2,
                    'u' => 4,
                    _ => 8,
                };
                let hex: String = chars.by_ref().take(len).collect();
                if let Some(c) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(c);
                }
            }
            Some(c) => out.push(c),
            None => {}
        }
    }
    out
}
