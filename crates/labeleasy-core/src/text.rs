//! Unicode helpers so non-Latin labels survive import, matching and export.

use crate::{Error, Result};
use std::path::Path;
use unicode_normalization::{UnicodeNormalization, is_nfc};

pub fn normalize_label(label: &str) -> String {
    let label = label.trim();
    if is_nfc(label) {
        label.to_string()
    } else {
        label.nfc().collect()
    }
}

/// decodes text files from Windows editors: UTF-8 with or without BOM, and UTF-16 (Notepad's "Unicode").
pub fn decode(bytes: &[u8]) -> std::result::Result<String, String> {
    let utf16 = |rest: &[u8], from: fn([u8; 2]) -> u16| {
        let units: Vec<u16> = rest.as_chunks::<2>().0.iter().map(|c| from(*c)).collect();
        String::from_utf16(&units).map_err(|e| e.to_string())
    };
    match bytes {
        [0xEF, 0xBB, 0xBF, rest @ ..] => {
            String::from_utf8(rest.to_vec()).map_err(|e| e.to_string())
        }
        [0xFF, 0xFE, rest @ ..] => utf16(rest, u16::from_le_bytes),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, u16::from_be_bytes),
        _ => String::from_utf8(bytes.to_vec())
            .map_err(|_| "not valid UTF-8 or UTF-16 text".to_string()),
    }
}

pub fn read(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).map_err(|e| Error::io(path, e))?;
    decode(&bytes).map_err(|e| Error::Other(format!("{}: {e}", path.display())))
}

/// a YAML double-quoted scalar.
pub fn yaml_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn to_ascii_json_pretty<T: serde::Serialize>(value: &T) -> serde_json::Result<Vec<u8>> {
    use serde_json::ser::{Formatter, PrettyFormatter, Serializer};
    use std::io::{self, Write};

    struct Ascii<'a>(PrettyFormatter<'a>);

    impl Formatter for Ascii<'_> {
        fn write_string_fragment<W: ?Sized + Write>(
            &mut self,
            w: &mut W,
            s: &str,
        ) -> io::Result<()> {
            for c in s.chars() {
                if c.is_ascii() {
                    w.write_all(&[c as u8])?;
                } else {
                    let mut units = [0u16; 2];
                    for u in c.encode_utf16(&mut units) {
                        write!(w, "\\u{u:04x}")?;
                    }
                }
            }
            Ok(())
        }
        fn begin_array<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
            self.0.begin_array(w)
        }
        fn end_array<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
            self.0.end_array(w)
        }
        fn begin_array_value<W: ?Sized + Write>(
            &mut self,
            w: &mut W,
            first: bool,
        ) -> io::Result<()> {
            self.0.begin_array_value(w, first)
        }
        fn end_array_value<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
            self.0.end_array_value(w)
        }
        fn begin_object<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
            self.0.begin_object(w)
        }
        fn end_object<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
            self.0.end_object(w)
        }
        fn begin_object_key<W: ?Sized + Write>(
            &mut self,
            w: &mut W,
            first: bool,
        ) -> io::Result<()> {
            self.0.begin_object_key(w, first)
        }
        fn begin_object_value<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
            self.0.begin_object_value(w)
        }
        fn end_object_value<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
            self.0.end_object_value(w)
        }
    }

    let mut out = Vec::new();
    let mut ser = Serializer::with_formatter(&mut out, Ascii(PrettyFormatter::new()));
    value.serialize(&mut ser)?;
    Ok(out)
}
