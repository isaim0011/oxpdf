use crate::cmap::CMap;
use crate::error::{Error, Result};
use crate::font::agl::{AdobeGlyphList, MAC_POST_NAMES};
use std::collections::HashMap;

/// Zero-copy parser and introspection engine for TrueType / OpenType (`/FontFile2`) font files.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrueTypeFont {
    gid_to_unicode: HashMap<u16, char>,
    code_to_unicode: HashMap<u32, char>,
}

impl TrueTypeFont {
    /// Parses a raw TrueType font file buffer into a `TrueTypeFont`.
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 12 {
            return Err(Error::UnexpectedEof(data.len()));
        }

        let num_tables = u16::from_be_bytes([data[4], data[5]]) as usize;
        let header_end = 12 + num_tables * 16;
        if data.len() < header_end {
            return Err(Error::UnexpectedEof(data.len()));
        }

        let mut cmap_offset: Option<(usize, usize)> = None;
        let mut post_offset: Option<(usize, usize)> = None;

        for i in 0..num_tables {
            let rec_offset = 12 + i * 16;
            let tag = &data[rec_offset..rec_offset + 4];
            let offset = u32::from_be_bytes([
                data[rec_offset + 8],
                data[rec_offset + 9],
                data[rec_offset + 10],
                data[rec_offset + 11],
            ]) as usize;
            let length = u32::from_be_bytes([
                data[rec_offset + 12],
                data[rec_offset + 13],
                data[rec_offset + 14],
                data[rec_offset + 15],
            ]) as usize;

            if offset.saturating_add(length) > data.len() {
                return Err(Error::SyntaxError {
                    offset,
                    message: "TrueType table record exceeds font data bounds",
                });
            }

            if tag == b"cmap" {
                cmap_offset = Some((offset, length));
            } else if tag == b"post" {
                post_offset = Some((offset, length));
            }
        }

        let mut font = TrueTypeFont::default();

        // 1. Parse cmap table if present
        if let Some((offset, length)) = cmap_offset {
            let cmap_data = &data[offset..offset + length];
            font.parse_cmap(cmap_data);
        }

        // 2. Parse post table if present (can supplement or override glyph names)
        if let Some((offset, length)) = post_offset {
            let post_data = &data[offset..offset + length];
            font.parse_post(post_data);
        }

        Ok(font)
    }

    /// Maps a Glyph ID (GID) to its corresponding Unicode character.
    #[inline]
    pub fn map_glyph_to_unicode(&self, gid: u16) -> Option<char> {
        self.gid_to_unicode.get(&gid).copied()
    }

    /// Maps a character code to its corresponding Unicode character.
    #[inline]
    pub fn map_code_to_unicode(&self, code: u32) -> Option<char> {
        self.code_to_unicode.get(&code).copied()
    }

    /// Returns the underlying mapping from GID to Unicode character.
    #[inline]
    pub fn glyph_to_unicode(&self) -> &HashMap<u16, char> {
        &self.gid_to_unicode
    }

    /// Returns the underlying mapping from character code to Unicode character.
    #[inline]
    pub fn code_to_unicode(&self) -> &HashMap<u32, char> {
        &self.code_to_unicode
    }

    /// Builds a `CMap` representing all resolved glyph and character mappings.
    pub fn to_cmap(&self) -> CMap {
        let mut cmap = CMap::new();
        cmap.set_has_2byte_codes(true);
        // Insert GID mappings (crucial for Type0 / CIDFontType2 Identity-H)
        for (&gid, &ch) in &self.gid_to_unicode {
            cmap.insert(gid as u32, ch.to_string());
        }
        // Insert character code mappings
        for (&code, &ch) in &self.code_to_unicode {
            cmap.insert(code, ch.to_string());
        }
        cmap
    }

    /// Parses the `cmap` table, prioritizing Format 12 (UCS-4) then Format 4 (BMP).
    fn parse_cmap(&mut self, data: &[u8]) {
        if data.len() < 4 {
            return;
        }

        let num_tables = u16::from_be_bytes([data[2], data[3]]) as usize;
        if data.len() < 4 + num_tables * 8 {
            return;
        }

        let mut format12_offset: Option<usize> = None;
        let mut format4_offset: Option<usize> = None;

        for i in 0..num_tables {
            let rec = 4 + i * 8;
            let platform_id = u16::from_be_bytes([data[rec], data[rec + 1]]);
            let encoding_id = u16::from_be_bytes([data[rec + 2], data[rec + 3]]);
            let sub_offset = u32::from_be_bytes([
                data[rec + 4],
                data[rec + 5],
                data[rec + 6],
                data[rec + 7],
            ]) as usize;

            if sub_offset + 2 > data.len() {
                continue;
            }

            let format = u16::from_be_bytes([data[sub_offset], data[sub_offset + 1]]);
            if format == 12 {
                format12_offset = Some(sub_offset);
            } else if format == 4 {
                // Prefer Unicode platform (0) or Windows Unicode (3/1 or 3/10)
                if platform_id == 0
                    || (platform_id == 3 && encoding_id == 1)
                    || format4_offset.is_none()
                {
                    format4_offset = Some(sub_offset);
                }
            }
        }

        // Parse Format 12 if found
        if let Some(offset) = format12_offset {
            self.parse_cmap_format_12(&data[offset..]);
        }

        // Parse Format 4
        if let Some(offset) = format4_offset {
            self.parse_cmap_format_4(&data[offset..]);
        }
    }

    /// Parses a `cmap` Format 12 subtable.
    fn parse_cmap_format_12(&mut self, data: &[u8]) {
        if data.len() < 16 {
            return;
        }
        let num_groups = u32::from_be_bytes([data[12], data[13], data[14], data[15]]) as usize;
        let needed = 16 + num_groups * 12;
        if data.len() < needed {
            return;
        }

        for i in 0..num_groups {
            let grp = 16 + i * 12;
            let start_char = u32::from_be_bytes([
                data[grp],
                data[grp + 1],
                data[grp + 2],
                data[grp + 3],
            ]);
            let end_char = u32::from_be_bytes([
                data[grp + 4],
                data[grp + 5],
                data[grp + 6],
                data[grp + 7],
            ]);
            let start_glyph = u32::from_be_bytes([
                data[grp + 8],
                data[grp + 9],
                data[grp + 10],
                data[grp + 11],
            ]);

            if end_char < start_char {
                continue;
            }

            let count = (end_char - start_char).min(65535);
            for k in 0..=count {
                let code = start_char + k;
                let gid = (start_glyph + k) as u16;
                if let Some(ch) = char::from_u32(code) {
                    self.gid_to_unicode.insert(gid, ch);
                    self.code_to_unicode.insert(code, ch);
                }
            }
        }
    }

    /// Parses a `cmap` Format 4 subtable.
    fn parse_cmap_format_4(&mut self, data: &[u8]) {
        if data.len() < 14 {
            return;
        }

        let seg_count_x2 = u16::from_be_bytes([data[6], data[7]]) as usize;
        let seg_count = seg_count_x2 / 2;
        if seg_count == 0 {
            return;
        }

        let end_code_offset = 14;
        let reserved_pad_offset = end_code_offset + seg_count * 2;
        let start_code_offset = reserved_pad_offset + 2;
        let id_delta_offset = start_code_offset + seg_count * 2;
        let id_range_offset_offset = id_delta_offset + seg_count * 2;

        if data.len() < id_range_offset_offset + seg_count * 2 {
            return;
        }

        for i in 0..seg_count {
            let end_code = u16::from_be_bytes([
                data[end_code_offset + i * 2],
                data[end_code_offset + i * 2 + 1],
            ]);
            let start_code = u16::from_be_bytes([
                data[start_code_offset + i * 2],
                data[start_code_offset + i * 2 + 1],
            ]);
            let id_delta = i16::from_be_bytes([
                data[id_delta_offset + i * 2],
                data[id_delta_offset + i * 2 + 1],
            ]);
            let id_range_offset = u16::from_be_bytes([
                data[id_range_offset_offset + i * 2],
                data[id_range_offset_offset + i * 2 + 1],
            ]);

            if start_code == 0xFFFF && end_code == 0xFFFF {
                continue;
            }
            if end_code < start_code {
                continue;
            }

            for c in start_code..=end_code {
                let gid = if id_range_offset == 0 {
                    ((c as i32 + id_delta as i32) & 0xFFFF) as u16
                } else {
                    let entry_offset = (id_range_offset_offset + i * 2)
                        .checked_add(id_range_offset as usize)
                        .and_then(|o| o.checked_add((c - start_code) as usize * 2));
                    match entry_offset {
                        Some(o) if o + 2 <= data.len() => {
                            let raw_gid = u16::from_be_bytes([data[o], data[o + 1]]);
                            if raw_gid != 0 {
                                ((raw_gid as i32 + id_delta as i32) & 0xFFFF) as u16
                            } else {
                                0
                            }
                        }
                        _ => 0,
                    }
                };

                if gid != 0 {
                    if let Some(ch) = char::from_u32(c as u32) {
                        self.gid_to_unicode.entry(gid).or_insert(ch);
                        self.code_to_unicode.entry(c as u32).or_insert(ch);
                    }
                }
            }
        }
    }

    /// Parses the `post` table (Version 1.0 or Version 2.0).
    fn parse_post(&mut self, data: &[u8]) {
        if data.len() < 32 {
            return;
        }

        let format_type = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);

        if format_type == 0x00010000 {
            // Version 1.0: standard 258 Mac glyph names
            for (gid, &name) in MAC_POST_NAMES.iter().enumerate() {
                if let Some(ch) = AdobeGlyphList::name_to_unicode(name) {
                    self.gid_to_unicode.entry(gid as u16).or_insert(ch);
                }
            }
        } else if format_type == 0x00020000 {
            // Version 2.0: custom glyph names via indices and Pascal strings
            if data.len() < 34 {
                return;
            }
            let num_glyphs = u16::from_be_bytes([data[32], data[33]]) as usize;
            if data.len() < 34 + num_glyphs * 2 {
                return;
            }

            // Extract Pascal strings following glyphNameIndex array
            let mut curr = 34 + num_glyphs * 2;
            let mut custom_strings: Vec<&str> = Vec::new();
            while curr < data.len() {
                let len = data[curr] as usize;
                curr += 1;
                if curr + len > data.len() {
                    break;
                }
                if let Ok(s) = std::str::from_utf8(&data[curr..curr + len]) {
                    custom_strings.push(s);
                } else {
                    custom_strings.push("");
                }
                curr += len;
            }

            for gid in 0..num_glyphs {
                let idx_pos = 34 + gid * 2;
                let name_idx =
                    u16::from_be_bytes([data[idx_pos], data[idx_pos + 1]]) as usize;
                let name: Option<&str> = if name_idx < 258 {
                    MAC_POST_NAMES.get(name_idx).copied()
                } else {
                    custom_strings.get(name_idx - 258).copied()
                };

                if let Some(n) = name {
                    if let Some(ch) = AdobeGlyphList::name_to_unicode(n) {
                        self.gid_to_unicode.insert(gid as u16, ch);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Constructs a minimal valid TrueType binary font containing given tables.
    fn build_synthetic_ttf(tables: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
        let num_tables = tables.len() as u16;
        let mut ttf = Vec::new();

        // SFNT Header (12 bytes)
        ttf.extend_from_slice(&0x00010000u32.to_be_bytes()); // sfntVersion
        ttf.extend_from_slice(&num_tables.to_be_bytes());
        ttf.extend_from_slice(&0u16.to_be_bytes()); // searchRange
        ttf.extend_from_slice(&0u16.to_be_bytes()); // entrySelector
        ttf.extend_from_slice(&0u16.to_be_bytes()); // rangeShift

        let mut offset = 12 + tables.len() * 16;
        for (tag, data) in tables {
            ttf.extend_from_slice(*tag);
            ttf.extend_from_slice(&0u32.to_be_bytes()); // checkSum
            ttf.extend_from_slice(&(offset as u32).to_be_bytes());
            ttf.extend_from_slice(&(data.len() as u32).to_be_bytes());
            offset += data.len();
        }

        for (_, data) in tables {
            ttf.extend_from_slice(data);
        }

        ttf
    }

    #[test]
    fn test_truetype_post_version_1_0() {
        let mut post = vec![0u8; 32];
        post[0..4].copy_from_slice(&0x00010000u32.to_be_bytes()); // Version 1.0

        let ttf_bytes = build_synthetic_ttf(&[(b"post", &post)]);
        let ttf = TrueTypeFont::parse(&ttf_bytes).expect("Failed to parse TTF");

        // GID 3 is 'space', GID 36 is 'A', GID 192 is 'fi'
        assert_eq!(ttf.map_glyph_to_unicode(3), Some(' '));
        assert_eq!(ttf.map_glyph_to_unicode(36), Some('A'));
        assert_eq!(ttf.map_glyph_to_unicode(192), Some('ﬁ'));
        assert_eq!(ttf.map_glyph_to_unicode(193), Some('ﬂ'));
    }

    #[test]
    fn test_truetype_post_version_2_0_custom_pascal_strings() {
        let mut post = vec![0u8; 32];
        post[0..4].copy_from_slice(&0x00020000u32.to_be_bytes()); // Version 2.0

        // numGlyphs = 3
        post.extend_from_slice(&3u16.to_be_bytes());
        // GID 0: .notdef (idx 0)
        // GID 1: custom Pascal string 0 (idx 258) -> "fi"
        // GID 2: custom Pascal string 1 (idx 259) -> "uni0042" (B)
        post.extend_from_slice(&0u16.to_be_bytes());
        post.extend_from_slice(&258u16.to_be_bytes());
        post.extend_from_slice(&259u16.to_be_bytes());

        // Pascal strings:
        // String 0: len 2, "fi"
        post.push(2);
        post.extend_from_slice(b"fi");
        // String 1: len 7, "uni0042"
        post.push(7);
        post.extend_from_slice(b"uni0042");

        let ttf_bytes = build_synthetic_ttf(&[(b"post", &post)]);
        let ttf = TrueTypeFont::parse(&ttf_bytes).expect("Failed to parse TTF");

        assert_eq!(ttf.map_glyph_to_unicode(1), Some('ﬁ'));
        assert_eq!(ttf.map_glyph_to_unicode(2), Some('B'));
        assert_eq!(ttf.map_glyph_to_unicode(0), None);
    }

    #[test]
    fn test_truetype_cmap_format_4() {
        // Build a synthetic cmap table with 1 subtable (Format 4)
        let mut cmap = Vec::new();
        cmap.extend_from_slice(&0u16.to_be_bytes()); // version
        cmap.extend_from_slice(&1u16.to_be_bytes()); // numTables = 1

        // Encoding record: platform 0, encoding 3, subtable offset 12
        cmap.extend_from_slice(&0u16.to_be_bytes());
        cmap.extend_from_slice(&3u16.to_be_bytes());
        cmap.extend_from_slice(&12u32.to_be_bytes());

        // Format 4 Subtable (offset 12)
        // Segments:
        // Seg 0: 'A'..='C' (0x0041..=0x0043) -> GID 10..=12 (idDelta = 10 - 0x0041 = -55)
        // Seg 1: 0xFFFF..=0xFFFF (end marker)
        let seg_count = 2u16;
        let mut fmt4 = Vec::new();
        fmt4.extend_from_slice(&4u16.to_be_bytes()); // format
        fmt4.extend_from_slice(&0u16.to_be_bytes()); // length dummy
        fmt4.extend_from_slice(&0u16.to_be_bytes()); // language
        fmt4.extend_from_slice(&(seg_count * 2).to_be_bytes()); // segCountX2
        fmt4.extend_from_slice(&0u16.to_be_bytes()); // searchRange
        fmt4.extend_from_slice(&0u16.to_be_bytes()); // entrySelector
        fmt4.extend_from_slice(&0u16.to_be_bytes()); // rangeShift

        // endCode: [0x0043, 0xFFFF]
        fmt4.extend_from_slice(&0x0043u16.to_be_bytes());
        fmt4.extend_from_slice(&0xFFFFu16.to_be_bytes());

        // reservedPad
        fmt4.extend_from_slice(&0u16.to_be_bytes());

        // startCode: [0x0041, 0xFFFF]
        fmt4.extend_from_slice(&0x0041u16.to_be_bytes());
        fmt4.extend_from_slice(&0xFFFFu16.to_be_bytes());

        // idDelta: [10 - 0x0041, 1]
        let delta0 = (10i32 - 0x0041i32) as i16;
        fmt4.extend_from_slice(&delta0.to_be_bytes());
        fmt4.extend_from_slice(&1i16.to_be_bytes());

        // idRangeOffset: [0, 0]
        fmt4.extend_from_slice(&0u16.to_be_bytes());
        fmt4.extend_from_slice(&0u16.to_be_bytes());

        cmap.extend_from_slice(&fmt4);

        let ttf_bytes = build_synthetic_ttf(&[(b"cmap", &cmap)]);
        let ttf = TrueTypeFont::parse(&ttf_bytes).expect("Failed to parse TTF");

        assert_eq!(ttf.map_glyph_to_unicode(10), Some('A'));
        assert_eq!(ttf.map_glyph_to_unicode(11), Some('B'));
        assert_eq!(ttf.map_glyph_to_unicode(12), Some('C'));
        assert_eq!(ttf.map_code_to_unicode(0x0041), Some('A'));
    }

    #[test]
    fn test_truetype_cmap_format_12() {
        let mut cmap = Vec::new();
        cmap.extend_from_slice(&0u16.to_be_bytes()); // version
        cmap.extend_from_slice(&1u16.to_be_bytes()); // numTables = 1

        // Encoding record: platform 0, encoding 4, subtable offset 12
        cmap.extend_from_slice(&0u16.to_be_bytes());
        cmap.extend_from_slice(&4u16.to_be_bytes());
        cmap.extend_from_slice(&12u32.to_be_bytes());

        // Format 12 Subtable
        let mut fmt12 = Vec::new();
        fmt12.extend_from_slice(&12u16.to_be_bytes()); // format
        fmt12.extend_from_slice(&0u16.to_be_bytes()); // reserved
        fmt12.extend_from_slice(&0u32.to_be_bytes()); // length
        fmt12.extend_from_slice(&0u32.to_be_bytes()); // language
        fmt12.extend_from_slice(&1u32.to_be_bytes()); // numGroups = 1

        // Group 0: U+1F600..=U+1F602 -> GID 50..=52
        fmt12.extend_from_slice(&0x1F600u32.to_be_bytes());
        fmt12.extend_from_slice(&0x1F602u32.to_be_bytes());
        fmt12.extend_from_slice(&50u32.to_be_bytes());

        cmap.extend_from_slice(&fmt12);

        let ttf_bytes = build_synthetic_ttf(&[(b"cmap", &cmap)]);
        let ttf = TrueTypeFont::parse(&ttf_bytes).expect("Failed to parse TTF");

        assert_eq!(ttf.map_glyph_to_unicode(50), Some('😀'));
        assert_eq!(ttf.map_glyph_to_unicode(51), Some('😁'));
        assert_eq!(ttf.map_glyph_to_unicode(52), Some('😂'));
    }

    #[test]
    fn test_truetype_corrupted_inputs_no_panic() {
        assert!(TrueTypeFont::parse(&[]).is_err());
        assert!(TrueTypeFont::parse(&[0u8; 5]).is_err());
        assert!(TrueTypeFont::parse(&[0x00, 0x01, 0x00, 0x00, 0xFF, 0xFF]).is_err());

        let garbage = vec![0xAA; 100];
        let _ = TrueTypeFont::parse(&garbage);
    }
}
