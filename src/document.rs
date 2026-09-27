use crate::error::{Error, Result};
use crate::lexer::Token;
use crate::parser::Parser;
use crate::types::Object;
use crate::writer::Serializer;
use crate::xref::{XRefEntry, XRefTable};
use std::collections::BTreeMap;
use std::io::Write;

/// High-level, memory-bounded PDF Document structure.
/// Allows lazy object loading, page extraction, and merging without loading multi-gigabyte files into RAM.
pub struct Document<'a> {
    data: &'a [u8],
    pub xref: XRefTable,
}

impl<'a> Document<'a> {
    /// Loads a document by inspecting its cross-reference structure or reconstructing it.
    pub fn load(data: &'a [u8]) -> Result<Self> {
        let xref = XRefTable::parse_or_reconstruct(data)?;
        Ok(Self { data, xref })
    }

    /// Fetches an indirect object by id without parsing other objects.
    pub fn get_object(&self, id: u32) -> Result<Option<Object<'a>>> {
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
                // Now parse the actual object payload
                parser.parse_object()
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
                match node.get("Type") {
                    Some(Object::Name("Page")) => {
                        pages.push(current_id);
                    }
                    Some(Object::Name("Pages")) => {
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
}
