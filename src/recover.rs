use crate::error::{Error, Result};
use crate::lexer::{Lexer, Token};
use crate::parser::Parser;
use crate::types::Object;
use crate::xref::{XRefEntry, XRefTable};
use memchr::memmem;
use std::collections::HashMap;

/// Performs a fault-tolerant repair pass over corrupted or severed PDF documents.
///
/// Implements §3.2 of the specification (PDFium/qpdf standard recovery algorithm):
/// 1. Linear scan the entire byte stream for `\d+\s+\d+\s+obj`.
/// 2. For every match, record `(object_id, generation, offset)` where later occurrences override earlier ones.
/// 3. Rebuild an in-memory XRefTable.
/// 4. Locate the `trailer` keyword and dictionary; if missing, locate `/Type /Catalog` and synthesize
///    a minimal trailer dictionary pointing at it.
/// 5. If recovery is completely exhausted, return `Error::RecoveryFailed`.
pub fn repair(data: &[u8]) -> Result<XRefTable> {
    let mut table = XRefTable::new();
    let obj_finder = memmem::Finder::new(b"obj");

    // Map of object_id -> (generation, exact_offset)
    let mut object_offsets: HashMap<u32, (u16, u64)> = HashMap::new();

    for match_idx in obj_finder.find_iter(data) {
        // Ensure "obj" is not part of a larger identifier (must be preceded by delimiter or whitespace)
        if match_idx > 0 && !data[match_idx - 1].is_ascii_whitespace() {
            continue;
        }
        // Ensure "obj" is followed by whitespace or delimiter
        if match_idx + 3 < data.len() {
            let next_b = data[match_idx + 3];
            if !next_b.is_ascii_whitespace()
                && next_b != b'<'
                && next_b != b'['
                && next_b != b'/'
                && next_b != b'%'
            {
                continue;
            }
        }

        // Look back up to 64 bytes to locate "<id> <gen> obj"
        let lookback_start = match_idx.saturating_sub(64);
        let slice = &data[lookback_start..match_idx];

        let mut lexer = Lexer::new(slice);
        let mut tokens_with_pos = Vec::new();

        // Cap at 128 tokens as defense-in-depth against lexer runaway
        while let Ok(Some(tok)) = lexer.next_token() {
            tokens_with_pos.push((tok, lexer.cursor()));
            if tokens_with_pos.len() > 128 {
                break;
            }
        }

        if tokens_with_pos.len() >= 2 {
            let (tok_id, _) = &tokens_with_pos[tokens_with_pos.len() - 2];
            let (tok_gen, _) = &tokens_with_pos[tokens_with_pos.len() - 1];

            if let (Token::Integer(id), Token::Integer(gen)) = (tok_id, tok_gen) {
                if *id > 0 && *gen >= 0 && *gen <= u16::MAX as i64 {
                    let obj_id = *id as u32;
                    let gen_u16 = *gen as u16;

                    // Calculate true start offset of the object definition (the start of `id`)
                    let id_str = id.to_string();
                    let relative_id_start = slice
                        .windows(id_str.len())
                        .rposition(|w| w == id_str.as_bytes())
                        .unwrap_or(0);
                    let true_offset = (lookback_start + relative_id_start) as u64;

                    // §3.2 rule 3: Last occurrence of a given (id, generation) wins
                    object_offsets.insert(obj_id, (gen_u16, true_offset));
                }
            }
        }
    }

    if object_offsets.is_empty() {
        return Err(Error::RecoveryFailed { attempts: 1 });
    }

    // Populate recovered xref table
    for (&id, &(gen, offset)) in &object_offsets {
        table.insert(id, XRefEntry::InUse { offset, gen });
    }

    // Step 5: Locate trailer keyword or synthesize catalog trailer
    let mut trailer_dict: Option<std::collections::BTreeMap<String, String>> = None;
    let trailer_finder = memmem::Finder::new(b"trailer");

    // Scan backwards from EOF for "trailer" keyword
    for t_idx in trailer_finder.find_iter(data) {
        let after_trailer = &data[t_idx + 7..];
        let mut parser = Parser::new(after_trailer);
        if let Ok(Some(Object::Dictionary(dict))) = parser.parse_object() {
            let mut string_dict = std::collections::BTreeMap::new();
            for (k, v) in dict {
                string_dict.insert(k.into_owned(), format!("{v:?}"));
            }
            trailer_dict = Some(string_dict);
            // Later trailer (closer to EOF) wins in incremental update model
        }
    }

    // If no valid trailer dictionary was found, locate /Type /Catalog directly
    if trailer_dict.is_none() {
        let catalog_finder = memmem::Finder::new(b"/Catalog");
        for cat_match in catalog_finder.find_iter(data) {
            // Find which object encloses this /Catalog marker
            let enclosing_obj = object_offsets
                .iter()
                .filter(|(_, (_, offset))| (*offset as usize) < cat_match)
                .max_by_key(|(_, (_, offset))| *offset);

            if let Some((&cat_id, _)) = enclosing_obj {
                let mut synth_trailer = std::collections::BTreeMap::new();
                synth_trailer.insert("Root".to_string(), cat_id.to_string());
                synth_trailer.insert("Size".to_string(), (table.entries.len() + 1).to_string());
                trailer_dict = Some(synth_trailer);
                break;
            }
        }
    }

    table.trailer_dict = trailer_dict;
    Ok(table)
}
