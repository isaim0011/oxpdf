use crate::error::{Error, Result};
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
                // Consume `<id> <gen> obj` header
                let _id_tok = parser.parse_object()?;
                let _gen_tok = parser.parse_object()?;
                // In a PDF file, next is the keyword `obj`, followed by the actual payload object
                parser.parse_object()
            }
            _ => Ok(None),
        }
    }

    /// Returns the total number of objects in the cross-reference table.
    pub fn object_count(&self) -> usize {
        self.xref.entries.len()
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
}
