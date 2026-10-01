use crate::content::{ContentParser, Operation, Operator};
use crate::document::Document;
use crate::error::{Error, Result};
use crate::stream::{FilterKind, StreamView};
use crate::types::Object;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};

/// Standard PDF font encodings for Latin-script text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontEncoding {
    #[default]
    WinAnsiEncoding,
    StandardEncoding,
    MacRomanEncoding,
    PdfDocEncoding,
    Ascii,
}

impl FontEncoding {
    pub fn from_name(name: &str) -> Self {
        let clean = name.trim_start_matches('/');
        match clean {
            "WinAnsiEncoding" => FontEncoding::WinAnsiEncoding,
            "StandardEncoding" => FontEncoding::StandardEncoding,
            "MacRomanEncoding" => FontEncoding::MacRomanEncoding,
            "PDFDocEncoding" => FontEncoding::PdfDocEncoding,
            "ASCII" | "Ascii" => FontEncoding::Ascii,
            _ => FontEncoding::WinAnsiEncoding,
        }
    }
}

/// Decodes a byte in Adobe WinAnsiEncoding (Windows CP1252) to Unicode char.
#[inline]
pub fn decode_winansi_char(b: u8) -> char {
    match b {
        0x80 => '€',
        0x82 => '‚',
        0x83 => 'ƒ',
        0x84 => '„',
        0x85 => '…',
        0x86 => '†',
        0x87 => '‡',
        0x88 => 'ˆ',
        0x89 => '‰',
        0x8A => 'Š',
        0x8B => '‹',
        0x8C => 'Œ',
        0x8E => 'Ž',
        0x91 => '‘',
        0x92 => '’',
        0x93 => '“',
        0x94 => '”',
        0x95 => '•',
        0x96 => '–',
        0x97 => '—',
        0x98 => '˜',
        0x99 => '™',
        0x9A => 'š',
        0x9B => '›',
        0x9C => 'œ',
        0x9E => 'ž',
        0x9F => 'Ÿ',
        _ => b as char,
    }
}

/// Decodes a byte in Adobe StandardEncoding to Unicode char.
#[inline]
pub fn decode_standard_char(b: u8) -> char {
    match b {
        0x20..=0x7E => b as char,
        0xA1 => '¡',
        0xA2 => '¢',
        0xA3 => '£',
        0xA4 => '⁄',
        0xA5 => '¥',
        0xA6 => 'ƒ',
        0xA7 => '§',
        0xA8 => '¤',
        0xA9 => '\'',
        0xAA => '“',
        0xAB => '«',
        0xAC => '‹',
        0xAD => '›',
        0xAE => 'ﬁ',
        0xAF => 'ﬂ',
        0xB1 => '–',
        0xB2 => '†',
        0xB3 => '‡',
        0xB4 => '·',
        0xB6 => '¶',
        0xB7 => '•',
        0xB8 => '‚',
        0xB9 => '„',
        0xBA => '”',
        0xBB => '»',
        0xBC => '…',
        0xBD => '‰',
        0xBF => '¿',
        0xC1 => '`',
        0xC2 => '´',
        0xC3 => 'ˆ',
        0xC4 => '˜',
        0xC5 => '¯',
        0xC6 => '˘',
        0xC7 => '˙',
        0xC8 => '¨',
        0xCA => '˚',
        0xCB => '¸',
        0xCD => '˝',
        0xCE => '˛',
        0xCF => 'ˇ',
        0xD0 => '—',
        0xE1 => 'Æ',
        0xE3 => 'ª',
        0xE8 => 'Ł',
        0xE9 => 'Ø',
        0xEA => 'Œ',
        0xEB => 'º',
        0xF1 => 'æ',
        0xF8 => 'ł',
        0xF9 => 'ø',
        0xFA => 'œ',
        0xFB => 'ß',
        _ => b as char,
    }
}

/// Decodes a byte in MacRomanEncoding to Unicode char.
#[inline]
pub fn decode_macroman_char(b: u8) -> char {
    match b {
        0x00..=0x7F => b as char,
        0x80 => 'Ä',
        0x81 => 'Å',
        0x82 => 'Ç',
        0x83 => 'É',
        0x84 => 'Ñ',
        0x85 => 'Ö',
        0x86 => 'Ü',
        0x87 => 'á',
        0x88 => 'à',
        0x89 => 'â',
        0x8A => 'ä',
        0x8B => 'ã',
        0x8C => 'å',
        0x8D => 'ç',
        0x8E => 'é',
        0x8F => 'è',
        0x90 => 'ê',
        0x91 => 'ë',
        0x92 => 'í',
        0x93 => 'ì',
        0x94 => 'î',
        0x95 => 'ï',
        0x96 => 'ñ',
        0x97 => 'ó',
        0x98 => 'ò',
        0x99 => 'ô',
        0x9A => 'ö',
        0x9B => 'õ',
        0x9C => 'ú',
        0x9D => 'ù',
        0x9E => 'û',
        0x9F => 'ü',
        0xA0 => '†',
        0xA1 => '°',
        0xA2 => '¢',
        0xA3 => '£',
        0xA4 => '§',
        0xA5 => '•',
        0xA6 => '¶',
        0xA7 => 'ß',
        0xA8 => '®',
        0xA9 => '©',
        0xAA => '™',
        0xAB => '´',
        0xAC => '¨',
        0xAD => '≠',
        0xAE => 'Æ',
        0xAF => 'Ø',
        0xB0 => '∞',
        0xB1 => '±',
        0xB2 => '≤',
        0xB3 => '≥',
        0xB4 => '¥',
        0xB5 => 'µ',
        0xB6 => '∂',
        0xB7 => '∑',
        0xB8 => '∏',
        0xB9 => 'π',
        0xBA => '∫',
        0xBB => 'ª',
        0xBC => 'º',
        0xBD => 'Ω',
        0xBE => 'æ',
        0xBF => 'ø',
        0xC0 => '¿',
        0xC1 => '¡',
        0xC2 => '¬',
        0xC3 => '√',
        0xC4 => 'ƒ',
        0xC5 => '≈',
        0xC6 => '∆',
        0xC7 => '«',
        0xC8 => '»',
        0xC9 => '…',
        0xCA => '\u{00A0}',
        0xCB => 'À',
        0xCC => 'Ã',
        0xCD => 'Õ',
        0xCE => 'Œ',
        0xCF => 'œ',
        0xD0 => '–',
        0xD1 => '—',
        0xD2 => '“',
        0xD3 => '”',
        0xD4 => '‘',
        0xD5 => '’',
        0xD6 => '÷',
        0xD7 => '◊',
        0xD8 => 'ÿ',
        0xD9 => 'Ÿ',
        0xDA => '⁄',
        0xDB => '€',
        0xDC => '‹',
        0xDD => '›',
        0xDE => 'ﬁ',
        0xDF => 'ﬂ',
        0xE0 => '‡',
        0xE1 => '·',
        0xE2 => '‚',
        0xE3 => '„',
        0xE4 => '‰',
        0xE5 => 'Â',
        0xE6 => 'Ê',
        0xE7 => 'Á',
        0xE8 => 'Ë',
        0xE9 => 'È',
        0xEA => 'Í',
        0xEB => 'Î',
        0xEC => 'Ï',
        0xED => 'Ì',
        0xEE => 'Ó',
        0xEF => 'Ô',
        0xF0 => '',
        0xF1 => 'Ò',
        0xF2 => 'Ú',
        0xF3 => 'Û',
        0xF4 => 'Ù',
        0xF5 => 'ı',
        0xF6 => 'ˆ',
        0xF7 => '˜',
        0xF8 => '¯',
        0xF9 => '˘',
        0xFA => '˙',
        0xFB => '˚',
        0xFC => '¸',
        0xFD => '˝',
        0xFE => '˛',
        0xFF => 'ˇ',
    }
}

/// Decodes a byte in PDFDocEncoding to Unicode char.
#[inline]
pub fn decode_pdfdoc_char(b: u8) -> char {
    match b {
        0x18 => '˘',
        0x19 => 'ˇ',
        0x1A => 'ˆ',
        0x1B => '˙',
        0x1C => '˝',
        0x1D => '˛',
        0x1E => '˚',
        0x1F => '˜',
        _ => decode_winansi_char(b),
    }
}

/// Decodes UTF-16BE bytes into a String.
pub fn decode_utf16be(bytes: &[u8]) -> String {
    let u16_iter = bytes
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]));
    char::decode_utf16(u16_iter)
        .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

/// Decodes UTF-16LE bytes into a String.
pub fn decode_utf16le(bytes: &[u8]) -> String {
    let u16_iter = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]));
    char::decode_utf16(u16_iter)
        .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

/// Decodes standard PDF byte strings into Unicode text:
/// - Handles UTF-16BE byte order mark (\xFE\xFF)
/// - Handles UTF-16LE byte order mark (\xFF\xFE)
/// - Handles UTF-8 byte order mark (\xEF\xBB\xBF)
/// - Falls back to specified font encoding (WinAnsi by default)
pub fn decode_text(bytes: &[u8], encoding: FontEncoding) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        return decode_utf16be(&bytes[2..]);
    }
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        return decode_utf16le(&bytes[2..]);
    }
    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        return String::from_utf8_lossy(&bytes[3..]).into_owned();
    }

    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        let ch = match encoding {
            FontEncoding::WinAnsiEncoding => decode_winansi_char(b),
            FontEncoding::StandardEncoding => decode_standard_char(b),
            FontEncoding::MacRomanEncoding => decode_macroman_char(b),
            FontEncoding::PdfDocEncoding => decode_pdfdoc_char(b),
            FontEncoding::Ascii => {
                if b <= 0x7F {
                    b as char
                } else {
                    '?'
                }
            }
        };
        out.push(ch);
    }
    out
}

/// Decodes escape sequences from literal string bytes (e.g. \n, \r, \t, octal escapes).
pub fn decode_literal_escapes(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'\\' && i + 1 < raw.len() {
            i += 1;
            match raw[i] {
                b'n' => out.push(b'\n'),
                b'r' => out.push(b'\r'),
                b't' => out.push(b'\t'),
                b'b' => out.push(0x08),
                b'f' => out.push(0x0C),
                b'(' => out.push(b'('),
                b')' => out.push(b')'),
                b'\\' => out.push(b'\\'),
                b'\r' => {
                    if i + 1 < raw.len() && raw[i + 1] == b'\n' {
                        i += 1;
                    }
                }
                b'\n' => {}
                b'0'..=b'7' => {
                    let mut oct = (raw[i] - b'0') as u16;
                    for _ in 0..2 {
                        if i + 1 < raw.len() && (b'0'..=b'7').contains(&raw[i + 1]) {
                            i += 1;
                            oct = (oct << 3) + ((raw[i] - b'0') as u16);
                        } else {
                            break;
                        }
                    }
                    out.push((oct & 0xFF) as u8);
                }
                other => out.push(other),
            }
        } else {
            out.push(raw[i]);
        }
        i += 1;
    }
    out
}

/// State machine for extracting text and layout from content stream operations.
pub struct TextExtractor {
    pub current_encoding: FontEncoding,
    pub font_encodings: HashMap<String, FontEncoding>,
    pub kerning_threshold: f64,
}

impl Default for TextExtractor {
    fn default() -> Self {
        Self {
            current_encoding: FontEncoding::WinAnsiEncoding,
            font_encodings: HashMap::new(),
            kerning_threshold: -100.0,
        }
    }
}

impl TextExtractor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_font_encodings(mut self, encodings: HashMap<String, FontEncoding>) -> Self {
        self.font_encodings = encodings;
        self
    }

    pub fn with_kerning_threshold(mut self, threshold: f64) -> Self {
        self.kerning_threshold = threshold;
        self
    }

    fn append_text(&self, output: &mut String, text: &str) {
        if output.ends_with(' ') && text.starts_with(' ') {
            output.push_str(text.trim_start());
        } else {
            output.push_str(text);
        }
    }

    fn handle_text_boundary(&self, output: &mut String, ended_block: &mut bool) {
        if *ended_block {
            if !output.is_empty() && !output.ends_with(' ') && !output.ends_with('\n') {
                output.push(' ');
            }
            *ended_block = false;
        }
    }

    /// Extracts plaintext from a sequence of parsed PDF operations.
    pub fn extract(&mut self, operations: &[Operation]) -> String {
        let mut output = String::new();
        let mut ended_block = false;
        let mut last_y: Option<f64> = None;
        let mut last_x: Option<f64> = None;

        for op in operations {
            match op.operator() {
                Operator::BT => {
                    if !output.is_empty() && !output.ends_with('\n') && !output.ends_with(' ') {
                        ended_block = true;
                    }
                }
                Operator::ET => {
                    ended_block = true;
                }
                Operator::Tf => {
                    if let Some(Object::Name(name)) = op.operands().first() {
                        let clean = name.trim_start_matches('/');
                        if let Some(enc) = self
                            .font_encodings
                            .get(clean)
                            .or_else(|| self.font_encodings.get(name.as_ref()))
                        {
                            self.current_encoding = *enc;
                        }
                    }
                }
                Operator::Tj => {
                    if let Some(Object::String(bytes)) = op.operands().first() {
                        self.handle_text_boundary(&mut output, &mut ended_block);
                        let s = decode_text(bytes, self.current_encoding);
                        self.append_text(&mut output, &s);
                    }
                }
                Operator::Quote => {
                    // Move to next line and show text
                    if !output.is_empty() && !output.ends_with('\n') {
                        output.push('\n');
                    }
                    ended_block = false;
                    if let Some(Object::String(bytes)) = op.operands().first() {
                        let s = decode_text(bytes, self.current_encoding);
                        self.append_text(&mut output, &s);
                    }
                }
                Operator::DoubleQuote => {
                    // Set spacing, move to next line, and show text: operands [aw, ac, string]
                    if !output.is_empty() && !output.ends_with('\n') {
                        output.push('\n');
                    }
                    ended_block = false;
                    if let Some(Object::String(bytes)) = op.operands().get(2) {
                        let s = decode_text(bytes, self.current_encoding);
                        self.append_text(&mut output, &s);
                    }
                }
                Operator::TJ => {
                    if let Some(Object::Array(items)) = op.operands().first() {
                        self.handle_text_boundary(&mut output, &mut ended_block);
                        for item in items {
                            match item {
                                Object::String(bytes) => {
                                    let s = decode_text(bytes, self.current_encoding);
                                    self.append_text(&mut output, &s);
                                }
                                Object::Integer(k)
                                    if (*k as f64) <= self.kerning_threshold
                                        && !output.is_empty()
                                        && !output.ends_with(' ')
                                        && !output.ends_with('\n') =>
                                {
                                    output.push(' ');
                                }
                                Object::Real(k)
                                    if *k <= self.kerning_threshold
                                        && !output.is_empty()
                                        && !output.ends_with(' ')
                                        && !output.ends_with('\n') =>
                                {
                                    output.push(' ');
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Operator::TStar => {
                    if !output.is_empty() && !output.ends_with('\n') {
                        output.push('\n');
                    }
                    ended_block = false;
                }
                Operator::Td | Operator::TD => {
                    let tx = op
                        .operands()
                        .first()
                        .and_then(|o| match o {
                            Object::Real(r) => Some(*r),
                            Object::Integer(i) => Some(*i as f64),
                            _ => None,
                        })
                        .unwrap_or(0.0);
                    let ty = op
                        .operands()
                        .get(1)
                        .and_then(|o| match o {
                            Object::Real(r) => Some(*r),
                            Object::Integer(i) => Some(*i as f64),
                            _ => None,
                        })
                        .unwrap_or(0.0);

                    if ty != 0.0 {
                        if !output.is_empty() && !output.ends_with('\n') {
                            output.push('\n');
                        }
                    } else if tx.abs() > 2.0
                        && !output.is_empty()
                        && !output.ends_with(' ')
                        && !output.ends_with('\n')
                    {
                        output.push(' ');
                    }
                    ended_block = false;
                }
                Operator::Tm => {
                    let x = op.operands().get(4).and_then(|o| match o {
                        Object::Real(r) => Some(*r),
                        Object::Integer(i) => Some(*i as f64),
                        _ => None,
                    });
                    let y = op.operands().get(5).and_then(|o| match o {
                        Object::Real(r) => Some(*r),
                        Object::Integer(i) => Some(*i as f64),
                        _ => None,
                    });

                    if let (Some(cur_y), Some(prev_y)) = (y, last_y) {
                        if (cur_y - prev_y).abs() > 1.0 {
                            if !output.is_empty() && !output.ends_with('\n') {
                                output.push('\n');
                            }
                        } else if let (Some(cur_x), Some(prev_x)) = (x, last_x) {
                            if (cur_x - prev_x).abs() > 2.0
                                && !output.is_empty()
                                && !output.ends_with(' ')
                                && !output.ends_with('\n')
                            {
                                output.push(' ');
                            }
                        }
                    }
                    last_x = x;
                    last_y = y;
                    ended_block = false;
                }
                _ => {}
            }
        }

        output.trim().to_string()
    }
}

/// Helper function to parse filter specifications from a stream dictionary.
fn extract_stream_filters(dict: &BTreeMap<Cow<'_, str>, Object<'_>>) -> Result<Vec<FilterKind>> {
    let filter_obj = match dict.get("Filter") {
        Some(f) => f,
        None => return Ok(Vec::new()),
    };

    let mut filters = Vec::new();
    match filter_obj {
        Object::Name(name) => {
            filters.push(parse_filter_name(name)?);
        }
        Object::Array(arr) => {
            for item in arr {
                if let Object::Name(name) = item {
                    filters.push(parse_filter_name(name)?);
                }
            }
        }
        _ => {}
    }
    Ok(filters)
}

fn parse_filter_name(name: &str) -> Result<FilterKind> {
    let clean = name.trim_start_matches('/');
    match clean {
        "FlateDecode" | "Fl" => Ok(FilterKind::FlateDecode),
        "ASCIIHexDecode" | "AHx" => Ok(FilterKind::AsciiHexDecode),
        "ASCII85Decode" | "A85" => Ok(FilterKind::Ascii85Decode),
        _ => Err(Error::UnsupportedFilter {
            name: clean.to_string(),
        }),
    }
}

/// Decompresses stream payload based on its dictionary metadata.
fn decompress_stream(dict: &BTreeMap<Cow<'_, str>, Object<'_>>, data: &[u8]) -> Result<Vec<u8>> {
    let filters = extract_stream_filters(dict)?;
    let mut view = StreamView::new(data);
    for f in filters {
        view = view.with_filter(f);
    }
    view.decode()
}

/// Recursively resolves an object and decompresses stream bytes if it's a stream or array of streams.
fn resolve_stream_bytes(doc: &Document<'_>, obj: &Object<'_>) -> Result<Vec<u8>> {
    let mut visited = std::collections::HashSet::new();
    resolve_stream_bytes_inner(doc, obj, &mut visited, 0)
}

fn resolve_stream_bytes_inner(
    doc: &Document<'_>,
    obj: &Object<'_>,
    visited: &mut std::collections::HashSet<u32>,
    depth: usize,
) -> Result<Vec<u8>> {
    if depth > 32 {
        return Err(Error::RecursionLimitExceeded(32));
    }
    match obj {
        Object::Stream { dict, data } => decompress_stream(dict, data),
        Object::Reference { id, .. } => {
            if !visited.insert(*id) {
                return Err(Error::CyclicReference {
                    object_id: *id,
                    generation: 0,
                });
            }
            if let Some(target) = doc.get_object(*id)? {
                resolve_stream_bytes_inner(doc, &target, visited, depth + 1)
            } else {
                Ok(Vec::new())
            }
        }
        Object::Array(items) => {
            let mut combined = Vec::new();
            for item in items {
                let stream_bytes = resolve_stream_bytes_inner(doc, item, visited, depth + 1)?;
                if !stream_bytes.is_empty() {
                    if combined.len() + stream_bytes.len() + 1 > StreamView::MAX_DECOMPRESS_BYTES {
                        return Err(Error::Unsupported(
                            "combined stream contents exceed 256 MB safety limit",
                        ));
                    }
                    if !combined.is_empty()
                        && !combined.ends_with(b"\n")
                        && !combined.ends_with(b" ")
                    {
                        combined.push(b'\n');
                    }
                    combined.extend_from_slice(&stream_bytes);
                }
            }
            Ok(combined)
        }
        _ => Ok(Vec::new()),
    }
}

/// Gathers font encodings defined in the page dictionary or its inherited resources.
fn collect_font_encodings(
    doc: &Document<'_>,
    page_dict: &BTreeMap<Cow<'_, str>, Object<'_>>,
) -> HashMap<String, FontEncoding> {
    let mut encodings = HashMap::new();

    // Check direct /Resources or inherit from /Parent
    let mut resources_obj = page_dict.get("Resources");
    let mut parent_owned = None;

    if resources_obj.is_none() {
        if let Some(Object::Reference { id: parent_id, .. }) = page_dict.get("Parent") {
            if let Ok(Some(parent)) = doc.get_object(*parent_id) {
                if let Some(dict) = parent.as_dict() {
                    if let Some(r) = dict.get("Resources") {
                        parent_owned = Some(r.clone());
                    }
                }
            }
        }
        resources_obj = parent_owned.as_ref();
    }

    let res_dict = match resources_obj {
        Some(Object::Dictionary(d)) => Some(Cow::Borrowed(d)),
        Some(Object::Reference { id, .. }) => match doc.get_object(*id) {
            Ok(Some(Object::Dictionary(d))) => Some(Cow::Owned(d)),
            _ => None,
        },
        _ => None,
    };

    let res_dict = match res_dict {
        Some(d) => d,
        None => return encodings,
    };

    let font_obj = match res_dict.get("Font") {
        Some(f) => f,
        None => return encodings,
    };

    let font_dict = match font_obj {
        Object::Dictionary(d) => Some(Cow::Borrowed(d)),
        Object::Reference { id, .. } => match doc.get_object(*id) {
            Ok(Some(Object::Dictionary(d))) => Some(Cow::Owned(d)),
            _ => None,
        },
        _ => None,
    };

    let font_dict = match font_dict {
        Some(d) => d,
        None => return encodings,
    };

    for (font_name, font_val) in font_dict.iter() {
        let single_font_dict = match font_val {
            Object::Dictionary(d) => Some(Cow::Borrowed(d)),
            Object::Reference { id, .. } => match doc.get_object(*id) {
                Ok(Some(Object::Dictionary(d))) => Some(Cow::Owned(d)),
                _ => None,
            },
            _ => None,
        };

        if let Some(f_dict) = single_font_dict {
            let encoding = match f_dict.get("Encoding") {
                Some(Object::Name(enc_name)) => FontEncoding::from_name(enc_name),
                Some(Object::Dictionary(enc_dict)) => {
                    if let Some(Object::Name(base_enc)) = enc_dict.get("BaseEncoding") {
                        FontEncoding::from_name(base_enc)
                    } else {
                        FontEncoding::WinAnsiEncoding
                    }
                }
                _ => FontEncoding::WinAnsiEncoding,
            };

            let clean = font_name.trim_start_matches('/').to_string();
            encodings.insert(font_name.to_string(), encoding);
            encodings.insert(clean, encoding);
        }
    }

    encodings
}

/// Reads the page dictionary, finds `/Contents`, decompresses them,
/// parses operations with the `content` module, and reconstructs the plaintext string.
pub fn extract_page_text(doc: &Document<'_>, page_id: u32) -> Result<String> {
    let page_obj = match doc.get_object(page_id)? {
        Some(obj) => obj,
        None => {
            return Err(Error::SyntaxError {
                offset: 0,
                message: "Page object not found in document",
            })
        }
    };

    let page_dict = match page_obj.as_dict() {
        Some(dict) => dict,
        None => {
            return Err(Error::SyntaxError {
                offset: 0,
                message: "Page object is not a dictionary",
            })
        }
    };

    let contents_obj = match page_dict.get("Contents") {
        Some(c) => c,
        None => return Ok(String::new()), // Empty page with no contents
    };

    let decompressed_content = resolve_stream_bytes(doc, contents_obj)?;
    if decompressed_content.is_empty() {
        return Ok(String::new());
    }

    let font_encodings = collect_font_encodings(doc, page_dict);

    let mut parser = ContentParser::new(&decompressed_content);
    let operations = parser.parse()?;

    let mut extractor = TextExtractor::new().with_font_encodings(font_encodings);
    let text = extractor.extract(&operations);

    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoding_decoders() {
        // UTF-16BE BOM \xFE\xFF
        let utf16be_bytes = [0xFE, 0xFF, 0x00, 0x48, 0x00, 0x69, 0x00, 0x21];
        assert_eq!(
            decode_text(&utf16be_bytes, FontEncoding::WinAnsiEncoding),
            "Hi!"
        );

        // WinAnsi Euro & TM
        let winansi = [0x80, 0x20, 0x99];
        assert_eq!(decode_text(&winansi, FontEncoding::WinAnsiEncoding), "€ ™");

        // MacRoman German & umlauts
        let macroman = [0x80, 0x81, 0x82];
        assert_eq!(
            decode_text(&macroman, FontEncoding::MacRomanEncoding),
            "ÄÅÇ"
        );

        // StandardEncoding cent & sterling
        let standard = [0xA2, 0xA3];
        assert_eq!(decode_text(&standard, FontEncoding::StandardEncoding), "¢£");

        // Escapes
        let escaped = b"Hello\\nWorld\\t\\101";
        let raw = decode_literal_escapes(escaped);
        assert_eq!(raw, b"Hello\nWorld\tA");
    }

    #[test]
    fn test_tj_and_tj_kerning_extraction() {
        // Tj simple
        let content = b"BT /F1 12 Tf 100 700 Td (Hello World) Tj ET";
        let mut parser = ContentParser::new(content);
        let ops = parser.parse().unwrap();
        let mut extractor = TextExtractor::new();
        assert_eq!(extractor.extract(&ops), "Hello World");

        // TJ with kerning: negative displacement creates space
        let content_tj = b"BT /F1 12 Tf [(Hello) -250 (World)] TJ ET";
        let mut parser2 = ContentParser::new(content_tj);
        let ops2 = parser2.parse().unwrap();
        let mut extractor2 = TextExtractor::new();
        assert_eq!(extractor2.extract(&ops2), "Hello World");

        // TJ positive kerning does not create space
        let content_tight = b"BT /F1 12 Tf [(Hello) 100 (World)] TJ ET";
        let mut parser3 = ContentParser::new(content_tight);
        let ops3 = parser3.parse().unwrap();
        let mut extractor3 = TextExtractor::new();
        assert_eq!(extractor3.extract(&ops3), "HelloWorld");
    }

    #[test]
    fn test_synthetic_pdf_document_text_extraction() {
        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.4\n");
        // Object 1: Catalog
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
        // Object 2: Pages root with 4 pages
        pdf.extend_from_slice(
            b"2 0 obj\n<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R 6 0 R] /Count 4 >>\nendobj\n",
        );
        // Object 3: Page 1 with single Tj stream
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /Contents 7 0 R >>\nendobj\n",
        );
        // Object 4: Page 2 with TJ kerning stream
        pdf.extend_from_slice(
            b"4 0 obj\n<< /Type /Page /Parent 2 0 R /Contents 8 0 R >>\nendobj\n",
        );
        // Object 5: Page 3 with multi-stream contents array
        pdf.extend_from_slice(
            b"5 0 obj\n<< /Type /Page /Parent 2 0 R /Contents [9 0 R 10 0 R] >>\nendobj\n",
        );
        // Object 6: Page 4 with no /Contents (empty page)
        pdf.extend_from_slice(b"6 0 obj\n<< /Type /Page /Parent 2 0 R >>\nendobj\n");

        // Stream 7: Tj
        let s7 = b"BT /F1 12 Tf 100 700 Td (Hello World from Tj!) Tj ET";
        pdf.extend_from_slice(format!("7 0 obj\n<< /Length {} >>\nstream\n", s7.len()).as_bytes());
        pdf.extend_from_slice(s7);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        // Stream 8: TJ with negative kerning creating spaces
        let s8 = b"BT /F1 12 Tf [(Hello) -250 (TJ) -250 (World)] TJ ET";
        pdf.extend_from_slice(format!("8 0 obj\n<< /Length {} >>\nstream\n", s8.len()).as_bytes());
        pdf.extend_from_slice(s8);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        // Stream 9: First stream of multi-stream page
        let s9 = b"BT /F1 12 Tf (First Stream) Tj ET";
        pdf.extend_from_slice(format!("9 0 obj\n<< /Length {} >>\nstream\n", s9.len()).as_bytes());
        pdf.extend_from_slice(s9);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        // Stream 10: Second stream of multi-stream page with newline displacement
        let s10 = b"BT /F1 12 Tf 0 -15 Td (Second Stream) Tj ET";
        pdf.extend_from_slice(
            format!("10 0 obj\n<< /Length {} >>\nstream\n", s10.len()).as_bytes(),
        );
        pdf.extend_from_slice(s10);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        // Trailer
        pdf.extend_from_slice(b"trailer\n<< /Size 11 /Root 1 0 R >>\nstartxref\n0\n%%EOF");

        let doc = Document::load(&pdf).expect("Document load failed");
        let page_ids = doc.get_page_ids().expect("Failed to get page IDs");
        assert_eq!(page_ids, vec![3, 4, 5, 6]);

        // Page 1: Tj single stream
        let text1 = doc.extract_text(3).expect("Extract page 1 failed");
        assert_eq!(text1, "Hello World from Tj!");

        // Page 2: TJ kerning
        let text2 = doc.extract_text(4).expect("Extract page 2 failed");
        assert_eq!(text2, "Hello TJ World");

        // Page 3: Multi-stream
        let text3 = doc.extract_text(5).expect("Extract page 3 failed");
        assert_eq!(text3, "First Stream\nSecond Stream");

        // Page 4: Empty page (no /Contents)
        let text4 = doc.extract_text(6).expect("Extract page 4 failed");
        assert_eq!(text4, "");

        // extract_text_all
        let all_text = doc.extract_text_all().expect("Extract all failed");
        assert_eq!(
            all_text,
            vec![
                "Hello World from Tj!".to_string(),
                "Hello TJ World".to_string(),
                "First Stream\nSecond Stream".to_string(),
                "".to_string(),
            ]
        );
    }

    #[test]
    fn test_synthetic_pdf_flate_and_font_encodings() {
        use flate2::write::ZlibEncoder;
        use flate2::Compression;
        use std::io::Write as IoWrite;

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(b"BT /F1 12 Tf (Compressed Flate Text!) Tj ET")
            .unwrap();
        let flate_bytes = encoder.finish().unwrap();

        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.4\n");
        // Object 1: Catalog
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
        // Object 2: Pages root
        pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
        // Object 3: Page with font resource specifying WinAnsiEncoding and Flate stream
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 << /Type /Font /Encoding /WinAnsiEncoding >> >> >> /Contents 4 0 R >>\nendobj\n",
        );
        // Object 4: Flate-compressed stream
        pdf.extend_from_slice(
            format!(
                "4 0 obj\n<< /Filter /FlateDecode /Length {} >>\nstream\n",
                flate_bytes.len()
            )
            .as_bytes(),
        );
        pdf.extend_from_slice(&flate_bytes);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        pdf.extend_from_slice(b"trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n0\n%%EOF");

        let doc = Document::load(&pdf).expect("Load flate PDF failed");
        let text = doc.extract_text(3).expect("Extract flate page failed");
        assert_eq!(text, "Compressed Flate Text!");
    }
}
