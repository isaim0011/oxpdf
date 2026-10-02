use crate::error::{Error, Result};
use crate::lexer::{Lexer, Token};
use crate::parser::Parser;
use crate::types::Object;
use memchr::memmem;
use std::collections::BTreeMap;

/// Reads up to 8 bytes as a big-endian u64. Used for XRef stream binary field decoding (PDF §7.5.8).
#[inline]
fn read_be_u64(bytes: &[u8]) -> u64 {
    let mut result = 0u64;
    for &b in bytes.iter().take(8) {
        result = (result << 8) | (b as u64);
    }
    result
}

/// Entry in the Cross-Reference table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XRefEntry {
    /// In-use object pointing to byte offset in document
    InUse { offset: u64, gen: u16 },
    /// Compressed object within an Object Stream (PDF 1.5+)
    Compressed { stream_obj_id: u32, index: u16 },
    /// Free object
    Free { next_free_id: u32, gen: u16 },
}

/// Hybrid cross-reference index supporting traditional tables, PDF 1.5+ stream xrefs,
/// and fallback linear scanning (Chromium PDFium-inspired fault tolerance).
#[derive(Debug, Clone, Default)]
pub struct XRefTable {
    pub entries: BTreeMap<u32, XRefEntry>,
    pub trailer_dict: Option<BTreeMap<String, String>>, // key info like /Root, /Info, /Size
    pub trailer: Option<BTreeMap<String, Object<'static>>>,
}

impl XRefTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: u32, entry: XRefEntry) {
        self.entries.insert(id, entry);
    }

    pub fn get(&self, id: u32) -> Option<&XRefEntry> {
        self.entries.get(&id)
    }

    /// Primary entrypoint: locates `startxref` near the end of the byte stream,
    /// parses the xref chain, and falls back to a full document scan if corrupt.
    pub fn parse_or_reconstruct(data: &[u8]) -> Result<Self> {
        if let Ok(table) = Self::parse_standard(data) {
            if !table.entries.is_empty() {
                return Ok(table);
            }
        }
        // Fallback per §3.2: Chromium PDFium-style resilient linear reconstructor owned by `recover.rs`
        crate::recover::repair(data)
    }

    /// Standard backward resolution from `startxref`.
    pub fn parse_standard(data: &[u8]) -> Result<Self> {
        let startxref_offset = Self::find_startxref_offset(data)?;
        let mut table = Self::new();
        let mut visited_offsets = std::collections::HashSet::new();
        let mut current_offset = startxref_offset;

        while current_offset < data.len() as u64 && !visited_offsets.contains(&current_offset) {
            visited_offsets.insert(current_offset);
            let cur_usize = match usize::try_from(current_offset) {
                Ok(u) if u < data.len() => u,
                _ => break,
            };
            let mut lexer = Lexer::new(&data[cur_usize..]);

            match lexer.next_token()? {
                Some(Token::Keyword("xref")) => {
                    let next_prev =
                        Self::parse_xref_subsections(&mut lexer, &mut table, data, cur_usize)?;
                    match next_prev {
                        Some(prev) => current_offset = prev,
                        None => break,
                    }
                }
                Some(Token::Integer(_)) => {
                    // PDF 1.5+ XRef Stream: `N G obj << /Type /XRef ... >> stream ... endstream`
                    // The startxref offset points to the object id, not to an `xref` keyword.
                    match Self::parse_xref_stream(data, cur_usize, &mut table) {
                        Ok(Some(prev)) => current_offset = prev,
                        Ok(None) => break,
                        Err(_) => break, // Fall through to linear reconstruction
                    }
                }
                _ => break,
            }
        }

        Ok(table)
    }

    /// Parses a PDF 1.5+ cross-reference stream object (ISO 32000-1 §7.5.8).
    ///
    /// Format: `N G obj << /Type /XRef /Size N /W [w1 w2 w3] ... >> stream ... endstream`
    /// Each entry is `w1+w2+w3` bytes:
    /// - Field 1 (w1): type (0=free, 1=uncompressed offset, 2=compressed in ObjStm)
    /// - Field 2 (w2): for type 1 = byte offset; for type 2 = ObjStm object id
    /// - Field 3 (w3): for type 1 = generation; for type 2 = index within ObjStm
    fn parse_xref_stream(data: &[u8], offset: usize, table: &mut XRefTable) -> Result<Option<u64>> {
        use crate::parser::Parser;
        use crate::types::Object;

        let mut parser = Parser::new(&data[offset..]);
        let lexer = parser.lexer_mut();

        // Skip: id gen obj
        let _id = lexer.next_token()?;
        let _gen = lexer.next_token()?;
        match lexer.next_token()? {
            Some(Token::Keyword("obj")) => {}
            _ => {
                return Err(Error::SyntaxError {
                    offset,
                    message: "Expected 'obj' keyword in XRef stream",
                })
            }
        }

        // Parse the stream object
        let stream_obj = parser.parse_object()?;
        let (dict, raw_data) = match stream_obj {
            Some(Object::Stream { dict, data }) => (dict, data),
            _ => {
                return Err(Error::SyntaxError {
                    offset,
                    message: "XRef stream is not a stream object",
                })
            }
        };

        // Validate /Type /XRef
        match dict.get("Type").and_then(|t| t.as_name()) {
            Some("XRef") => {}
            _ => {
                return Err(Error::SyntaxError {
                    offset,
                    message: "Stream at startxref is not /Type /XRef",
                })
            }
        }

        // Decompress if FlateDecode
        let is_flate = dict
            .get("Filter")
            .and_then(|f| f.as_name())
            .map(|n| n == "FlateDecode")
            .unwrap_or(false);

        let mut decoded: Vec<u8> = if is_flate {
            let view = crate::stream::StreamView::new(&raw_data)
                .with_filter(crate::stream::FilterKind::FlateDecode);
            view.decode()?
        } else {
            raw_data.to_vec()
        };

        // Handle /DecodeParms predictor if specified (e.g. Predictor 12 PNG Up)
        if let Some(decode_parms) = dict.get("DecodeParms") {
            let (predictor, columns, colors, bpc) = match decode_parms {
                Object::Dictionary(dp) => {
                    let pred = match dp.get("Predictor") {
                        Some(Object::Integer(p)) => *p,
                        _ => 1,
                    };
                    let cols = match dp.get("Columns") {
                        Some(Object::Integer(c)) => *c,
                        _ => 1,
                    };
                    let clrs = match dp.get("Colors") {
                        Some(Object::Integer(c)) => *c,
                        _ => 1,
                    };
                    let bits = match dp.get("BitsPerComponent") {
                        Some(Object::Integer(b)) => *b,
                        _ => 8,
                    };
                    (pred, cols as usize, clrs as usize, bits as usize)
                }
                _ => (1, 1, 1, 8),
            };

            if predictor >= 10 {
                // PNG predictor (10..15): each row has 1 filter byte prefix
                let bytes_per_pixel = (colors * bpc).div_ceil(8);
                let bpp = bytes_per_pixel.max(1);
                let row_bytes = (columns * colors * bpc).div_ceil(8);
                let stride = row_bytes + 1;

                if stride > 1 && decoded.len() >= stride {
                    let mut unpredicted = Vec::with_capacity(decoded.len());
                    let mut prev_row = vec![0u8; row_bytes];

                    for chunk in decoded.chunks_exact(stride) {
                        let filter_byte = chunk[0];
                        let raw = &chunk[1..];
                        let mut curr_row = vec![0u8; row_bytes];

                        for i in 0..row_bytes {
                            let left = if i >= bpp { curr_row[i - bpp] } else { 0 };
                            let up = prev_row[i];
                            let up_left = if i >= bpp { prev_row[i - bpp] } else { 0 };

                            let val = match filter_byte {
                                0 => raw[i], // None
                                1 => raw[i].wrapping_add(left), // Sub
                                2 => raw[i].wrapping_add(up), // Up
                                3 => {
                                    // Average
                                    let avg = ((left as u16 + up as u16) / 2) as u8;
                                    raw[i].wrapping_add(avg)
                                }
                                4 => {
                                    // Paeth
                                    let p = left as i32 + up as i32 - up_left as i32;
                                    let pa = (p - left as i32).abs();
                                    let pb = (p - up as i32).abs();
                                    let pc = (p - up_left as i32).abs();
                                    let pr = if pa <= pb && pa <= pc {
                                        left
                                    } else if pb <= pc {
                                        up
                                    } else {
                                        up_left
                                    };
                                    raw[i].wrapping_add(pr)
                                }
                                _ => raw[i],
                            };
                            curr_row[i] = val;
                        }
                        unpredicted.extend_from_slice(&curr_row);
                        prev_row = curr_row;
                    }
                    decoded = unpredicted;
                }
            } else if predictor == 2 {
                // TIFF Predictor 2 (Horizontal Differencing)
                let bytes_per_pixel = (colors * bpc).div_ceil(8);
                let bpp = bytes_per_pixel.max(1);
                let row_bytes = (columns * colors * bpc).div_ceil(8);
                for chunk in decoded.chunks_exact_mut(row_bytes) {
                    for i in bpp..row_bytes {
                        chunk[i] = chunk[i].wrapping_add(chunk[i - bpp]);
                    }
                }
            }
        }

        // Read /W [w1 w2 w3] field widths
        let w_arr = match dict.get("W") {
            Some(Object::Array(arr)) => arr,
            _ => {
                return Err(Error::SyntaxError {
                    offset,
                    message: "XRef stream missing /W array",
                })
            }
        };
        if w_arr.len() < 3 {
            return Err(Error::SyntaxError {
                offset,
                message: "XRef stream /W must have exactly 3 elements",
            });
        }
        let w0 = match &w_arr[0] {
            Object::Integer(i) if *i >= 0 && *i <= 8 => *i as usize,
            _ => 0,
        };
        let w1 = match &w_arr[1] {
            Object::Integer(i) if *i >= 0 && *i <= 8 => *i as usize,
            _ => 0,
        };
        let w2 = match &w_arr[2] {
            Object::Integer(i) if *i >= 0 && *i <= 8 => *i as usize,
            _ => 0,
        };
        let entry_size = match w0.checked_add(w1).and_then(|s| s.checked_add(w2)) {
            Some(s) if s > 0 => s,
            _ => {
                return Err(Error::SyntaxError {
                    offset,
                    message: "XRef stream has invalid entry size",
                });
            }
        };

        // Read /Index [start count start count ...] — defaults to [0, /Size]
        let size = match dict.get("Size") {
            Some(Object::Integer(s)) if *s >= 0 => u32::try_from(*s).unwrap_or(0),
            _ => 0,
        };
        let index_ranges: Vec<(u32, u32)> = match dict.get("Index") {
            Some(Object::Array(arr)) if arr.len() >= 2 => arr
                .chunks(2)
                .filter_map(|chunk| {
                    if chunk.len() == 2 {
                        if let (Object::Integer(start), Object::Integer(count)) =
                            (&chunk[0], &chunk[1])
                        {
                            if *start >= 0 && *count >= 0 {
                                if let (Ok(s), Ok(c)) =
                                    (u32::try_from(*start), u32::try_from(*count))
                                {
                                    return Some((s, c));
                                }
                            }
                        }
                    }
                    None
                })
                .collect(),
            _ => vec![(0, size)],
        };

        // Decode entries
        let mut pos = 0usize;
        for (start_id, count) in &index_ranges {
            for i in 0..*count {
                let is_oob = match pos.checked_add(entry_size) {
                    Some(end) => end > decoded.len(),
                    None => true,
                };
                if is_oob {
                    break;
                }
                let obj_id = match start_id.checked_add(i) {
                    Some(id) => id,
                    None => break,
                };

                // Read field 1: type
                let field_type = if w0 == 0 {
                    1u64 // default type is 1 when w0=0
                } else {
                    read_be_u64(&decoded[pos..pos + w0])
                };
                // Read field 2
                let field2 = if w1 == 0 {
                    0u64
                } else {
                    read_be_u64(&decoded[pos + w0..pos + w0 + w1])
                };
                // Read field 3
                let field3 = if w2 == 0 {
                    0u64
                } else {
                    read_be_u64(&decoded[pos + w0 + w1..pos + entry_size])
                };

                // Only insert if not already present (earlier xref takes precedence for incremental updates)
                if !table.entries.contains_key(&obj_id) {
                    match field_type {
                        0 => {
                            table.insert(
                                obj_id,
                                XRefEntry::Free {
                                    next_free_id: u32::try_from(field2).unwrap_or(u32::MAX),
                                    gen: u16::try_from(field3).unwrap_or(u16::MAX),
                                },
                            );
                        }
                        1 => {
                            table.insert(
                                obj_id,
                                XRefEntry::InUse {
                                    offset: field2,
                                    gen: u16::try_from(field3).unwrap_or(u16::MAX),
                                },
                            );
                        }
                        2 => {
                            table.insert(
                                obj_id,
                                XRefEntry::Compressed {
                                    stream_obj_id: u32::try_from(field2).unwrap_or(u32::MAX),
                                    index: u16::try_from(field3).unwrap_or(u16::MAX),
                                },
                            );
                        }
                        _ => {} // Unknown type — skip
                    }
                }
                pos = match pos.checked_add(entry_size) {
                    Some(p) => p,
                    None => break,
                };
            }
        }

        if table.trailer.is_none() {
            let mut owned_map = std::collections::BTreeMap::new();
            for (k, v) in &dict {
                owned_map.insert(k.to_string(), v.clone().into_owned());
            }
            table.trailer = Some(owned_map);
        }
        if table.trailer_dict.is_none() {
            let mut trailer_map = std::collections::BTreeMap::new();
            for (k, v) in &dict {
                let val = match v {
                    Object::Reference { id, .. } => id.to_string(),
                    Object::Integer(s) => s.to_string(),
                    Object::Name(n) => n.to_string(),
                    _ => format!("{v:?}"),
                };
                trailer_map.insert(k.to_string(), val);
            }
            if !trailer_map.is_empty() {
                table.trailer_dict = Some(trailer_map);
            }
        }

        // Follow /Prev chain for incremental updates
        match dict.get("Prev") {
            Some(Object::Integer(prev)) if *prev > 0 && (*prev as u64) < data.len() as u64 => {
                Ok(Some(*prev as u64))
            }
            _ => Ok(None),
        }
    }

    /// Find the byte offset of `startxref` by scanning backward from the file end.
    pub fn find_startxref_offset(data: &[u8]) -> Result<u64> {
        // PDF spec suggests startxref is within the last 1024 bytes
        let scan_start = data.len().saturating_sub(2048);
        let tail = &data[scan_start..];

        let finder = memmem::Finder::new(b"startxref");
        let matches: Vec<usize> = finder.find_iter(tail).collect();
        let last_match = matches.last().ok_or(Error::SyntaxError {
            offset: scan_start,
            message: "Missing 'startxref' token in file trailer",
        })?;

        let mut lexer = Lexer::new(&tail[last_match + b"startxref".len()..]);
        match lexer.next_token()? {
            Some(Token::Integer(offset)) if offset >= 0 && (offset as u64) < data.len() as u64 => {
                Ok(offset as u64)
            }
            _ => Err(Error::SyntaxError {
                offset: scan_start + last_match,
                message: "Invalid or missing byte offset after 'startxref'",
            }),
        }
    }

    fn parse_xref_subsections(
        lexer: &mut Lexer<'_>,
        table: &mut XRefTable,
        data: &[u8],
        base_offset: usize,
    ) -> Result<Option<u64>> {
        loop {
            let tok1 = match lexer.next_token()? {
                Some(t) => t,
                None => break,
            };

            if let Token::Keyword("trailer") = tok1 {
                // Parse trailer dictionary to extract /Prev and /Root
                let abs_pos = match base_offset.checked_add(lexer.cursor()) {
                    Some(p) if p < data.len() => p,
                    _ => break,
                };
                let mut parser = Parser::new(&data[abs_pos..]);
                if let Ok(Some(Object::Dictionary(dict))) = parser.parse_object() {
                    if table.trailer.is_none() {
                        let mut owned_map = BTreeMap::new();
                        for (k, v) in &dict {
                            owned_map.insert(k.to_string(), v.clone().into_owned());
                        }
                        table.trailer = Some(owned_map);
                    }
                    if table.trailer_dict.is_none() {
                        let mut trailer_map = BTreeMap::new();
                        for (k, v) in &dict {
                            let val = match v {
                                Object::Reference { id, .. } => id.to_string(),
                                Object::Integer(size) => size.to_string(),
                                Object::Name(n) => n.to_string(),
                                _ => format!("{v:?}"),
                            };
                            trailer_map.insert(k.to_string(), val);
                        }
                        table.trailer_dict = Some(trailer_map);
                    }
                    if let Some(Object::Integer(prev)) = dict.get("Prev") {
                        if *prev > 0 {
                            return Ok(Some(*prev as u64));
                        }
                    }
                }
                break;
            }

            if let Token::Integer(first_id) = tok1 {
                if let Some(Token::Integer(num_entries)) = lexer.next_token()? {
                    // Safety cap: each traditional xref entry is exactly 20 bytes.
                    // A subsection claiming more entries than the file can physically hold
                    // (e.g., num_entries = 3_221_225_472 in a malformed PDF) would OOM.
                    let max_possible = (data.len() / 20).max(1) as i64;
                    if num_entries < 0 || num_entries > max_possible {
                        return Err(Error::SyntaxError {
                            offset: lexer.cursor(),
                            message: "xref subsection entry count exceeds file size",
                        });
                    }
                    if first_id < 0 || first_id > u32::MAX as i64 {
                        return Err(Error::SyntaxError {
                            offset: lexer.cursor(),
                            message: "Invalid first object id in xref subsection",
                        });
                    }
                    let first = first_id as u32;
                    let count = num_entries as u32;
                    for current_id in first..(first.saturating_add(count)) {
                        let offset = match lexer.next_token()? {
                            Some(Token::Integer(val)) if val >= 0 => val as u64,
                            _ => {
                                return Err(Error::SyntaxError {
                                    offset: lexer.cursor(),
                                    message: "Corrupt xref offset",
                                })
                            }
                        };
                        let gen = match lexer.next_token()? {
                            Some(Token::Integer(val)) if val >= 0 && val <= u16::MAX as i64 => {
                                val as u16
                            }
                            _ => {
                                return Err(Error::SyntaxError {
                                    offset: lexer.cursor(),
                                    message: "Corrupt xref generation",
                                })
                            }
                        };
                        let flag = match lexer.next_token()? {
                            Some(Token::Keyword("n")) => true,
                            Some(Token::Keyword("f")) => false,
                            _ => {
                                return Err(Error::SyntaxError {
                                    offset: lexer.cursor(),
                                    message: "Corrupt xref flag",
                                })
                            }
                        };

                        if flag {
                            table.insert(current_id, XRefEntry::InUse { offset, gen });
                        } else {
                            table.insert(
                                current_id,
                                XRefEntry::Free {
                                    next_free_id: u32::try_from(offset).unwrap_or(u32::MAX),
                                    gen,
                                },
                            );
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    /// PDFium-inspired raw forward scanner: scans entire file looking for `N M obj` tokens.
    ///
    /// Delegates directly to `crate::recover::repair` which owns the §3.2 repair algorithm.
    pub fn reconstruct_linear_scan(data: &[u8]) -> Result<Self> {
        crate::recover::repair(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_standard_xref() {
        let sample = b"%PDF-1.4\n\
1 0 obj\n<< /Type /Catalog >>\nendobj\n\
xref\n\
0 2\n\
0000000000 65535 f \r\n\
0000000009 00000 n \r\n\
trailer\n\
<< /Size 2 /Root 1 0 R >>\n\
startxref\n\
45\n\
%%EOF";

        let table = XRefTable::parse_or_reconstruct(sample).unwrap();
        assert_eq!(table.entries.len(), 2);
        assert_eq!(table.get(1), Some(&XRefEntry::InUse { offset: 9, gen: 0 }));
    }

    #[test]
    fn test_corrupted_startxref_linear_reconstruction() {
        // Document with intentional missing startxref and corrupted trailer
        let corrupt = b"%PDF-1.4\n\
1 0 obj\n<< /Type /Catalog >>\nendobj\n\
2 0 obj\n<< /Type /Pages /Count 0 >>\nendobj\n\
%% Corrupted EOF without xref or startxref";

        let table = XRefTable::parse_or_reconstruct(corrupt).unwrap();
        assert!(table.entries.contains_key(&1));
        assert!(table.entries.contains_key(&2));
    }
}
