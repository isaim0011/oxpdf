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
        // Fallback: Chromium PDFium-style resilient linear reconstructor
        Self::reconstruct_linear_scan(data)
    }

    /// Standard backward resolution from `startxref`.
    pub fn parse_standard(data: &[u8]) -> Result<Self> {
        let startxref_offset = Self::find_startxref_offset(data)?;
        let mut table = Self::new();
        let mut visited_offsets = std::collections::HashSet::new();
        let mut current_offset = startxref_offset;

        while current_offset < data.len() as u64 && !visited_offsets.contains(&current_offset) {
            visited_offsets.insert(current_offset);
            let mut lexer = Lexer::new(&data[current_offset as usize..]);

            match lexer.next_token()? {
                Some(Token::Keyword("xref")) => {
                    let next_prev = Self::parse_xref_subsections(
                        &mut lexer,
                        &mut table,
                        data,
                        current_offset as usize,
                    )?;
                    match next_prev {
                        Some(prev) => current_offset = prev,
                        None => break,
                    }
                }
                Some(Token::Integer(_)) => {
                    // PDF 1.5+ XRef Stream: `N G obj << /Type /XRef ... >> stream ... endstream`
                    // The startxref offset points to the object id, not to an `xref` keyword.
                    match Self::parse_xref_stream(data, current_offset as usize, &mut table) {
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
    fn parse_xref_stream(
        data: &[u8],
        offset: usize,
        table: &mut XRefTable,
    ) -> Result<Option<u64>> {
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

        let decoded: Vec<u8> = if is_flate {
            let view = crate::stream::StreamView::new(&raw_data)
                .with_filter(crate::stream::FilterKind::FlateDecode);
            view.decode()?
        } else {
            raw_data.to_vec()
        };

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
        let w0 = match &w_arr[0] { Object::Integer(i) => *i as usize, _ => 0 };
        let w1 = match &w_arr[1] { Object::Integer(i) => *i as usize, _ => 0 };
        let w2 = match &w_arr[2] { Object::Integer(i) => *i as usize, _ => 0 };
        let entry_size = w0 + w1 + w2;
        if entry_size == 0 {
            return Err(Error::SyntaxError {
                offset,
                message: "XRef stream has zero entry size",
            });
        }

        // Read /Index [start count start count ...] — defaults to [0, /Size]
        let size = match dict.get("Size") {
            Some(Object::Integer(s)) => *s as u32,
            _ => 0,
        };
        let index_ranges: Vec<(u32, u32)> = match dict.get("Index") {
            Some(Object::Array(arr)) if arr.len() >= 2 => {
                arr.chunks(2)
                    .filter_map(|chunk| {
                        if let (Object::Integer(start), Object::Integer(count)) = (&chunk[0], &chunk[1]) {
                            Some((*start as u32, *count as u32))
                        } else {
                            None
                        }
                    })
                    .collect()
            }
            _ => vec![(0, size)],
        };

        // Decode entries
        let mut pos = 0usize;
        for (start_id, count) in &index_ranges {
            for i in 0..*count {
                if pos + entry_size > decoded.len() {
                    break;
                }
                let obj_id = start_id + i;

                // Read field 1: type
                let field_type = if w0 == 0 {
                    1u64 // default type is 1 when w0=0
                } else {
                    read_be_u64(&decoded[pos..pos + w0])
                };
                // Read field 2
                let field2 = if w1 == 0 { 0u64 } else { read_be_u64(&decoded[pos + w0..pos + w0 + w1]) };
                // Read field 3
                let field3 = if w2 == 0 { 0u64 } else { read_be_u64(&decoded[pos + w0 + w1..pos + entry_size]) };

                // Only insert if not already present (earlier xref takes precedence for incremental updates)
                if !table.entries.contains_key(&obj_id) {
                    match field_type {
                        0 => {
                            table.insert(obj_id, XRefEntry::Free {
                                next_free_id: field2 as u32,
                                gen: field3 as u16,
                            });
                        }
                        1 => {
                            table.insert(obj_id, XRefEntry::InUse {
                                offset: field2,
                                gen: field3 as u16,
                            });
                        }
                        2 => {
                            table.insert(obj_id, XRefEntry::Compressed {
                                stream_obj_id: field2 as u32,
                                index: field3 as u16,
                            });
                        }
                        _ => {} // Unknown type — skip
                    }
                }
                pos += entry_size;
            }
        }

        // Extract /Root and /Info for trailer_dict (only on first/latest xref)
        if table.trailer_dict.is_none() {
            let mut trailer_map = std::collections::BTreeMap::new();
            if let Some(Object::Reference { id, .. }) = dict.get("Root") {
                trailer_map.insert("Root".to_string(), id.to_string());
            }
            if let Some(Object::Integer(s)) = dict.get("Size") {
                trailer_map.insert("Size".to_string(), s.to_string());
            }
            if !trailer_map.is_empty() {
                table.trailer_dict = Some(trailer_map);
            }
        }

        // Follow /Prev chain for incremental updates
        match dict.get("Prev") {
            Some(Object::Integer(prev)) if *prev > 0 && (*prev as usize) < data.len() => {
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
            Some(Token::Integer(offset)) if offset >= 0 && (offset as usize) < data.len() => {
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
                let abs_pos = base_offset + lexer.cursor();
                let mut parser = Parser::new(&data[abs_pos..]);
                if let Ok(Some(Object::Dictionary(dict))) = parser.parse_object() {
                    let mut trailer_map = BTreeMap::new();
                    if let Some(Object::Reference { id, .. }) = dict.get("Root") {
                        trailer_map.insert("Root".to_string(), id.to_string());
                    }
                    if let Some(Object::Integer(size)) = dict.get("Size") {
                        trailer_map.insert("Size".to_string(), size.to_string());
                    }
                    if table.trailer_dict.is_none() {
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
                    let first = first_id.max(0) as u32;
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
                            Some(Token::Integer(val)) => val as u16,
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
                                    next_free_id: offset as u32,
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
    /// Used when the trailer is severed, xref table is corrupt, or `/Prev` loop causes recursion failure.
    pub fn reconstruct_linear_scan(data: &[u8]) -> Result<Self> {
        let mut table = Self::new();
        let obj_finder = memmem::Finder::new(b"obj");

        for match_idx in obj_finder.find_iter(data) {
            // Check preceding tokens backwards: need `<id> <gen> obj`
            let lookback_start = match_idx.saturating_sub(64);
            let slice = &data[lookback_start..match_idx];

            let mut lexer = Lexer::new(slice);
            let mut tokens = Vec::new();
            // Cap at 128 tokens — a 64-byte lookback slice can only produce ~30 tokens
            // in normal PDF. The cap is defense-in-depth against lexer runaway.
            while let Ok(Some(tok)) = lexer.next_token() {
                tokens.push(tok);
                if tokens.len() > 128 {
                    break;
                }
            }

            if tokens.len() >= 2 {
                if let (Token::Integer(id), Token::Integer(gen)) =
                    (&tokens[tokens.len() - 2], &tokens[tokens.len() - 1])
                {
                    if *id > 0 && *gen >= 0 {
                        // Compute true byte offset of start of `<id>`
                        table.insert(
                            *id as u32,
                            XRefEntry::InUse {
                                offset: lookback_start as u64,
                                gen: *gen as u16,
                            },
                        );
                    }
                }
            }
        }

        Ok(table)
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
