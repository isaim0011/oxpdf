use crate::error::Result;
use crate::text::{decode_winansi_char, FontEncoding};
use std::collections::HashMap;

/// A codespace range defined in `begincodespacerange ... endcodespacerange`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeSpaceRange {
    pub byte_len: usize,
    pub low: u32,
    pub high: u32,
}

/// Parsed CMap table mapping character codes to Unicode characters or strings.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CMap {
    mappings: HashMap<u32, String>,
    codespace_ranges: Vec<CodeSpaceRange>,
    has_2byte_codes: bool,
}

impl CMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Looks up the Unicode string mapped to the given character code.
    /// Supports both single Unicode scalar values and multi-codepoint ligatures.
    #[inline]
    pub fn lookup(&self, code: u32) -> Option<&str> {
        self.mappings.get(&code).map(|s| s.as_str())
    }

    /// Looks up the character code and returns a single `char` if the mapping contains
    /// exactly one Unicode scalar value.
    #[inline]
    pub fn lookup_char(&self, code: u32) -> Option<char> {
        let s = self.lookup(code)?;
        let mut chars = s.chars();
        let first = chars.next()?;
        if chars.next().is_none() {
            Some(first)
        } else {
            None
        }
    }

    /// Decodes a byte sequence into a Unicode `String` using the CMap mappings,
    /// falling back to standard WinAnsi encoding for unmapped characters.
    pub fn decode_string(&self, bytes: &[u8]) -> String {
        self.decode_string_with_fallback(bytes, FontEncoding::WinAnsiEncoding)
    }

    /// Decodes a byte sequence into a Unicode `String` using the CMap mappings,
    /// falling back to the specified font encoding for unmapped characters.
    pub fn decode_string_with_fallback(&self, bytes: &[u8], fallback: FontEncoding) -> String {
        let mut out = String::new();
        let mut i = 0;

        // 1. If explicit codespace ranges are specified:
        if !self.codespace_ranges.is_empty() {
            while i < bytes.len() {
                let mut matched = false;
                for range in &self.codespace_ranges {
                    if i + range.byte_len <= bytes.len() {
                        let code = bytes_to_u32(&bytes[i..i + range.byte_len]);
                        if code >= range.low && code <= range.high {
                            if let Some(s) = self.lookup(code) {
                                out.push_str(s);
                            } else if code <= 0xFF {
                                out.push(decode_fallback_char(code as u8, fallback));
                            } else if let Some(ch) = char::from_u32(code) {
                                out.push(ch);
                            } else {
                                out.push('\u{FFFD}');
                            }
                            i += range.byte_len;
                            matched = true;
                            break;
                        }
                    }
                }
                if !matched {
                    let code = bytes[i] as u32;
                    if let Some(s) = self.lookup(code) {
                        out.push_str(s);
                    } else {
                        out.push(decode_fallback_char(bytes[i], fallback));
                    }
                    i += 1;
                }
            }
            return out;
        }

        // 2. If no codespace ranges, but font has 2-byte codes and even length:
        if self.has_2byte_codes && bytes.len() >= 2 && bytes.len() % 2 == 0 {
            let mut can_decode_2byte = false;
            for chunk in bytes.chunks_exact(2) {
                let code = ((chunk[0] as u32) << 8) | (chunk[1] as u32);
                if self.mappings.contains_key(&code) {
                    can_decode_2byte = true;
                    break;
                }
            }
            if can_decode_2byte {
                for chunk in bytes.chunks_exact(2) {
                    let code = ((chunk[0] as u32) << 8) | (chunk[1] as u32);
                    if let Some(s) = self.lookup(code) {
                        out.push_str(s);
                    } else if code <= 0xFF {
                        out.push(decode_fallback_char(code as u8, fallback));
                    } else if let Some(ch) = char::from_u32(code) {
                        out.push(ch);
                    } else {
                        out.push('\u{FFFD}');
                    }
                }
                return out;
            }
        }

        // 3. Fallback variable-length greedy matching:
        while i < bytes.len() {
            if self.has_2byte_codes && i + 1 < bytes.len() {
                let code2 = ((bytes[i] as u32) << 8) | (bytes[i + 1] as u32);
                if let Some(s) = self.lookup(code2) {
                    out.push_str(s);
                    i += 2;
                    continue;
                }
            }

            let code1 = bytes[i] as u32;
            if let Some(s) = self.lookup(code1) {
                out.push_str(s);
                i += 1;
            } else if self.has_2byte_codes && bytes.len() % 2 == 0 && i + 1 < bytes.len() {
                let code2 = ((bytes[i] as u32) << 8) | (bytes[i + 1] as u32);
                if code2 <= 0xFF {
                    out.push(decode_fallback_char(code2 as u8, fallback));
                } else if let Some(ch) = char::from_u32(code2) {
                    out.push(ch);
                } else {
                    out.push('\u{FFFD}');
                }
                i += 2;
            } else {
                out.push(decode_fallback_char(bytes[i], fallback));
                i += 1;
            }
        }

        out
    }

    #[inline]
    pub fn has_2byte_codes(&self) -> bool {
        self.has_2byte_codes
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.mappings.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    #[inline]
    pub fn insert(&mut self, code: u32, target: String) {
        if code > 0xFF {
            self.has_2byte_codes = true;
        }
        self.mappings.insert(code, target);
    }

    #[inline]
    pub fn set_has_2byte_codes(&mut self, val: bool) {
        self.has_2byte_codes = val;
    }

    /// Parses a CMap stream into a `CMap` instance.
    pub fn parse(stream_bytes: &[u8]) -> Result<Self> {
        let mut cmap = CMap::default();
        let mut lexer = CMapLexer::new(stream_bytes);

        while let Some(tok) = lexer.next_token() {
            match tok {
                CMapToken::Keyword("beginbfchar") => {
                    while let Some(tok) = lexer.next_token() {
                        if let CMapToken::Keyword("endbfchar") = tok {
                            break;
                        }
                        let (src_code, byte_len) = match token_to_code(&tok) {
                            Some(c) => c,
                            None => continue,
                        };
                        if byte_len >= 2 || src_code > 0xFF {
                            cmap.has_2byte_codes = true;
                        }
                        let dst_tok = match lexer.next_token() {
                            Some(t) => t,
                            None => break,
                        };
                        let dst_str = token_to_dest_str(&dst_tok);
                        cmap.mappings.insert(src_code, dst_str);
                    }
                }
                CMapToken::Keyword("beginbfrange") => {
                    while let Some(tok) = lexer.next_token() {
                        if let CMapToken::Keyword("endbfrange") = tok {
                            break;
                        }
                        let (start_code, byte_len1) = match token_to_code(&tok) {
                            Some(c) => c,
                            None => continue,
                        };
                        let end_tok = match lexer.next_token() {
                            Some(t) => t,
                            None => break,
                        };
                        let (end_code, byte_len2) = match token_to_code(&end_tok) {
                            Some(c) => c,
                            None => continue,
                        };
                        if byte_len1 >= 2 || byte_len2 >= 2 || start_code > 0xFF || end_code > 0xFF
                        {
                            cmap.has_2byte_codes = true;
                        }
                        if end_code < start_code {
                            continue;
                        }
                        let range_len = (end_code - start_code).min(65535);

                        let dst_tok = match lexer.next_token() {
                            Some(t) => t,
                            None => break,
                        };

                        match dst_tok {
                            CMapToken::ArrayOpen => {
                                let mut offset = 0;
                                while let Some(elem_tok) = lexer.next_token() {
                                    if let CMapToken::ArrayClose = elem_tok {
                                        break;
                                    }
                                    if offset <= range_len {
                                        let code = start_code + offset;
                                        let dst_str = token_to_dest_str(&elem_tok);
                                        cmap.mappings.insert(code, dst_str);
                                        offset += 1;
                                    }
                                }
                            }
                            _ => match &dst_tok {
                                CMapToken::HexString(dst_bytes, _)
                                | CMapToken::String(dst_bytes) => {
                                    for k in 0..=range_len {
                                        let code = start_code + k;
                                        let inc = increment_dst_bytes(dst_bytes, k);
                                        let dst_str = decode_unicode_dest(&inc);
                                        cmap.mappings.insert(code, dst_str);
                                    }
                                }
                                CMapToken::Integer(dst_val) if *dst_val >= 0 => {
                                    let base = *dst_val as u32;
                                    for k in 0..=range_len {
                                        let code = start_code + k;
                                        let cur = base.wrapping_add(k);
                                        let dst_str = char::from_u32(cur)
                                            .map(|c| c.to_string())
                                            .unwrap_or_else(|| "\u{FFFD}".to_string());
                                        cmap.mappings.insert(code, dst_str);
                                    }
                                }
                                _ => {}
                            },
                        }
                    }
                }
                CMapToken::Keyword("begincodespacerange") => {
                    while let Some(tok) = lexer.next_token() {
                        if let CMapToken::Keyword("endcodespacerange") = tok {
                            break;
                        }
                        let (low, byte_len1) = match token_to_code(&tok) {
                            Some(c) => c,
                            None => continue,
                        };
                        let high_tok = match lexer.next_token() {
                            Some(t) => t,
                            None => break,
                        };
                        let (high, byte_len2) = match token_to_code(&high_tok) {
                            Some(c) => c,
                            None => continue,
                        };
                        let byte_len = byte_len1.max(byte_len2);
                        if byte_len >= 2 {
                            cmap.has_2byte_codes = true;
                        }
                        cmap.codespace_ranges.push(CodeSpaceRange {
                            byte_len,
                            low,
                            high,
                        });
                    }
                }
                _ => {}
            }
        }

        Ok(cmap)
    }
}

#[inline]
pub(crate) fn decode_fallback_char(b: u8, encoding: FontEncoding) -> char {
    match encoding {
        FontEncoding::WinAnsiEncoding => decode_winansi_char(b),
        FontEncoding::StandardEncoding => crate::text::decode_standard_char(b),
        FontEncoding::MacRomanEncoding => crate::text::decode_macroman_char(b),
        FontEncoding::PdfDocEncoding => crate::text::decode_pdfdoc_char(b),
        FontEncoding::Ascii => {
            if b <= 0x7F {
                b as char
            } else {
                '?'
            }
        }
    }
}

#[inline]
pub(crate) fn bytes_to_u32(bytes: &[u8]) -> u32 {
    let mut val = 0u32;
    for &b in bytes {
        val = (val << 8) | (b as u32);
    }
    val
}

#[inline]
pub(crate) fn increment_dst_bytes(base: &[u8], offset: u32) -> Vec<u8> {
    if base.is_empty() {
        return Vec::new();
    }
    let mut result = base.to_vec();
    let mut carry = offset;
    for b in result.iter_mut().rev() {
        let sum = (*b as u32) + (carry & 0xFF);
        *b = (sum & 0xFF) as u8;
        carry = (carry >> 8) + (sum >> 8);
        if carry == 0 {
            break;
        }
    }
    result
}

/// Decodes Unicode destination bytes (supporting UTF-16BE with surrogate pairs, UTF-8, and 1-byte fallback).
pub fn decode_unicode_dest(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    // Single byte destination: map to ASCII / Latin1
    if bytes.len() == 1 {
        return (bytes[0] as char).to_string();
    }
    // Even byte length: try UTF-16BE
    if bytes.len() % 2 == 0 {
        let u16_units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect();
        let res: std::result::Result<String, _> =
            char::decode_utf16(u16_units.iter().copied()).collect();
        if let Ok(s) = res {
            return s;
        }
    }
    // Try UTF-8
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    // Fall back to Latin1
    bytes.iter().map(|&b| b as char).collect()
}

#[derive(Debug, Clone, PartialEq)]
enum CMapToken<'a> {
    Keyword(&'a str),
    Integer(i64),
    HexString(Vec<u8>, usize),
    String(Vec<u8>),
    ArrayOpen,
    ArrayClose,
    Other,
}

struct CMapLexer<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> CMapLexer<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    #[inline]
    fn is_whitespace(b: u8) -> bool {
        matches!(b, 0x00 | 0x09 | 0x0A | 0x0C | 0x0D | 0x20)
    }

    #[inline]
    fn is_delimiter_or_ws(b: u8) -> bool {
        Self::is_whitespace(b)
            || matches!(
                b,
                b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
            )
    }

    fn skip_whitespace_and_comments(&mut self) {
        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            if Self::is_whitespace(b) {
                self.pos += 1;
            } else if b == b'%' {
                self.pos += 1;
                while self.pos < self.data.len() {
                    let c = self.data[self.pos];
                    self.pos += 1;
                    if c == b'\r' || c == b'\n' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    fn next_token(&mut self) -> Option<CMapToken<'a>> {
        self.skip_whitespace_and_comments();
        if self.pos >= self.data.len() {
            return None;
        }

        let b = self.data[self.pos];

        // 2-character delimiters: << and >>
        if b == b'<' && self.pos + 1 < self.data.len() && self.data[self.pos + 1] == b'<' {
            self.pos += 2;
            return Some(CMapToken::Other);
        }
        if b == b'>' && self.pos + 1 < self.data.len() && self.data[self.pos + 1] == b'>' {
            self.pos += 2;
            return Some(CMapToken::Other);
        }

        match b {
            b'[' => {
                self.pos += 1;
                Some(CMapToken::ArrayOpen)
            }
            b']' => {
                self.pos += 1;
                Some(CMapToken::ArrayClose)
            }
            b'<' => self.read_hex_string(),
            b'(' => self.read_literal_string(),
            b'/' => {
                self.pos += 1;
                while self.pos < self.data.len() && !Self::is_delimiter_or_ws(self.data[self.pos]) {
                    self.pos += 1;
                }
                Some(CMapToken::Other)
            }
            b'0'..=b'9' | b'+' | b'-' => self.read_number_or_keyword(),
            _ => self.read_keyword_or_other(),
        }
    }

    fn read_hex_string(&mut self) -> Option<CMapToken<'a>> {
        self.pos += 1; // skip '<'
        let mut out = Vec::new();
        let mut high_nibble: Option<u8> = None;
        let mut digits = 0;

        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            self.pos += 1;
            if b == b'>' {
                if let Some(h) = high_nibble {
                    out.push(h << 4);
                }
                return Some(CMapToken::HexString(out, digits));
            }
            if Self::is_whitespace(b) {
                continue;
            }
            let nibble = match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                b'A'..=b'F' => b - b'A' + 10,
                _ => continue,
            };
            digits += 1;
            match high_nibble {
                None => high_nibble = Some(nibble),
                Some(h) => {
                    out.push((h << 4) | nibble);
                    high_nibble = None;
                }
            }
        }
        if let Some(h) = high_nibble {
            out.push(h << 4);
        }
        Some(CMapToken::HexString(out, digits))
    }

    fn read_literal_string(&mut self) -> Option<CMapToken<'a>> {
        self.pos += 1; // skip '('
        let mut out = Vec::new();
        let mut depth = 1;

        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            self.pos += 1;
            if b == b'(' {
                depth += 1;
                out.push(b);
            } else if b == b')' {
                depth -= 1;
                if depth == 0 {
                    return Some(CMapToken::String(out));
                }
                out.push(b);
            } else if b == b'\\' && self.pos < self.data.len() {
                let esc = self.data[self.pos];
                self.pos += 1;
                match esc {
                    b'n' => out.push(b'\n'),
                    b'r' => out.push(b'\r'),
                    b't' => out.push(b'\t'),
                    b'b' => out.push(0x08),
                    b'f' => out.push(0x0C),
                    b'(' => out.push(b'('),
                    b')' => out.push(b')'),
                    b'\\' => out.push(b'\\'),
                    b'0'..=b'7' => {
                        let mut oct = (esc - b'0') as u16;
                        for _ in 0..2 {
                            if self.pos < self.data.len()
                                && (b'0'..=b'7').contains(&self.data[self.pos])
                            {
                                oct = (oct << 3) + ((self.data[self.pos] - b'0') as u16);
                                self.pos += 1;
                            } else {
                                break;
                            }
                        }
                        out.push((oct & 0xFF) as u8);
                    }
                    other => out.push(other),
                }
            } else {
                out.push(b);
            }
        }
        Some(CMapToken::String(out))
    }

    fn read_number_or_keyword(&mut self) -> Option<CMapToken<'a>> {
        let start = self.pos;
        while self.pos < self.data.len() && !Self::is_delimiter_or_ws(self.data[self.pos]) {
            self.pos += 1;
        }
        if self.pos == start {
            self.pos += 1;
            return Some(CMapToken::Other);
        }
        let slice = &self.data[start..self.pos];
        if let Ok(s) = std::str::from_utf8(slice) {
            if let Ok(n) = s.parse::<i64>() {
                return Some(CMapToken::Integer(n));
            }
            return Some(CMapToken::Keyword(s));
        }
        Some(CMapToken::Other)
    }

    fn read_keyword_or_other(&mut self) -> Option<CMapToken<'a>> {
        let start = self.pos;
        while self.pos < self.data.len() && !Self::is_delimiter_or_ws(self.data[self.pos]) {
            self.pos += 1;
        }
        if self.pos == start {
            self.pos += 1;
            return Some(CMapToken::Other);
        }
        let slice = &self.data[start..self.pos];
        if let Ok(s) = std::str::from_utf8(slice) {
            Some(CMapToken::Keyword(s))
        } else {
            Some(CMapToken::Other)
        }
    }
}

fn token_to_code(tok: &CMapToken<'_>) -> Option<(u32, usize)> {
    match tok {
        CMapToken::HexString(bytes, digits) => {
            let byte_len = if *digits <= 2 {
                1
            } else if *digits <= 4 {
                2
            } else {
                bytes.len()
            };
            Some((bytes_to_u32(bytes), byte_len))
        }
        CMapToken::Integer(val) if *val >= 0 => {
            let code = *val as u32;
            let byte_len = if code > 0xFF { 2 } else { 1 };
            Some((code, byte_len))
        }
        _ => None,
    }
}

fn token_to_dest_str(tok: &CMapToken<'_>) -> String {
    match tok {
        CMapToken::HexString(bytes, _) => decode_unicode_dest(bytes),
        CMapToken::String(bytes) => decode_unicode_dest(bytes),
        CMapToken::Integer(val) if *val >= 0 => char::from_u32(*val as u32)
            .map(|c| c.to_string())
            .unwrap_or_else(|| "\u{FFFD}".to_string()),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmap_beginbfchar_single_mapping() {
        let stream = br#"
            /CIDInit /ProcSet findresource begin
            12 dict begin
            begincmap
            /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def
            /CMapName /Adobe-Identity-UCS def
            /CMapType 2 def
            3 beginbfchar
            <0001> <0048>
            <0002> <00660069>
            <0003> <D83DDE00>
            endbfchar
            endcmap
            CMapName currentdict /CMap defineresource pop
            end
            end
        "#;

        let cmap = CMap::parse(stream).expect("Failed to parse CMap");
        assert_eq!(cmap.lookup(0x0001), Some("H"));
        assert_eq!(cmap.lookup_char(0x0001), Some('H'));

        // Multi-codepoint ligature "fi"
        assert_eq!(cmap.lookup(0x0002), Some("fi"));
        assert_eq!(cmap.lookup_char(0x0002), None);

        // Surrogate pair 😀 (U+1F600)
        assert_eq!(cmap.lookup(0x0003), Some("😀"));
        assert_eq!(cmap.lookup_char(0x0003), Some('😀'));

        // Unmapped
        assert_eq!(cmap.lookup(0x0099), None);
    }

    #[test]
    fn test_cmap_beginbfrange_contiguous_mapping() {
        let stream = br#"
            1 beginbfchar
            <0001> <0048>
            endbfchar
            1 beginbfrange
            <0002> <0004> <0049>
            endbfrange
        "#;

        let cmap = CMap::parse(stream).expect("Failed to parse CMap");
        assert_eq!(cmap.lookup(0x0001), Some("H"));
        assert_eq!(cmap.lookup(0x0002), Some("I"));
        assert_eq!(cmap.lookup(0x0003), Some("J"));
        assert_eq!(cmap.lookup(0x0004), Some("K"));
        assert_eq!(cmap.lookup(0x0005), None);

        // Decode 2-byte string
        let raw_bytes = [0x00, 0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x04];
        let decoded = cmap.decode_string(&raw_bytes);
        assert_eq!(decoded, "HIJK");
    }

    #[test]
    fn test_cmap_beginbfrange_destination_array() {
        let stream = br#"
            1 beginbfrange
            <0006> <0008> [ <004E> <004F> <0050> ]
            endbfrange
            1 beginbfrange
            <0010> <0011> [ <00660069> (xyz) ]
            endbfrange
        "#;

        let cmap = CMap::parse(stream).expect("Failed to parse CMap");
        assert_eq!(cmap.lookup(0x0006), Some("N"));
        assert_eq!(cmap.lookup(0x0007), Some("O"));
        assert_eq!(cmap.lookup(0x0008), Some("P"));
        assert_eq!(cmap.lookup(0x0009), None);

        // Array with ligature and literal string
        assert_eq!(cmap.lookup(0x0010), Some("fi"));
        assert_eq!(cmap.lookup(0x0011), Some("xyz"));
    }

    #[test]
    fn test_cmap_codespace_range_and_mixed_encodings() {
        let stream = br#"
            2 begincodespacerange
            <00> <80>
            <8140> <9FFC>
            endcodespacerange
            1 beginbfchar
            <20> <0020>
            endbfchar
            1 beginbfchar
            <8140> <3000>
            endbfchar
        "#;

        let cmap = CMap::parse(stream).expect("Failed to parse CMap");
        assert_eq!(cmap.lookup(0x20), Some(" "));
        assert_eq!(cmap.lookup(0x8140), Some("\u{3000}"));

        let bytes = [0x20, 0x81, 0x40, 0x20];
        let decoded = cmap.decode_string(&bytes);
        assert_eq!(decoded, " \u{3000} ");
    }
}
