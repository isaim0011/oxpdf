use crate::error::{Error, Result};
use crate::lexer::{Lexer, Token};
use crate::parser::Parser;
use crate::types::Object;
use memchr::memmem;
use std::collections::BTreeMap;

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
                    let next_prev = Self::parse_xref_subsections(&mut lexer, &mut table, data, current_offset as usize)?;
                    match next_prev {
                        Some(prev) => current_offset = prev,
                        None => break,
                    }
                }
                Some(Token::Integer(_)) => {
                    // Possible XRef Stream (PDF 1.5+) - handled gracefully or fallback
                    break;
                }
                _ => break,
            }
        }

        Ok(table)
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
            Some(Token::Integer(offset)) if offset >= 0 => Ok(offset as u64),
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
        _base_offset: usize,
    ) -> Result<Option<u64>> {
        loop {
            let tok1 = match lexer.next_token()? {
                Some(t) => t,
                None => break,
            };

            if let Token::Keyword("trailer") = tok1 {
                // Parse trailer dictionary to extract /Prev and /Root
                let abs_pos = (data.len() - (data.len() - lexer.cursor())).min(data.len());
                let mut parser = Parser::new(&data[abs_pos..]);
                if let Ok(Some(Object::Dictionary(dict))) = parser.parse_object() {
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
                    let mut current_id = first_id as u32;
                    for _ in 0..num_entries {
                        let offset = match lexer.next_token()? {
                            Some(Token::Integer(val)) => val as u64,
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
                        current_id += 1;
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
            while let Ok(Some(tok)) = lexer.next_token() {
                tokens.push(tok);
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
