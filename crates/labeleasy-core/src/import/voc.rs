//! Pascal VOC: one `Annotations/<image>.xml` per image; each `<object>`'s
//! `<bndbox>` becomes a rectangle.

use super::{Importer, Parsed, Record, Source, files_with, rectangle, subdir_or_self};
use crate::Result;
use crate::geometry::Point;
use std::path::Path;

pub struct PascalVoc;

impl Importer for PascalVoc {
    fn name(&self) -> &'static str {
        "Pascal VOC (XML)"
    }

    fn source(&self) -> Source {
        Source::Folder
    }

    fn read(&self, source: &Path) -> Result<Parsed> {
        let mut parsed = Parsed::default();

        for path in files_with(&subdir_or_self(source, "Annotations"), &["xml"])? {
            let xml = match crate::text::read(&path) {
                Ok(text) => strip_comments(&text),
                Err(e) => {
                    parsed.warnings.push(e.to_string());
                    continue;
                }
            };
            let Some(root) = elements(&xml, "annotation").into_iter().next() else {
                parsed
                    .warnings
                    .push(format!("{}: not a VOC annotation", path.display()));
                continue;
            };

            // <filename> first; the xml's own name if that doesn't match
            let mut names: Vec<String> = text_of(root, "filename").into_iter().collect();
            names.extend(path.file_stem().map(|s| s.to_string_lossy().into_owned()));
            let size = elements(root, "size").into_iter().next().unwrap_or("");
            let dim = |tag| {
                text_of(size, tag)
                    .and_then(|v| v.parse::<f32>().ok())
                    .map_or(0, |v| v as u32)
            };
            let mut record = Record {
                names,
                width: dim("width"),
                height: dim("height"),
                ..Default::default()
            };

            for object in elements(root, "object") {
                // a person's <part>s carry their own <name> and <bndbox>
                let object = remove_elements(object, "part");
                let label = text_of(&object, "name").unwrap_or_default();
                let Some(bndbox) = elements(&object, "bndbox").into_iter().next() else {
                    continue;
                };
                let coord = |tag| text_of(bndbox, tag).and_then(|v| v.parse::<f32>().ok());
                match (coord("xmin"), coord("ymin"), coord("xmax"), coord("ymax")) {
                    (Some(x0), Some(y0), Some(x1), Some(y1)) => record.shapes.push(rectangle(
                        &label,
                        Point::new(x0, y0),
                        Point::new(x1, y1),
                    )),
                    _ => parsed
                        .warnings
                        .push(format!("{}: object without a full bndbox", path.display())),
                }
            }
            parsed.records.push(record);
        }
        Ok(parsed)
    }
}

fn strip_comments(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        rest = match rest[start..].find("-->") {
            Some(end) => &rest[start + end + 3..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

/// byte range of the next `<tag>` / `<tag attr..>` opening at or after `from`:
/// (start of `<`, end of `>`, self-closing).
fn open_tag(xml: &str, tag: &str, from: usize) -> Option<(usize, usize, bool)> {
    let needle = format!("<{tag}");
    let mut at = from;
    loop {
        let start = at + xml[at..].find(&needle)?;
        let after = start + needle.len();
        // `<name` must not match `<names`
        if xml[after..].starts_with(|c: char| c == '>' || c == '/' || c.is_whitespace()) {
            let end = after + xml[after..].find('>')? + 1;
            return Some((start, end, xml[..end].ends_with("/>")));
        }
        at = after;
    }
}

/// the contents of every top-level `<tag>` element in `xml` (nested
/// same-named elements stay inside their parent's contents).
fn elements<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    element_ranges(xml, tag)
        .into_iter()
        .map(|(_, inner, _)| &xml[inner.0..inner.1])
        .collect()
}

/// (outer start, (inner start, inner end), outer end) of each element.
fn element_ranges(xml: &str, tag: &str) -> Vec<(usize, (usize, usize), usize)> {
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut at = 0;
    while let Some((start, inner_start, self_closing)) = open_tag(xml, tag, at) {
        if self_closing {
            out.push((start, (inner_start, inner_start), inner_start));
            at = inner_start;
            continue;
        }
        // walk past nested same-named elements to the matching close
        let (mut depth, mut pos) = (1, inner_start);
        let end = loop {
            let next_close = xml[pos..].find(&close).map(|i| pos + i);
            let next_open = open_tag(xml, tag, pos).filter(|o| !o.2);
            match (next_open, next_close) {
                (Some((o, oe, _)), Some(c)) if o < c => {
                    depth += 1;
                    pos = oe;
                }
                (_, Some(c)) => {
                    depth -= 1;
                    pos = c + close.len();
                    if depth == 0 {
                        break Some(c);
                    }
                }
                (_, None) => break None,
            }
        };
        let Some(inner_end) = end else { break };
        let outer_end = inner_end + close.len();
        out.push((start, (inner_start, inner_end), outer_end));
        at = outer_end;
    }
    out
}

fn remove_elements(xml: &str, tag: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut last = 0;
    for (start, _, end) in element_ranges(xml, tag) {
        out.push_str(&xml[last..start]);
        last = end;
    }
    out.push_str(&xml[last..]);
    out
}

/// the unescaped, trimmed text of the first `<tag>` in `xml`.
fn text_of(xml: &str, tag: &str) -> Option<String> {
    let inner = elements(xml, tag).into_iter().next()?.trim();
    Some(
        match inner
            .strip_prefix("<![CDATA[")
            .and_then(|s| s.strip_suffix("]]>"))
        {
            Some(cdata) => cdata.to_string(),
            None => unescape(inner),
        },
    )
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let Some(semi) = rest.find(';').filter(|&i| i <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..semi];
        let c = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .map(|h| u32::from_str_radix(h, 16))
                .or_else(|| entity.strip_prefix('#').map(str::parse))
                .and_then(|n| n.ok())
                .and_then(char::from_u32),
        };
        match c {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}
