use crate::error::{Error, Result};
use crate::lexer::Token;
use crate::parser::Parser;
use crate::types::Object;
use crate::writer::Serializer;
use crate::xref::{XRefEntry, XRefTable};
use std::collections::BTreeMap;
use std::io::Write;

use std::cell::RefCell;
use std::collections::HashMap;

/// High-level, memory-bounded PDF Document structure.
/// Allows lazy object loading, page extraction, and merging without loading multi-gigabyte files into RAM.
pub struct Document<'a> {
    data: &'a [u8],
    pub xref: XRefTable,
    obj_stm_cache: RefCell<HashMap<u32, (usize, Vec<u8>)>>,
}

impl<'a> Document<'a> {
    /// Loads a document by inspecting its cross-reference structure or reconstructing it.
    pub fn load(data: &'a [u8]) -> Result<Self> {
        let xref = XRefTable::parse_or_reconstruct(data)?;
        Ok(Self {
            data,
            xref,
            obj_stm_cache: RefCell::new(HashMap::new()),
        })
    }

    /// Fetches an indirect object by id without parsing other objects.
    /// If the object is stored directly in the document, returns a zero-copy borrowed `Object<'static>`.
    pub fn get_object(&self, id: u32) -> Result<Option<Object<'static>>> {
        match self.xref.get(id) {
            Some(XRefEntry::InUse { offset, .. }) => {
                let start = *offset as usize;
                if start >= self.data.len() {
                    return Err(Error::UnexpectedEof(start));
                }

                let mut parser = Parser::new(&self.data[start..]);
                // Consume `<id> <gen> obj` header tokens via lexer
                let lexer = parser.lexer_mut();
                let _id_tok = lexer.next_token()?;
                let _gen_tok = lexer.next_token()?;
                let obj_tok = lexer.next_token()?;
                match obj_tok {
                    Some(Token::Keyword("obj")) => {}
                    _ => {
                        return Err(Error::SyntaxError {
                            offset: start,
                            message: "Expected 'obj' keyword after object id and generation",
                        })
                    }
                }
                // Parse the actual object payload and convert into owned for unified lifetime
                let obj = parser.parse_object()?;
                Ok(obj.map(|o| o.into_owned()))
            }
            Some(XRefEntry::Compressed {
                stream_obj_id,
                index,
            }) => {
                let stm_id = *stream_obj_id;
                let idx = *index;

                // Check cache first or populate it
                let has_cached = self.obj_stm_cache.borrow().contains_key(&stm_id);
                if !has_cached {
                    // Fetch the parent /ObjStm object stream
                    let (first_offset, decompressed) = match self.get_object(stm_id)? {
                        Some(Object::Stream { dict, data }) => {
                            let is_flate = dict
                                .get("Filter")
                                .and_then(|f| f.as_name())
                                .map(|name| name == "FlateDecode")
                                .unwrap_or(false);

                            let dec = if is_flate {
                                let view = crate::stream::StreamView::new(&data)
                                    .with_filter(crate::stream::FilterKind::FlateDecode);
                                view.decode()?
                            } else {
                                data.to_vec()
                            };

                            let first = match dict.get("First") {
                                Some(Object::Integer(f)) if *f >= 0 => *f as usize,
                                _ => 0,
                            };
                            (first, dec)
                        }
                        _ => return Ok(None),
                    };

                    self.obj_stm_cache
                        .borrow_mut()
                        .insert(stm_id, (first_offset, decompressed));
                }

                let cache = self.obj_stm_cache.borrow();
                let (first_offset, decompressed) = match cache.get(&stm_id) {
                    Some((f, d)) => (*f, d.as_slice()),
                    None => return Ok(None),
                };

                let mut header_lexer = crate::lexer::Lexer::new(decompressed);
                let mut target_offset_in_data = None;

                for i in 0..=idx {
                    let _oid = header_lexer.next_token()?;
                    let offset_tok = header_lexer.next_token()?;
                    if i == idx {
                        if let Some(Token::Integer(o)) = offset_tok {
                            target_offset_in_data = Some(first_offset + o as usize);
                        }
                        break;
                    }
                }

                if let Some(byte_pos) = target_offset_in_data {
                    if byte_pos < decompressed.len() {
                        let mut obj_parser = Parser::new(&decompressed[byte_pos..]);
                        let obj = obj_parser.parse_object()?;
                        return Ok(obj.map(|o| o.into_owned()));
                    }
                }

                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// Returns the total number of objects in the cross-reference table.
    pub fn object_count(&self) -> usize {
        self.xref.entries.len()
    }

    /// Finds the Root Catalog dictionary object id.
    pub fn catalog_id(&self) -> Option<u32> {
        self.xref
            .trailer_dict
            .as_ref()
            .and_then(|t| t.get("Root"))
            .and_then(|r| r.parse::<u32>().ok())
    }

    /// Recursively flattens the page tree (/Pages -> /Kids) using a loop-guarded worklist.
    pub fn get_page_ids(&self) -> Result<Vec<u32>> {
        let catalog_id = match self.catalog_id() {
            Some(id) => id,
            None => return Ok(Vec::new()),
        };

        let catalog_obj = match self.get_object(catalog_id)? {
            Some(Object::Dictionary(d)) => d,
            _ => return Ok(Vec::new()),
        };

        let pages_id = match catalog_obj.get("Pages") {
            Some(Object::Reference { id, .. }) => *id,
            _ => return Ok(Vec::new()),
        };

        let mut pages = Vec::new();
        let mut worklist = vec![pages_id];
        let mut visited = std::collections::HashSet::new();

        while let Some(current_id) = worklist.pop() {
            if visited.contains(&current_id) {
                // Guard against cyclic page trees (Pillar 4)
                continue;
            }
            visited.insert(current_id);

            if let Some(Object::Dictionary(node)) = self.get_object(current_id)? {
                match node.get("Type").and_then(|t| t.as_name()) {
                    Some("Page") => {
                        pages.push(current_id);
                    }
                    Some("Pages") => {
                        if let Some(Object::Array(kids)) = node.get("Kids") {
                            // Reverse to maintain natural document page order
                            for kid in kids.iter().rev() {
                                if let Object::Reference { id, .. } = kid {
                                    worklist.push(*id);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(pages)
    }

    /// Returns the resolved page count of the document.
    pub fn page_count(&self) -> Result<usize> {
        self.get_page_ids().map(|p| p.len())
    }

    /// Linearizes and serializes the document into any writer target with clean xref rebuilding.
    pub fn write_to<W: Write>(&self, mut writer: W) -> Result<u64> {
        let mut ser = Serializer::new(&mut writer);
        ser.write_header((1, 7))?;

        let mut new_xref = BTreeMap::new();

        for (&id, entry) in &self.xref.entries {
            if let XRefEntry::InUse { .. } = entry {
                if let Ok(Some(obj)) = self.get_object(id) {
                    let offset = ser.write_indirect_object_header(id, 0)?;
                    ser.write_object(&obj)?;
                    ser.write_indirect_object_footer()?;
                    new_xref.insert(id, offset);
                }
            }
        }

        // Write xref table
        let xref_offset = ser.bytes_written();
        ser.write_bytes(b"xref\n")?;
        let s = format!("0 {}\n", new_xref.len() + 1);
        ser.write_bytes(s.as_bytes())?;
        ser.write_bytes(b"0000000000 65535 f \r\n")?;

        for &offset in new_xref.values() {
            let entry_line = format!("{:010} 00000 n \r\n", offset);
            ser.write_bytes(entry_line.as_bytes())?;
        }

        // Write trailer
        ser.write_bytes(b"trailer\n<< /Size ")?;
        let size_str = format!("{} >>\n", new_xref.len() + 1);
        ser.write_bytes(size_str.as_bytes())?;
        ser.write_bytes(b"startxref\n")?;
        let startxref_str = format!("{}\n%%EOF\n", xref_offset);
        ser.write_bytes(startxref_str.as_bytes())?;

        Ok(ser.bytes_written())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_load_and_object_count() {
        let sample = b"%PDF-1.4\n\
1 0 obj\n<< /Type /Catalog >>\nendobj\n\
2 0 obj\n<< /Type /Pages /Count 0 >>\nendobj\n\
xref\n\
0 3\n\
0000000000 65535 f \r\n\
0000000009 00000 n \r\n\
0000000045 00000 n \r\n\
trailer\n\
<< /Size 3 /Root 1 0 R >>\n\
startxref\n\
89\n\
%%EOF";

        let doc = Document::load(sample).unwrap();
        assert!(doc.object_count() >= 2);
    }

    #[test]
    fn test_document_page_tree_and_cycle_guard() {
        let sample = b"%PDF-1.4\n\
1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
3 0 obj\n<< /Type /Page /Parent 2 0 R >>\nendobj\n\
xref\n\
0 4\n\
0000000000 65535 f \r\n\
0000000009 00000 n \r\n\
0000000058 00000 n \r\n\
0000000115 00000 n \r\n\
trailer\n\
<< /Size 4 /Root 1 0 R >>\n\
startxref\n\
162\n\
%%EOF";

        let doc = Document::load(sample).unwrap();
        assert_eq!(doc.page_count().unwrap(), 1);
        assert_eq!(doc.get_page_ids().unwrap(), vec![3]);
    }

    #[test]
    fn test_compressed_object_stream() {
        // Parent object 10: /Type /ObjStm with two embedded objects (id 11 and id 12)
        // Header contains: 11 0 12 11 (id 11 at offset 0, id 12 at offset 11)
        // First offset: 9
        // Body: 11 0 /FirstObject 12 11 /SecondObject
        // Decompressed: b"11 0 12 14 /FirstObject /SecondObject"
        let stream_payload = b"11 0 12 13 /FirstObject /SecondObject";
        let mut doc_bytes = Vec::new();
        doc_bytes.extend_from_slice(b"%PDF-1.5\n");
        let objstm_offset = doc_bytes.len();
        doc_bytes.extend_from_slice(
            b"10 0 obj\n<< /Type /ObjStm /N 2 /First 10 /Length 37 >>\nstream\n",
        );
        doc_bytes.extend_from_slice(stream_payload);
        doc_bytes.extend_from_slice(b"\nendstream\nendobj\n");
        let xref_offset = doc_bytes.len();
        doc_bytes.extend_from_slice(
            b"xref\n0 1\n0000000000 65535 f \r\ntrailer\n<< /Size 11 >>\nstartxref\n",
        );
        doc_bytes.extend_from_slice(format!("{}\n%%EOF", xref_offset).as_bytes());

        let mut doc = Document::load(&doc_bytes).unwrap();
        // Insert XRef entries: Object 10 is InUse; Object 11 and 12 are Compressed in Object 10
        doc.xref.insert(
            10,
            XRefEntry::InUse {
                offset: objstm_offset as u64,
                gen: 0,
            },
        );
        doc.xref.insert(
            11,
            XRefEntry::Compressed {
                stream_obj_id: 10,
                index: 0,
            },
        );
        doc.xref.insert(
            12,
            XRefEntry::Compressed {
                stream_obj_id: 10,
                index: 1,
            },
        );

        let obj11 = doc.get_object(11).unwrap().unwrap();
        assert_eq!(obj11, Object::name("FirstObject"));

        let obj12 = doc.get_object(12).unwrap().unwrap();
        assert_eq!(obj12, Object::name("SecondObject"));
    }
}
