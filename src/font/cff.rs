use crate::cmap::CMap;
use crate::error::{Error, Result};
use crate::font::agl::AdobeGlyphList;
use std::collections::HashMap;

/// Standard 391 CFF predefined string identifiers (SIDs 0..=390).
pub static CFF_STANDARD_STRINGS: [&str; 391] = [
    ".notdef",
    "space",
    "exclam",
    "quotedbl",
    "numbersign",
    "dollar",
    "percent",
    "ampersand",
    "quoteright",
    "parenleft",
    "parenright",
    "asterisk",
    "plus",
    "comma",
    "hyphen",
    "period",
    "slash",
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "colon",
    "semicolon",
    "less",
    "equal",
    "greater",
    "question",
    "at",
    "A",
    "B",
    "C",
    "D",
    "E",
    "F",
    "G",
    "H",
    "I",
    "J",
    "K",
    "L",
    "M",
    "N",
    "O",
    "P",
    "Q",
    "R",
    "S",
    "T",
    "U",
    "V",
    "W",
    "X",
    "Y",
    "Z",
    "bracketleft",
    "backslash",
    "bracketright",
    "asciicircum",
    "underscore",
    "quoteleft",
    "a",
    "b",
    "c",
    "d",
    "e",
    "f",
    "g",
    "h",
    "i",
    "j",
    "k",
    "l",
    "m",
    "n",
    "o",
    "p",
    "q",
    "r",
    "s",
    "t",
    "u",
    "v",
    "w",
    "x",
    "y",
    "z",
    "braceleft",
    "bar",
    "braceright",
    "asciitilde",
    "exclamdown",
    "cent",
    "sterling",
    "fraction",
    "yen",
    "florin",
    "section",
    "currency",
    "quotesingle",
    "quotedblleft",
    "guillemotleft",
    "guilsinglleft",
    "guilsinglright",
    "fi",
    "fl",
    "endash",
    "dagger",
    "daggerdbl",
    "periodcentered",
    "paragraph",
    "bullet",
    "quotesinglbase",
    "quotedblbase",
    "quotedblright",
    "guillemotright",
    "ellipsis",
    "perthousand",
    "questiondown",
    "grave",
    "acute",
    "circumflex",
    "tilde",
    "macron",
    "breve",
    "dotaccent",
    "dieresis",
    "ring",
    "cedilla",
    "hungarumlaut",
    "ogonek",
    "caron",
    "emdash",
    "AE",
    "ordfeminine",
    "Lslash",
    "Oslash",
    "OE",
    "ordmasculine",
    "ae",
    "dotlessi",
    "lslash",
    "oslash",
    "oe",
    "germandbls",
    "onesuperior",
    "logicalnot",
    "mu",
    "trademark",
    "Eth",
    "onehalf",
    "plusminus",
    "Thorn",
    "onequarter",
    "divide",
    "brokenbar",
    "degree",
    "thorn",
    "threequarters",
    "twosuperior",
    "registered",
    "minus",
    "eth",
    "multiply",
    "threesuperior",
    "copyright",
    "Aacute",
    "Acircumflex",
    "Adieresis",
    "Agrave",
    "Aring",
    "Atilde",
    "Ccedilla",
    "Eacute",
    "Ecircumflex",
    "Edieresis",
    "Egrave",
    "Iacute",
    "Icircumflex",
    "Idieresis",
    "Igrave",
    "Ntilde",
    "Oacute",
    "Ocircumflex",
    "Odieresis",
    "Ograve",
    "Otilde",
    "Scaron",
    "Uacute",
    "Ucircumflex",
    "Udieresis",
    "Ugrave",
    "Yacute",
    "Zcaron",
    "aacute",
    "acircumflex",
    "adieresis",
    "agrave",
    "aring",
    "atilde",
    "ccedilla",
    "eacute",
    "ecircumflex",
    "edieresis",
    "egrave",
    "iacute",
    "icircumflex",
    "idieresis",
    "igrave",
    "ntilde",
    "oacute",
    "ocircumflex",
    "odieresis",
    "ograve",
    "otilde",
    "scaron",
    "uacute",
    "ucircumflex",
    "udieresis",
    "ugrave",
    "yacute",
    "ydieresis",
    "zcaron",
    "Exclamdown",
    "Cent",
    "Lslash",
    "Scaron",
    "Zcaron",
    "brokenbar",
    "Section",
    "Dieresis",
    "Copyright",
    "Ordfeminine",
    "Guillemotleft",
    "Logicalnot",
    "Minus",
    "Registered",
    "Macron",
    "Degree",
    "Plusminus",
    "Twosuperior",
    "Threesuperior",
    "Acute",
    "Mu",
    "Paragraph",
    "Periodcentered",
    "Cedilla",
    "Onesuperior",
    "Ordmasculine",
    "Guillemotright",
    "Onequarter",
    "Onehalf",
    "Threequarters",
    "Questiondown",
    "Agrave",
    "Aacute",
    "Acircumflex",
    "Atilde",
    "Adieresis",
    "Aring",
    "AE",
    "Ccedilla",
    "Egrave",
    "Eacute",
    "Ecircumflex",
    "Edieresis",
    "Igrave",
    "Iacute",
    "Icircumflex",
    "Idieresis",
    "Eth",
    "Ntilde",
    "Ograve",
    "Oacute",
    "Ocircumflex",
    "Otilde",
    "Odieresis",
    "Multiply",
    "Oslash",
    "Ugrave",
    "Uacute",
    "Ucircumflex",
    "Udieresis",
    "Yacute",
    "Thorn",
    "Germandbls",
    "agrave",
    "aacute",
    "acircumflex",
    "atilde",
    "adieresis",
    "aring",
    "ae",
    "ccedilla",
    "egrave",
    "eacute",
    "ecircumflex",
    "edieresis",
    "igrave",
    "iacute",
    "icircumflex",
    "idieresis",
    "eth",
    "ntilde",
    "ograve",
    "oacute",
    "ocircumflex",
    "otilde",
    "odieresis",
    "divide",
    "oslash",
    "ugrave",
    "uacute",
    "ucircumflex",
    "udieresis",
    "yacute",
    "thorn",
    "ydieresis",
    "dollar",
    "onesuperior",
    "twosuperior",
    "threesuperior",
    "degree",
    "mu",
    "periodcentered",
    "onequarter",
    "onehalf",
    "threequarters",
    "Agrave",
    "Aacute",
    "Acircumflex",
    "Atilde",
    "Adieresis",
    "Aring",
    "Ccedilla",
    "Egrave",
    "Eacute",
    "Ecircumflex",
    "Edieresis",
    "Igrave",
    "Iacute",
    "Icircumflex",
    "Idieresis",
    "Ntilde",
    "Ograve",
    "Oacute",
    "Ocircumflex",
    "Otilde",
    "Odieresis",
    "Ugrave",
    "Uacute",
    "Ucircumflex",
    "Udieresis",
    "Yacute",
    "agrave",
    "aacute",
    "acircumflex",
    "atilde",
    "adieresis",
    "aring",
    "ccedilla",
    "egrave",
    "eacute",
    "ecircumflex",
    "edieresis",
    "igrave",
    "iacute",
    "icircumflex",
    "idieresis",
    "ntilde",
    "ograve",
    "oacute",
    "ocircumflex",
    "otilde",
    "odieresis",
    "ugrave",
    "uacute",
    "ucircumflex",
    "udieresis",
    "yacute",
    "ydieresis",
    "dotlessj",
    "f_f",
    "f_i",
    "f_l",
    "f_f_i",
];

/// Zero-copy binary parser and glyph-to-Unicode mapping for CFF (`/FontFile3`) font files.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CffFont {
    gid_to_unicode: HashMap<u16, char>,
}

impl CffFont {
    /// Parses a raw CFF font byte stream into a `CffFont`.
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 4 {
            return Err(Error::UnexpectedEof(data.len()));
        }

        let hdr_size = data[2] as usize;
        if hdr_size < 4 || hdr_size > data.len() {
            return Err(Error::SyntaxError {
                offset: 2,
                message: "Invalid CFF header size",
            });
        }

        // 1. Name INDEX
        let name_idx = CffIndex::parse(&data[hdr_size..], hdr_size)?;
        let top_dict_pos = hdr_size + name_idx.total_size;
        if top_dict_pos > data.len() {
            return Err(Error::UnexpectedEof(top_dict_pos));
        }

        // 2. Top DICT INDEX
        let top_dict_idx = CffIndex::parse(&data[top_dict_pos..], top_dict_pos)?;
        let string_idx_pos = top_dict_pos + top_dict_idx.total_size;
        if string_idx_pos > data.len() {
            return Err(Error::UnexpectedEof(string_idx_pos));
        }

        // 3. String INDEX
        let string_idx = CffIndex::parse(&data[string_idx_pos..], string_idx_pos)?;

        // 4. Parse Top DICT for font 0
        if top_dict_idx.count == 0 {
            return Ok(CffFont::default());
        }
        let top_dict_bytes = top_dict_idx.get(0).unwrap_or(&[]);

        let mut charset_offset: usize = 0; // Default: 0 = ISOAdobe
        let mut charstrings_offset: Option<usize> = None;

        parse_cff_dict(top_dict_bytes, |op, operands| match op {
            15 => {
                // charset
                if let Some(&val) = operands.last() {
                    if val >= 0 {
                        charset_offset = val as usize;
                    }
                }
            }
            17 => {
                // CharStrings
                if let Some(&val) = operands.last() {
                    if val >= 0 {
                        charstrings_offset = Some(val as usize);
                    }
                }
            }
            _ => {}
        })?;

        // CharStrings count gives num_glyphs
        let num_glyphs = if let Some(cs_pos) = charstrings_offset {
            if cs_pos + 2 <= data.len() {
                u16::from_be_bytes([data[cs_pos], data[cs_pos + 1]]) as usize
            } else {
                0
            }
        } else {
            0
        };

        if num_glyphs == 0 {
            return Ok(CffFont::default());
        }

        // 5. Parse Charset to map GID -> SID
        let mut gid_to_sid: HashMap<u16, u16> = HashMap::with_capacity(num_glyphs);
        // GID 0 is always .notdef (SID 0)
        gid_to_sid.insert(0, 0);

        if charset_offset == 0 {
            // ISOAdobe charset: GIDs 1..num_glyphs map to SIDs 1..num_glyphs (up to 228)
            for gid in 1..num_glyphs {
                if gid <= 228 {
                    gid_to_sid.insert(gid as u16, gid as u16);
                }
            }
        } else if charset_offset < data.len() {
            let cs_data = &data[charset_offset..];
            if !cs_data.is_empty() {
                let format = cs_data[0];
                match format {
                    0 => {
                        // Format 0: array of u16 SIDs for GIDs 1..num_glyphs
                        let needed = 1 + (num_glyphs - 1) * 2;
                        if cs_data.len() >= needed {
                            for gid in 1..num_glyphs {
                                let pos = 1 + (gid - 1) * 2;
                                let sid = u16::from_be_bytes([cs_data[pos], cs_data[pos + 1]]);
                                gid_to_sid.insert(gid as u16, sid);
                            }
                        }
                    }
                    1 => {
                        // Format 1: ranges with nLeft (u8)
                        let mut curr_gid = 1usize;
                        let mut pos = 1usize;
                        while curr_gid < num_glyphs && pos + 3 <= cs_data.len() {
                            let first =
                                u16::from_be_bytes([cs_data[pos], cs_data[pos + 1]]) as usize;
                            let n_left = cs_data[pos + 2] as usize;
                            pos += 3;

                            for k in 0..=n_left {
                                if curr_gid < num_glyphs {
                                    gid_to_sid.insert(curr_gid as u16, (first + k) as u16);
                                    curr_gid += 1;
                                }
                            }
                        }
                    }
                    2 => {
                        // Format 2: ranges with nLeft (u16)
                        let mut curr_gid = 1usize;
                        let mut pos = 1usize;
                        while curr_gid < num_glyphs && pos + 4 <= cs_data.len() {
                            let first =
                                u16::from_be_bytes([cs_data[pos], cs_data[pos + 1]]) as usize;
                            let n_left =
                                u16::from_be_bytes([cs_data[pos + 2], cs_data[pos + 3]]) as usize;
                            pos += 4;

                            for k in 0..=n_left {
                                if curr_gid < num_glyphs {
                                    gid_to_sid.insert(curr_gid as u16, (first + k) as u16);
                                    curr_gid += 1;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // 6. Map SIDs to Unicode via AGL
        let mut gid_to_unicode = HashMap::new();
        for (&gid, &sid) in &gid_to_sid {
            let name_opt: Option<&str> = if (sid as usize) < CFF_STANDARD_STRINGS.len() {
                Some(CFF_STANDARD_STRINGS[sid as usize])
            } else {
                let str_idx = (sid as usize) - 391;
                if let Some(bytes) = string_idx.get(str_idx) {
                    std::str::from_utf8(bytes).ok()
                } else {
                    None
                }
            };

            if let Some(name) = name_opt {
                if let Some(ch) = AdobeGlyphList::name_to_unicode(name) {
                    gid_to_unicode.insert(gid, ch);
                }
            }
        }

        Ok(CffFont { gid_to_unicode })
    }

    /// Maps a Glyph ID to a Unicode character.
    #[inline]
    pub fn map_glyph_to_unicode(&self, gid: u16) -> Option<char> {
        self.gid_to_unicode.get(&gid).copied()
    }

    /// Returns the underlying mapping from GID to Unicode character.
    #[inline]
    pub fn glyph_to_unicode(&self) -> &HashMap<u16, char> {
        &self.gid_to_unicode
    }

    /// Builds a `CMap` representing all resolved glyph mappings.
    pub fn to_cmap(&self) -> CMap {
        let mut cmap = CMap::new();
        cmap.set_has_2byte_codes(true);
        for (&gid, &ch) in &self.gid_to_unicode {
            cmap.insert(gid as u32, ch.to_string());
        }
        cmap
    }
}

/// Representation of a parsed CFF INDEX structure.
struct CffIndex<'a> {
    count: usize,
    offsets: Vec<usize>,
    data: &'a [u8],
    total_size: usize,
}

impl<'a> CffIndex<'a> {
    fn parse(bytes: &'a [u8], base_offset: usize) -> Result<Self> {
        if bytes.len() < 2 {
            return Err(Error::UnexpectedEof(base_offset));
        }

        let count = u16::from_be_bytes([bytes[0], bytes[1]]) as usize;
        if count == 0 {
            return Ok(Self {
                count: 0,
                offsets: Vec::new(),
                data: &[],
                total_size: 2,
            });
        }

        if bytes.len() < 3 {
            return Err(Error::UnexpectedEof(base_offset + 2));
        }

        let off_size = bytes[2] as usize;
        if off_size == 0 || off_size > 4 {
            return Err(Error::SyntaxError {
                offset: base_offset + 2,
                message: "Invalid CFF INDEX offset size",
            });
        }

        let offsets_bytes_len = (count + 1) * off_size;
        if bytes.len() < 3 + offsets_bytes_len {
            return Err(Error::UnexpectedEof(base_offset + 3));
        }

        let mut offsets = Vec::with_capacity(count + 1);
        for i in 0..=count {
            let p = 3 + i * off_size;
            let mut off = 0usize;
            for b in &bytes[p..p + off_size] {
                off = (off << 8) | (*b as usize);
            }
            offsets.push(off);
        }

        let data_start = 3 + offsets_bytes_len;
        let last_off = offsets[count];
        if last_off < 1 {
            return Err(Error::SyntaxError {
                offset: base_offset,
                message: "Invalid CFF INDEX terminating offset",
            });
        }

        let data_len = last_off - 1;
        let total_size = data_start + data_len;
        if bytes.len() < total_size {
            return Err(Error::UnexpectedEof(base_offset + data_start));
        }

        let data = &bytes[data_start..total_size];

        Ok(Self {
            count,
            offsets,
            data,
            total_size,
        })
    }

    fn get(&self, idx: usize) -> Option<&'a [u8]> {
        if idx >= self.count {
            return None;
        }
        let start = self.offsets[idx].checked_sub(1)?;
        let end = self.offsets[idx + 1].checked_sub(1)?;
        if start <= end && end <= self.data.len() {
            Some(&self.data[start..end])
        } else {
            None
        }
    }
}

/// Parses CFF DICT key-value pairs.
fn parse_cff_dict<F>(dict_bytes: &[u8], mut callback: F) -> Result<()>
where
    F: FnMut(u16, &[i32]),
{
    let mut operands: Vec<i32> = Vec::new();
    let mut i = 0;

    while i < dict_bytes.len() {
        let b0 = dict_bytes[i];
        i += 1;

        match b0 {
            // Operator: 2-byte escape (starts with 12)
            12 => {
                if i < dict_bytes.len() {
                    let b1 = dict_bytes[i];
                    i += 1;
                    let op = 0x0C00 | (b1 as u16);
                    callback(op, &operands);
                    operands.clear();
                }
            }
            // Operator: 1-byte
            0..=21 => {
                callback(b0 as u16, &operands);
                operands.clear();
            }
            // Reserved
            22..=27 | 31 => {}
            // Operand: 16-bit integer
            28 => {
                if i + 2 <= dict_bytes.len() {
                    let val = i16::from_be_bytes([dict_bytes[i], dict_bytes[i + 1]]) as i32;
                    i += 2;
                    operands.push(val);
                }
            }
            // Operand: 32-bit integer
            29 => {
                if i + 4 <= dict_bytes.len() {
                    let val = i32::from_be_bytes([
                        dict_bytes[i],
                        dict_bytes[i + 1],
                        dict_bytes[i + 2],
                        dict_bytes[i + 3],
                    ]);
                    i += 4;
                    operands.push(val);
                }
            }
            // Operand: float (nibbles ending in 0xF)
            30 => {
                while i < dict_bytes.len() {
                    let byte = dict_bytes[i];
                    i += 1;
                    if (byte & 0x0F) == 0x0F || (byte >> 4) == 0x0F {
                        break;
                    }
                }
                // Push integer 0 as placeholder for float in dict operands
                operands.push(0);
            }
            // Operand: -107..107
            32..=246 => {
                operands.push((b0 as i32) - 139);
            }
            // Operand: 108..1131
            247..=250 => {
                if i < dict_bytes.len() {
                    let b1 = dict_bytes[i] as i32;
                    i += 1;
                    operands.push(((b0 as i32) - 247) * 256 + b1 + 108);
                }
            }
            // Operand: -1131..-108
            251..=254 => {
                if i < dict_bytes.len() {
                    let b1 = dict_bytes[i] as i32;
                    i += 1;
                    operands.push(-((b0 as i32) - 251) * 256 - b1 - 108);
                }
            }
            255 => {}
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Constructs a synthetic CFF byte sequence for testing.
    fn build_synthetic_cff(
        custom_strings: &[&str],
        charset_format: u8,
        glyph_sids: &[u16],
    ) -> Vec<u8> {
        let mut out = Vec::new();

        // 1. Header (4 bytes): major=1, minor=0, hdrSize=4, offSize=1
        out.extend_from_slice(&[1, 0, 4, 1]);

        // 2. Name INDEX with 1 name ("TestFont")
        let font_name = b"TestFont";
        out.extend_from_slice(&1u16.to_be_bytes()); // count = 1
        out.push(1); // offSize = 1
        out.push(1); // offset[0] = 1
        out.push(1 + font_name.len() as u8); // offset[1] = 1 + len
        out.extend_from_slice(font_name);

        // We will build CharStrings INDEX, String INDEX, and Charset,
        // then patch Top DICT with their offsets.
        // Placeholder for Top DICT INDEX:
        // count=1, offSize=1, offsets=[1, 1+30], data=30 bytes of 0
        let dummy_dict_len = 30;
        out.extend_from_slice(&1u16.to_be_bytes());
        out.push(1);
        out.push(1);
        out.push(1 + dummy_dict_len as u8);
        let top_dict_data_start = out.len();
        out.extend_from_slice(&vec![0u8; dummy_dict_len]);

        // String INDEX
        out.extend_from_slice(&(custom_strings.len() as u16).to_be_bytes());
        if !custom_strings.is_empty() {
            out.push(2); // offSize = 2
            let mut off = 1u16;
            out.extend_from_slice(&off.to_be_bytes());
            for s in custom_strings {
                off += s.len() as u16;
                out.extend_from_slice(&off.to_be_bytes());
            }
            for s in custom_strings {
                out.extend_from_slice(s.as_bytes());
            }
        }

        // Charset
        let charset_pos = out.len();
        out.push(charset_format);
        match charset_format {
            0 => {
                // SIDs for GIDs 1..num_glyphs
                for &sid in &glyph_sids[1..] {
                    out.extend_from_slice(&sid.to_be_bytes());
                }
            }
            1 => {
                // Ranges (first: u16, nLeft: u8)
                // For simplicity, 1 glyph per range
                for &sid in &glyph_sids[1..] {
                    out.extend_from_slice(&sid.to_be_bytes());
                    out.push(0); // nLeft = 0 (1 glyph)
                }
            }
            2 => {
                // Ranges (first: u16, nLeft: u16)
                for &sid in &glyph_sids[1..] {
                    out.extend_from_slice(&sid.to_be_bytes());
                    out.extend_from_slice(&0u16.to_be_bytes());
                }
            }
            _ => {}
        }

        // CharStrings INDEX: num_glyphs = glyph_sids.len()
        let charstrings_pos = out.len();
        let num_glyphs = glyph_sids.len() as u16;
        out.extend_from_slice(&num_glyphs.to_be_bytes());
        out.push(1); // offSize
        out.push(1); // offset 0
        for i in 1..=num_glyphs {
            out.push(1 + i as u8);
        }
        // Dummy 1-byte charstrings (endchar = 14)
        out.resize(out.len() + num_glyphs as usize, 14);

        // Now generate actual Top DICT bytes:
        // charset: offset (operator 15)
        // CharStrings: offset (operator 17)
        let mut real_dict = Vec::new();
        // charset operand (29: 32-bit int) + op 15
        real_dict.push(29);
        real_dict.extend_from_slice(&(charset_pos as i32).to_be_bytes());
        real_dict.push(15);

        // CharStrings operand + op 17
        real_dict.push(29);
        real_dict.extend_from_slice(&(charstrings_pos as i32).to_be_bytes());
        real_dict.push(17);

        assert!(
            real_dict.len() <= dummy_dict_len,
            "Real dict too big for dummy slot"
        );
        // Pad with NOPs (22..=27 are reserved / ignored)
        while real_dict.len() < dummy_dict_len {
            real_dict.push(22);
        }

        out[top_dict_data_start..top_dict_data_start + dummy_dict_len]
            .copy_from_slice(&real_dict);

        out
    }

    #[test]
    fn test_cff_standard_sids() {
        // GID 0: .notdef (SID 0)
        // GID 1: 'A' (SID 34)
        // GID 2: 'fi' (SID 109)
        // GID 3: 'ampersand' (SID 7)
        let glyph_sids = [0, 34, 109, 7];
        let cff_bytes = build_synthetic_cff(&[], 0, &glyph_sids);

        let font = CffFont::parse(&cff_bytes).expect("Failed to parse CFF font");
        assert_eq!(font.map_glyph_to_unicode(0), None); // .notdef
        assert_eq!(font.map_glyph_to_unicode(1), Some('A'));
        assert_eq!(font.map_glyph_to_unicode(2), Some('ﬁ'));
        assert_eq!(font.map_glyph_to_unicode(3), Some('&'));
    }

    #[test]
    fn test_cff_custom_string_index() {
        // Custom string 0 (SID 391): "uni0042" (B)
        // Custom string 1 (SID 392): "Euro" (€)
        // GID 1: SID 391
        // GID 2: SID 392
        let custom_strings = ["uni0042", "Euro"];
        let glyph_sids = [0, 391, 392];
        let cff_bytes = build_synthetic_cff(&custom_strings, 1, &glyph_sids);

        let font = CffFont::parse(&cff_bytes).expect("Failed to parse CFF font");
        assert_eq!(font.map_glyph_to_unicode(1), Some('B'));
        assert_eq!(font.map_glyph_to_unicode(2), Some('€'));
    }

    #[test]
    fn test_cff_charset_format_2() {
        let custom_strings = ["u1F600"]; // SID 391 -> 😀
        let glyph_sids = [0, 391];
        let cff_bytes = build_synthetic_cff(&custom_strings, 2, &glyph_sids);

        let font = CffFont::parse(&cff_bytes).expect("Failed to parse CFF font");
        assert_eq!(font.map_glyph_to_unicode(1), Some('😀'));
    }

    #[test]
    fn test_cff_corrupted_inputs_no_panic() {
        assert!(CffFont::parse(&[]).is_err());
        assert!(CffFont::parse(&[1, 0]).is_err());
        assert!(CffFont::parse(&[1, 0, 2, 1]).is_err());

        let garbage = vec![0xBB; 100];
        let _ = CffFont::parse(&garbage);
    }
}
