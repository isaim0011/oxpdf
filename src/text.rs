use crate::cmap::CMap;
use crate::content::{ContentParser, Operation, Operator};
use crate::document::Document;
use crate::error::{Error, Result};
use crate::font::{CffFont, EmbeddedFont, TrueTypeFont};
use crate::geom::{FontInfo, GraphicsStateTracker, TextSpan};
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
    pub font_cmaps: HashMap<String, CMap>,
    pub font_embedded: HashMap<String, EmbeddedFont>,
    pub current_cmap: Option<CMap>,
    pub current_embedded: Option<EmbeddedFont>,
    pub kerning_threshold: f64,
}

impl Default for TextExtractor {
    fn default() -> Self {
        Self {
            current_encoding: FontEncoding::WinAnsiEncoding,
            font_encodings: HashMap::new(),
            font_cmaps: HashMap::new(),
            font_embedded: HashMap::new(),
            current_cmap: None,
            current_embedded: None,
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

    pub fn with_font_cmaps(mut self, cmaps: HashMap<String, CMap>) -> Self {
        self.font_cmaps = cmaps;
        self
    }

    pub fn with_font_embedded(mut self, embedded: HashMap<String, EmbeddedFont>) -> Self {
        self.font_embedded = embedded;
        self
    }

    pub fn with_kerning_threshold(mut self, threshold: f64) -> Self {
        self.kerning_threshold = threshold;
        self
    }

    #[inline]
    fn decode_text_bytes(&self, bytes: &[u8]) -> String {
        if let Some(ref cmap) = self.current_cmap {
            cmap.decode_string_with_fallback(bytes, self.current_encoding)
        } else if let Some(ref embedded) = self.current_embedded {
            let mut out = String::new();
            for &b in bytes {
                if let Some(ch) = embedded.map_glyph_to_unicode(b as u16) {
                    out.push(ch);
                } else {
                    out.push(crate::cmap::decode_fallback_char(b, self.current_encoding));
                }
            }
            out
        } else {
            decode_text(bytes, self.current_encoding)
        }
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
                        self.current_cmap = self
                            .font_cmaps
                            .get(clean)
                            .or_else(|| self.font_cmaps.get(name.as_ref()))
                            .cloned();
                        self.current_embedded = self
                            .font_embedded
                            .get(clean)
                            .or_else(|| self.font_embedded.get(name.as_ref()))
                            .cloned();
                    }
                }
                Operator::Tj => {
                    if let Some(Object::String(bytes)) = op.operands().first() {
                        self.handle_text_boundary(&mut output, &mut ended_block);
                        let s = self.decode_text_bytes(bytes);
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
                        let s = self.decode_text_bytes(bytes);
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
                        let s = self.decode_text_bytes(bytes);
                        self.append_text(&mut output, &s);
                    }
                }
                Operator::TJ => {
                    if let Some(Object::Array(items)) = op.operands().first() {
                        self.handle_text_boundary(&mut output, &mut ended_block);
                        for item in items {
                            match item {
                                Object::String(bytes) => {
                                    let s = self.decode_text_bytes(bytes);
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

    let decode_parms_obj = dict.get("DecodeParms");

    let mut filters = Vec::new();
    match filter_obj {
        Object::Name(name) => {
            let parms_dict = decode_parms_obj.and_then(|o| o.as_dict());
            filters.push(parse_filter_with_parms(name, parms_dict)?);
        }
        Object::Array(arr) => {
            let parms_arr = match decode_parms_obj {
                Some(Object::Array(pa)) => Some(pa.as_slice()),
                _ => None,
            };
            for (i, item) in arr.iter().enumerate() {
                if let Object::Name(name) = item {
                    let parms_dict = parms_arr
                        .and_then(|pa| pa.get(i))
                        .and_then(|o| o.as_dict());
                    filters.push(parse_filter_with_parms(name, parms_dict)?);
                }
            }
        }
        _ => {}
    }
    Ok(filters)
}

fn parse_filter_with_parms(
    name: &str,
    parms: Option<&BTreeMap<Cow<'_, str>, Object<'_>>>,
) -> Result<FilterKind> {
    let clean = name.trim_start_matches('/');
    match clean {
        "FlateDecode" | "Fl" => Ok(FilterKind::FlateDecode),
        "ASCIIHexDecode" | "AHx" => Ok(FilterKind::AsciiHexDecode),
        "ASCII85Decode" | "A85" => Ok(FilterKind::Ascii85Decode),
        "CCITTFaxDecode" | "CCF" => {
            let mut ccitt_params = crate::filter::CcittParams::default();
            if let Some(dict) = parms {
                if let Some(Object::Integer(k)) = dict.get("K") {
                    ccitt_params.k = *k as i32;
                }
                if let Some(Object::Boolean(eol)) = dict.get("EndOfLine") {
                    ccitt_params.end_of_line = *eol;
                }
                if let Some(Object::Boolean(b1)) = dict.get("BlackIs1") {
                    ccitt_params.black_is1 = *b1;
                }
                if let Some(Object::Integer(cols)) = dict.get("Columns") {
                    ccitt_params.columns = (*cols).max(1) as u32;
                }
                if let Some(Object::Integer(rows)) = dict.get("Rows") {
                    ccitt_params.rows = (*rows).max(0) as u32;
                }
                if let Some(Object::Boolean(eob)) = dict.get("EndOfBlock") {
                    ccitt_params.end_of_block = *eob;
                }
                if let Some(Object::Boolean(align)) = dict.get("EncodedByteAlign") {
                    ccitt_params.encoded_byte_align = *align;
                }
            }
            Ok(FilterKind::CCITTFaxDecode {
                params: Some(Box::new(ccitt_params)),
            })
        }
        "JBIG2Decode" => {
            let mut jbig2_params = crate::filter::Jbig2Params::default();
            if let Some(dict) = parms {
                if let Some(Object::Stream { data, .. }) = dict.get("JBIG2Globals") {
                    jbig2_params.globals = Some(data.to_vec());
                }
            }
            Ok(FilterKind::JBIG2Decode {
                params: Some(Box::new(jbig2_params)),
            })
        }
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

fn resolve_to_unicode_cmap(
    doc: &Document<'_>,
    font_dict: &BTreeMap<Cow<'_, str>, Object<'_>>,
) -> Option<CMap> {
    let to_unicode_obj: Object<'_> = if let Some(obj) = font_dict.get("ToUnicode") {
        obj.clone()
    } else if let Some(Object::Array(descendants)) = font_dict.get("DescendantFonts") {
        match descendants.first() {
            Some(Object::Dictionary(d)) => d.get("ToUnicode").cloned()?,
            Some(Object::Reference { id, .. }) => {
                if let Ok(Some(Object::Dictionary(d))) = doc.get_object(*id) {
                    d.get("ToUnicode").cloned()?
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    } else {
        return None;
    };

    let stream_bytes = resolve_stream_bytes(doc, &to_unicode_obj).ok()?;
    if stream_bytes.is_empty() {
        return None;
    }
    CMap::parse(&stream_bytes).ok()
}

/// Attempts to locate and parse an embedded font (/FontFile2 for TrueType, /FontFile3 for CFF)
/// from the font dictionary or its descriptor.
fn resolve_embedded_font(
    doc: &Document<'_>,
    font_dict: &BTreeMap<Cow<'_, str>, Object<'_>>,
) -> Option<EmbeddedFont> {
    // 1. Resolve /FontDescriptor
    let desc_dict: Option<BTreeMap<Cow<'static, str>, Object<'static>>> = if let Some(desc_obj) = font_dict.get("FontDescriptor") {
        match desc_obj {
            Object::Dictionary(d) => Some(d.clone().into_iter().map(|(k, v)| (Cow::Owned(k.into_owned()), v.into_owned())).collect()),
            Object::Reference { id, .. } => match doc.get_object(*id) {
                Ok(Some(Object::Dictionary(d))) => Some(d.into_iter().map(|(k, v)| (Cow::Owned(k.into_owned()), v.into_owned())).collect()),
                _ => None,
            },
            _ => None,
        }
    } else if let Some(Object::Array(descendants)) = font_dict.get("DescendantFonts") {
        match descendants.first() {
            Some(Object::Dictionary(d)) => match d.get("FontDescriptor") {
                Some(Object::Dictionary(desc)) => Some(desc.clone().into_iter().map(|(k, v)| (Cow::Owned(k.into_owned()), v.into_owned())).collect()),
                Some(Object::Reference { id, .. }) => match doc.get_object(*id) {
                    Ok(Some(Object::Dictionary(desc))) => Some(desc.into_iter().map(|(k, v)| (Cow::Owned(k.into_owned()), v.into_owned())).collect()),
                    _ => None,
                },
                _ => None,
            },
            Some(Object::Reference { id, .. }) => match doc.get_object(*id) {
                Ok(Some(Object::Dictionary(d))) => match d.get("FontDescriptor") {
                    Some(Object::Dictionary(desc)) => Some(desc.clone().into_iter().map(|(k, v)| (Cow::Owned(k.into_owned()), v.into_owned())).collect()),
                    Some(Object::Reference { id: desc_id, .. }) => match doc.get_object(*desc_id) {
                        Ok(Some(Object::Dictionary(desc))) => Some(desc.into_iter().map(|(k, v)| (Cow::Owned(k.into_owned()), v.into_owned())).collect()),
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        }
    } else {
        None
    };

    // 2. Check /FontFile2 (TrueType) in FontDescriptor, or fallback directly on font_dict
    let ff2_obj = desc_dict
        .as_ref()
        .and_then(|d| d.get("FontFile2"))
        .or_else(|| font_dict.get("FontFile2"));

    if let Some(ff2) = ff2_obj {
        if let Ok(bytes) = resolve_stream_bytes(doc, ff2) {
            if !bytes.is_empty() {
                if let Ok(ttf) = TrueTypeFont::parse(&bytes) {
                    return Some(EmbeddedFont::TrueType(ttf));
                }
            }
        }
    }

    // 3. Check /FontFile3 (CFF) in FontDescriptor, or fallback directly on font_dict
    let ff3_obj = desc_dict
        .as_ref()
        .and_then(|d| d.get("FontFile3"))
        .or_else(|| font_dict.get("FontFile3"));

    if let Some(ff3) = ff3_obj {
        if let Ok(bytes) = resolve_stream_bytes(doc, ff3) {
            if !bytes.is_empty() {
                if let Ok(cff) = CffFont::parse(&bytes) {
                    return Some(EmbeddedFont::Cff(cff));
                }
            }
        }
    }

    None
}

/// Gathers font encodings and ToUnicode CMaps defined in the page dictionary or its inherited resources.
fn collect_font_resources(
    doc: &Document<'_>,
    page_dict: &BTreeMap<Cow<'_, str>, Object<'_>>,
) -> (
    HashMap<String, FontEncoding>,
    HashMap<String, CMap>,
    HashMap<String, EmbeddedFont>,
) {
    let mut encodings = HashMap::new();
    let mut cmaps = HashMap::new();
    let mut embedded_fonts = HashMap::new();

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
        None => return (encodings, cmaps, embedded_fonts),
    };

    let font_obj = match res_dict.get("Font") {
        Some(f) => f,
        None => return (encodings, cmaps, embedded_fonts),
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
        None => return (encodings, cmaps, embedded_fonts),
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

            let maybe_cmap = resolve_to_unicode_cmap(doc, &f_dict);
            let maybe_embedded = if maybe_cmap.is_none() {
                resolve_embedded_font(doc, &f_dict)
            } else {
                None
            };

            let clean = font_name.trim_start_matches('/').to_string();
            encodings.insert(font_name.to_string(), encoding);
            encodings.insert(clean.clone(), encoding);

            if let Some(cmap) = maybe_cmap {
                cmaps.insert(font_name.to_string(), cmap.clone());
                cmaps.insert(clean.clone(), cmap);
            } else if let Some(ref embedded) = maybe_embedded {
                let embedded_cmap = embedded.to_cmap();
                cmaps.insert(font_name.to_string(), embedded_cmap.clone());
                cmaps.insert(clean.clone(), embedded_cmap);
            }

            if let Some(embedded) = maybe_embedded {
                embedded_fonts.insert(font_name.to_string(), embedded.clone());
                embedded_fonts.insert(clean, embedded);
            }
        }
    }

    (encodings, cmaps, embedded_fonts)
}

#[allow(dead_code)]
fn collect_font_encodings(
    doc: &Document<'_>,
    page_dict: &BTreeMap<Cow<'_, str>, Object<'_>>,
) -> HashMap<String, FontEncoding> {
    collect_font_resources(doc, page_dict).0
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

    let (font_encodings, font_cmaps, font_embedded) = collect_font_resources(doc, page_dict);

    let mut parser = ContentParser::new(&decompressed_content);
    let operations = parser.parse()?;

    let mut extractor = TextExtractor::new()
        .with_font_encodings(font_encodings)
        .with_font_cmaps(font_cmaps)
        .with_font_embedded(font_embedded);
    let text = extractor.extract(&operations);

    Ok(text)
}

#[derive(Default)]
struct ExtendedFontResources {
    encodings: HashMap<String, FontEncoding>,
    cmaps: HashMap<String, CMap>,
    widths: HashMap<String, HashMap<u32, f32>>,
    missing_widths: HashMap<String, f32>,
    font_info: HashMap<String, FontInfo>,
}

fn collect_extended_font_resources(
    doc: &Document<'_>,
    page_dict: &BTreeMap<Cow<'_, str>, Object<'_>>,
) -> ExtendedFontResources {
    let mut encodings = HashMap::new();
    let mut cmaps = HashMap::new();
    let mut widths = HashMap::new();
    let mut missing_widths = HashMap::new();
    let mut font_info = HashMap::new();

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
        None => {
            return ExtendedFontResources {
                encodings,
                cmaps,
                widths,
                missing_widths,
                font_info,
            }
        }
    };

    let font_obj = match res_dict.get("Font") {
        Some(f) => f,
        None => {
            return ExtendedFontResources {
                encodings,
                cmaps,
                widths,
                missing_widths,
                font_info,
            }
        }
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
        None => {
            return ExtendedFontResources {
                encodings,
                cmaps,
                widths,
                missing_widths,
                font_info,
            }
        }
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

            let maybe_cmap = resolve_to_unicode_cmap(doc, &f_dict);

            let clean = font_name.trim_start_matches('/').to_string();
            encodings.insert(font_name.to_string(), encoding);
            encodings.insert(clean.clone(), encoding);

            if let Some(cmap) = maybe_cmap {
                cmaps.insert(font_name.to_string(), cmap.clone());
                cmaps.insert(clean.clone(), cmap);
            } else if let Some(embedded) = resolve_embedded_font(doc, &f_dict) {
                let embedded_cmap = embedded.to_cmap();
                cmaps.insert(font_name.to_string(), embedded_cmap.clone());
                cmaps.insert(clean.clone(), embedded_cmap);
            }

            let first_char = match f_dict.get("FirstChar") {
                Some(Object::Integer(i)) => *i as u32,
                _ => 0,
            };
            if let Some(widths_obj) = f_dict.get("Widths") {
                let widths_arr = match widths_obj {
                    Object::Array(arr) => Some(Cow::Borrowed(arr)),
                    Object::Reference { id, .. } => match doc.get_object(*id) {
                        Ok(Some(Object::Array(arr))) => Some(Cow::Owned(arr)),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(arr) = widths_arr {
                    let mut char_widths = HashMap::new();
                    for (idx, w_obj) in arr.iter().enumerate() {
                        let w = match w_obj {
                            Object::Integer(i) => *i as f32,
                            Object::Real(r) => *r as f32,
                            _ => 0.0,
                        };
                        char_widths.insert(first_char + idx as u32, w);
                    }
                    widths.insert(font_name.to_string(), char_widths.clone());
                    widths.insert(clean.clone(), char_widths);
                }
            }

            let desc_dict = match f_dict.get("FontDescriptor") {
                Some(Object::Dictionary(d)) => Some(Cow::Borrowed(d)),
                Some(Object::Reference { id, .. }) => match doc.get_object(*id) {
                    Ok(Some(Object::Dictionary(d))) => Some(Cow::Owned(d)),
                    _ => None,
                },
                _ => None,
            };

            if let Some(desc) = desc_dict {
                let mut info = FontInfo::default();
                if let Some(Object::Integer(flags)) = desc.get("Flags") {
                    if flags & (1 << 6) != 0 {
                        info.is_italic = true;
                    }
                }
                if let Some(Object::Integer(weight)) = desc.get("FontWeight") {
                    if *weight >= 700 {
                        info.is_bold = true;
                    }
                }
                if let Some(mw) = desc.get("MissingWidth") {
                    let missing = match mw {
                        Object::Integer(i) => *i as f32,
                        Object::Real(r) => *r as f32,
                        _ => 500.0,
                    };
                    missing_widths.insert(font_name.to_string(), missing);
                    missing_widths.insert(clean.clone(), missing);
                }
                font_info.insert(font_name.to_string(), info);
                font_info.insert(clean, info);
            }
        }
    }

    ExtendedFontResources {
        encodings,
        cmaps,
        widths,
        missing_widths,
        font_info,
    }
}

/// Reads the page dictionary, decompresses `/Contents`, parses operations,
/// tracks the transformation matrix and graphics state, and extracts positioned text spans (§1.4).
pub fn extract_page_spans(doc: &Document<'_>, page_id: u32) -> Result<Vec<TextSpan<'static>>> {
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
        None => return Ok(Vec::new()), // Empty page with no contents
    };

    let decompressed_content = resolve_stream_bytes(doc, contents_obj)?;
    if decompressed_content.is_empty() {
        return Ok(Vec::new());
    }

    let font_res = collect_extended_font_resources(doc, page_dict);

    let mut parser = ContentParser::new(&decompressed_content);
    let operations = parser.parse()?;

    let mut tracker = GraphicsStateTracker::new()
        .with_font_encodings(font_res.encodings)
        .with_font_cmaps(font_res.cmaps)
        .with_font_widths(font_res.widths)
        .with_font_missing_widths(font_res.missing_widths)
        .with_font_info(font_res.font_info);

    let spans = tracker.process_operations(&operations);
    Ok(spans)
}

impl<'a> Document<'a> {
    /// Extracts spatial text spans with device-space bounding boxes and transformation matrices from a page (§1.4).
    pub fn extract_spans(&self, page_id: u32) -> Result<Vec<TextSpan<'static>>> {
        extract_page_spans(self, page_id)
    }

    /// Extracts spatial text spans for all pages in document order (§1.4).
    pub fn extract_spans_all(&self) -> Result<Vec<Vec<TextSpan<'static>>>> {
        let page_ids = self.get_page_ids()?;
        let mut pages = Vec::with_capacity(page_ids.len());
        for id in page_ids {
            pages.push(self.extract_spans(id)?);
        }
        Ok(pages)
    }
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

    #[test]
    fn test_synthetic_pdf_to_unicode_cmap_cjk_and_remapped() {
        let cmap_stream = br#"
            /CIDInit /ProcSet findresource begin
            12 dict begin
            begincmap
            /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def
            /CMapName /Custom-ToUnicode def
            /CMapType 2 def
            1 begincodespacerange
            <0000> <FFFF>
            endcodespacerange
            2 beginbfchar
            <0001> <4E2D>
            <0002> <6587>
            endbfchar
            1 beginbfrange
            <0003> <0005> <0041>
            endbfrange
            endcmap
            CMapName currentdict /CMap defineresource pop
            end
            end
        "#;

        let content_stream = b"BT /F1 12 Tf 100 700 Td <00010002000300040005> Tj ET";

        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.4\n");
        // 1: Catalog
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
        // 2: Pages root
        pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
        // 3: Page with font resource referencing font 4
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>\nendobj\n",
        );
        // 4: Font dictionary referencing ToUnicode stream 6
        pdf.extend_from_slice(
            b"4 0 obj\n<< /Type /Font /Subtype /Type0 /BaseFont /CustomCJK /Encoding /Identity-H /ToUnicode 6 0 R >>\nendobj\n",
        );
        // 5: Contents stream
        pdf.extend_from_slice(
            format!("5 0 obj\n<< /Length {} >>\nstream\n", content_stream.len()).as_bytes(),
        );
        pdf.extend_from_slice(content_stream);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");
        // 6: ToUnicode CMap stream
        pdf.extend_from_slice(
            format!("6 0 obj\n<< /Length {} >>\nstream\n", cmap_stream.len()).as_bytes(),
        );
        pdf.extend_from_slice(cmap_stream);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        pdf.extend_from_slice(b"trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n0\n%%EOF");

        let doc = Document::load(&pdf).expect("Document load failed");
        let text = doc.extract_text(3).expect("Extract page text failed");
        // <0001> -> U+4E2D ('中')
        // <0002> -> U+6587 ('文')
        // <0003> -> U+0041 ('A')
        // <0004> -> U+0042 ('B')
        // <0005> -> U+0043 ('C')
        assert_eq!(text, "中文ABC");
    }

    #[test]
    fn test_synthetic_pdf_to_unicode_cmap_ligatures_and_tj() {
        let cmap_stream = br#"
            1 begincodespacerange
            <0000> <FFFF>
            endcodespacerange
            2 beginbfchar
            <0001> <00660069>
            <0002> <0066006C>
            endbfchar
            1 beginbfrange
            <0010> <0012> [ <0048> <0049> <0021> ]
            endbfrange
        "#;

        let content_stream = b"BT /F1 12 Tf [<0001> -250 <0002> -250 <001000110012>] TJ ET";

        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.4\n");
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
        pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>\nendobj\n",
        );
        pdf.extend_from_slice(
            b"4 0 obj\n<< /Type /Font /Subtype /Type0 /BaseFont /CustomLigatures /Encoding /Identity-H /ToUnicode 6 0 R >>\nendobj\n",
        );
        pdf.extend_from_slice(
            format!("5 0 obj\n<< /Length {} >>\nstream\n", content_stream.len()).as_bytes(),
        );
        pdf.extend_from_slice(content_stream);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");
        pdf.extend_from_slice(
            format!("6 0 obj\n<< /Length {} >>\nstream\n", cmap_stream.len()).as_bytes(),
        );
        pdf.extend_from_slice(cmap_stream);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");
        pdf.extend_from_slice(b"trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n0\n%%EOF");

        let doc = Document::load(&pdf).expect("Document load failed");
        let text = doc.extract_text(3).expect("Extract page text failed");
        assert_eq!(text, "fi fl HI!");
    }

    #[test]
    fn test_synthetic_pdf_extract_spans_with_ctm_and_matrices() {
        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.4\n");
        // 1: Catalog
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
        // 2: Pages root with 2 pages
        pdf.extend_from_slice(
            b"2 0 obj\n<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>\nendobj\n",
        );
        // 3: Page 1 with CTM scaling + translation
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /Contents 5 0 R >>\nendobj\n",
        );
        // 4: Page 2 with bold font TJ kerning
        pdf.extend_from_slice(
            b"4 0 obj\n<< /Type /Page /Parent 2 0 R /Contents 6 0 R >>\nendobj\n",
        );

        // Stream 5: CTM + multiline text
        let s5 = b"q 2 0 0 2 50 100 cm BT /Helvetica 12 Tf 14 TL 10 20 Td (First Span) Tj T* (Second Span) Tj ET Q";
        pdf.extend_from_slice(format!("5 0 obj\n<< /Length {} >>\nstream\n", s5.len()).as_bytes());
        pdf.extend_from_slice(s5);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        // Stream 6: Bold font TJ kerning
        let s6 = b"BT /Helvetica-Bold 16 Tf 0 500 Td [(Hello) -250 (World)] TJ ET";
        pdf.extend_from_slice(format!("6 0 obj\n<< /Length {} >>\nstream\n", s6.len()).as_bytes());
        pdf.extend_from_slice(s6);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        pdf.extend_from_slice(b"trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n0\n%%EOF");

        let doc = Document::load(&pdf).expect("Document load failed");

        // Test single page span extraction on page 1 (object 3)
        let page1_spans = doc.extract_spans(3).expect("Extract spans page 1 failed");
        assert_eq!(page1_spans.len(), 2);

        let s1 = &page1_spans[0];
        assert_eq!(s1.text, "First Span");
        assert_eq!(s1.font_name, "Helvetica");
        assert_eq!(s1.font_size, 12.0);
        assert!(!s1.is_bold);
        assert!(!s1.is_italic);
        // CTM = [2 0 0 2 50 100], Tm = [1 0 0 1 10 20]
        // Tdevice = Tm * CTM => a=2, d=2, e=70, f=140
        assert_eq!(s1.transform.a, 2.0);
        assert_eq!(s1.transform.d, 2.0);
        assert_eq!(s1.transform.e, 70.0);
        assert_eq!(s1.transform.f, 140.0);
        assert_eq!(s1.bbox.min_x, 70.0);
        assert_eq!(s1.bbox.min_y, 140.0);

        let s2 = &page1_spans[1];
        assert_eq!(s2.text, "Second Span");
        // T* moves y by -14 leading in text space: 20 - 14 = 6
        // in device space: 6 * 2 + 100 = 112
        assert_eq!(s2.transform.e, 70.0);
        assert_eq!(s2.transform.f, 112.0);
        assert_eq!(s2.bbox.min_x, 70.0);
        assert_eq!(s2.bbox.min_y, 112.0);

        // Test page 2 (object 4)
        let page2_spans = doc.extract_spans(4).expect("Extract spans page 2 failed");
        assert_eq!(page2_spans.len(), 1);
        assert_eq!(page2_spans[0].text, "Hello World");
        assert_eq!(page2_spans[0].font_name, "Helvetica-Bold");
        assert_eq!(page2_spans[0].font_size, 16.0);
        assert!(page2_spans[0].is_bold);
        assert!(!page2_spans[0].is_italic);

        // Test extract_spans_all
        let all_spans = doc.extract_spans_all().expect("Extract all spans failed");
        assert_eq!(all_spans.len(), 2);
        assert_eq!(all_spans[0].len(), 2);
        assert_eq!(all_spans[1].len(), 1);
    }

    #[test]
    fn test_synthetic_pdf_embedded_truetype_font_introspection() {
        // Construct a synthetic TrueType font with post Version 2.0:
        // GID 1 -> "fi" ('ﬁ')
        // GID 2 -> "ampersand" ('&')
        let mut post = vec![0u8; 32];
        post[0..4].copy_from_slice(&0x00020000u32.to_be_bytes()); // Version 2.0
        post.extend_from_slice(&3u16.to_be_bytes()); // numGlyphs = 3
        post.extend_from_slice(&0u16.to_be_bytes()); // GID 0: .notdef (idx 0)
        post.extend_from_slice(&258u16.to_be_bytes()); // GID 1: custom string 0 -> "fi"
        post.extend_from_slice(&259u16.to_be_bytes()); // GID 2: custom string 1 -> "ampersand"
        // Pascal strings:
        post.push(2);
        post.extend_from_slice(b"fi");
        post.push(9);
        post.extend_from_slice(b"ampersand");

        // Build SFNT font
        let mut ttf = Vec::new();
        ttf.extend_from_slice(&0x00010000u32.to_be_bytes()); // sfntVersion
        ttf.extend_from_slice(&1u16.to_be_bytes()); // numTables = 1
        ttf.extend_from_slice(&0u16.to_be_bytes());
        ttf.extend_from_slice(&0u16.to_be_bytes());
        ttf.extend_from_slice(&0u16.to_be_bytes());

        let table_offset = 12 + 16;
        ttf.extend_from_slice(b"post");
        ttf.extend_from_slice(&0u32.to_be_bytes()); // checksum
        ttf.extend_from_slice(&(table_offset as u32).to_be_bytes());
        ttf.extend_from_slice(&(post.len() as u32).to_be_bytes());
        ttf.extend_from_slice(&post);

        // Content stream displaying GID 1 and GID 2 (<00010002>)
        let content_stream = b"BT /F1 12 Tf <00010002> Tj ET";

        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.4\n");
        // 1: Catalog
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
        // 2: Pages root
        pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
        // 3: Page with font resource referencing font 4
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>\nendobj\n",
        );
        // 4: Font dictionary without ToUnicode, referencing FontDescriptor 6
        pdf.extend_from_slice(
            b"4 0 obj\n<< /Type /Font /Subtype /TrueType /BaseFont /CustomTTF /FontDescriptor 6 0 R >>\nendobj\n",
        );
        // 5: Contents stream
        pdf.extend_from_slice(
            format!("5 0 obj\n<< /Length {} >>\nstream\n", content_stream.len()).as_bytes(),
        );
        pdf.extend_from_slice(content_stream);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");
        // 6: FontDescriptor referencing FontFile2 stream 7
        pdf.extend_from_slice(
            b"6 0 obj\n<< /Type /FontDescriptor /FontName /CustomTTF /FontFile2 7 0 R >>\nendobj\n",
        );
        // 7: FontFile2 stream
        pdf.extend_from_slice(
            format!("7 0 obj\n<< /Length {} >>\nstream\n", ttf.len()).as_bytes(),
        );
        pdf.extend_from_slice(&ttf);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        pdf.extend_from_slice(b"trailer\n<< /Size 8 /Root 1 0 R >>\nstartxref\n0\n%%EOF");

        let doc = Document::load(&pdf).expect("Document load failed");
        let text = doc.extract_text(3).expect("Extract page text failed");
        assert_eq!(text, "ﬁ&");
    }

    #[test]
    fn test_synthetic_pdf_embedded_cff_font_introspection() {
        // Construct a synthetic CFF font with:
        // GID 1 -> Standard SID 34 ('A')
        // GID 2 -> Custom String 0 (SID 391) -> "uni0042" ('B')
        let mut cff = Vec::new();
        // Header
        cff.extend_from_slice(&[1, 0, 4, 1]);
        // Name INDEX
        cff.extend_from_slice(&1u16.to_be_bytes()); // count = 1
        cff.push(1); // offSize = 1
        cff.push(1); // off 0
        cff.push(1 + 7); // off 1
        cff.extend_from_slice(b"CFFTest");

        // Top DICT INDEX placeholder (30 bytes)
        let dummy_len = 30;
        cff.extend_from_slice(&1u16.to_be_bytes());
        cff.push(1);
        cff.push(1);
        cff.push(1 + dummy_len as u8);
        let top_dict_data_start = cff.len();
        cff.extend_from_slice(&vec![0u8; dummy_len]);

        // String INDEX: 1 custom string "uni0042"
        let custom_str = b"uni0042";
        cff.extend_from_slice(&1u16.to_be_bytes()); // count = 1
        cff.push(2); // offSize = 2
        cff.extend_from_slice(&1u16.to_be_bytes()); // off 0 = 1
        cff.extend_from_slice(&(1 + custom_str.len() as u16).to_be_bytes()); // off 1
        cff.extend_from_slice(custom_str);

        // Charset (Format 0): GID 1 -> SID 34 ('A'), GID 2 -> SID 391 ('B')
        let charset_pos = cff.len();
        cff.push(0); // format 0
        cff.extend_from_slice(&34u16.to_be_bytes()); // GID 1
        cff.extend_from_slice(&391u16.to_be_bytes()); // GID 2

        // CharStrings INDEX: 3 glyphs (0, 1, 2)
        let charstrings_pos = cff.len();
        cff.extend_from_slice(&3u16.to_be_bytes()); // count = 3
        cff.push(1);
        cff.push(1);
        cff.push(2);
        cff.push(3);
        cff.push(4);
        cff.extend_from_slice(&[14, 14, 14]); // endchar

        // Top DICT: charset op 15, CharStrings op 17
        let mut dict_bytes = Vec::new();
        dict_bytes.push(29);
        dict_bytes.extend_from_slice(&(charset_pos as i32).to_be_bytes());
        dict_bytes.push(15);
        dict_bytes.push(29);
        dict_bytes.extend_from_slice(&(charstrings_pos as i32).to_be_bytes());
        dict_bytes.push(17);
        while dict_bytes.len() < dummy_len {
            dict_bytes.push(22);
        }
        cff[top_dict_data_start..top_dict_data_start + dummy_len].copy_from_slice(&dict_bytes);

        // Content stream: <00010002>
        let content_stream = b"BT /F1 12 Tf <00010002> Tj ET";

        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.4\n");
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
        pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>\nendobj\n",
        );
        pdf.extend_from_slice(
            b"4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /CFFTest /FontDescriptor 6 0 R >>\nendobj\n",
        );
        pdf.extend_from_slice(
            format!("5 0 obj\n<< /Length {} >>\nstream\n", content_stream.len()).as_bytes(),
        );
        pdf.extend_from_slice(content_stream);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");
        pdf.extend_from_slice(
            b"6 0 obj\n<< /Type /FontDescriptor /FontName /CFFTest /FontFile3 7 0 R >>\nendobj\n",
        );
        pdf.extend_from_slice(
            format!("7 0 obj\n<< /Length {} >>\nstream\n", cff.len()).as_bytes(),
        );
        pdf.extend_from_slice(&cff);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        pdf.extend_from_slice(b"trailer\n<< /Size 8 /Root 1 0 R >>\nstartxref\n0\n%%EOF");

        let doc = Document::load(&pdf).expect("Document load failed");
        let text = doc.extract_text(3).expect("Extract page text failed");
        assert_eq!(text, "AB");
    }
}
