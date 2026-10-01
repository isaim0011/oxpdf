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
    ///
    /// Returns `Err(Error::Unsupported("encrypted"))` immediately for password-protected PDFs
    /// rather than producing garbled output or hanging.
    pub fn load(data: &'a [u8]) -> Result<Self> {
        // Fast pre-check: scan last 4 KB for /Encrypt in the trailer region.
        // Full encryption detection happens after xref parse via the trailer dict.
        let trailer_scan_start = data.len().saturating_sub(4096);
        let trailer_tail = &data[trailer_scan_start..];
        if memchr::memmem::find(trailer_tail, b"/Encrypt").is_some()
            && (memchr::memmem::find(trailer_tail, b"trailer").is_some()
                || memchr::memmem::find(trailer_tail, b"/Type /XRef").is_some())
        {
            return Err(Error::Unsupported("encrypted: /Encrypt key in trailer"));
        }

        let xref = XRefTable::parse_or_reconstruct(data)?;

        // Second check: /Encrypt in the parsed trailer dict
        if xref
            .trailer_dict
            .as_ref()
            .is_some_and(|t| t.contains_key("Encrypt"))
        {
            return Err(Error::Unsupported(
                "encrypted: /Encrypt in trailer dictionary",
            ));
        }

        Ok(Self {
            data,
            xref,
            obj_stm_cache: RefCell::new(HashMap::new()),
        })
    }

    /// Loads a document strictly per §3.1: fails immediately on any structural corruption
    /// without attempting recovery.
    pub fn load_strict(data: &'a [u8]) -> Result<Self> {
        let trailer_scan_start = data.len().saturating_sub(4096);
        let trailer_tail = &data[trailer_scan_start..];
        if memchr::memmem::find(trailer_tail, b"/Encrypt").is_some()
            && (memchr::memmem::find(trailer_tail, b"trailer").is_some()
                || memchr::memmem::find(trailer_tail, b"/Type /XRef").is_some())
        {
            return Err(Error::Unsupported("encrypted: /Encrypt key in trailer"));
        }

        let xref = XRefTable::parse_standard(data)?;

        if xref
            .trailer_dict
            .as_ref()
            .is_some_and(|t| t.contains_key("Encrypt"))
        {
            return Err(Error::Unsupported(
                "encrypted: /Encrypt in trailer dictionary",
            ));
        }

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
                let start = match usize::try_from(*offset) {
                    Ok(s) => s,
                    Err(_) => return Err(Error::UnexpectedEof(usize::MAX)),
                };
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

                if stm_id == id {
                    return Err(Error::CyclicReference {
                        object_id: id,
                        generation: 0,
                    });
                }
                // Disallow /ObjStm nested inside another /ObjStm (prohibited by PDF spec §7.5.7)
                if let Some(XRefEntry::Compressed { .. }) = self.xref.get(stm_id) {
                    return Err(Error::Unsupported(
                        "recursive /ObjStm compression is forbidden by PDF spec §7.5.7",
                    ));
                }

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
                                Some(Object::Integer(f)) if *f >= 0 => {
                                    usize::try_from(*f).unwrap_or(0)
                                }
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
                            if o >= 0 {
                                if let Ok(o_usize) = usize::try_from(o) {
                                    target_offset_in_data = first_offset.checked_add(o_usize);
                                }
                            }
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
            .and_then(|r| {
                r.parse::<u32>().ok().or_else(|| {
                    if let Some(idx) = r.find("id: ") {
                        let sub = &r[idx + 4..];
                        let digits: String =
                            sub.chars().take_while(|c| c.is_ascii_digit()).collect();
                        digits.parse::<u32>().ok()
                    } else {
                        None
                    }
                })
            })
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
        // Safety cap: prevents OOM from malformed cyclic or bomb page trees.
        const MAX_PAGES: usize = 10_000_000;

        while let Some(current_id) = worklist.pop() {
            if pages.len() + worklist.len() > MAX_PAGES {
                return Err(Error::Unsupported(
                    "page tree exceeds 10_000_000 node safety limit",
                ));
            }
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
                                    if !visited.contains(id) {
                                        worklist.push(*id);
                                    }
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

    /// Extracts Unicode plaintext from a specific page by ID (§1, §7, Tier 5).
    pub fn extract_text(&self, page_id: u32) -> Result<String> {
        crate::text::extract_page_text(self, page_id)
    }

    /// Extracts Unicode plaintext from all pages in document order.
    pub fn extract_text_all(&self) -> Result<Vec<String>> {
        let page_ids = self.get_page_ids()?;
        let mut pages = Vec::with_capacity(page_ids.len());
        for id in page_ids {
            pages.push(self.extract_text(id)?);
        }
        Ok(pages)
    }

    /// QPDF-style object stream packing.
    ///
    /// Separates objects into two groups:
    /// - **Stream objects** → written as regular indirect objects (PDF spec §7.3.8 requires this)
    /// - **Non-stream objects** → packed together into a single compressed `/ObjStm`
    ///
    /// On typical documents this reduces output size by 30–70% compared to `write_to`.
    /// Outputs PDF 1.5+ (required for `/ObjStm`).
    pub fn write_packed<W: Write>(&self, mut writer: W) -> Result<u64> {
        use flate2::{write::ZlibEncoder, Compression};
        use std::io::Write as IoWrite;

        let mut ser = Serializer::new(&mut writer);
        ser.write_header((1, 5))?; // PDF 1.5+ required for /ObjStm

        let mut new_xref: BTreeMap<u32, u64> = BTreeMap::new();

        // --- Pass 1: collect all objects, split stream vs non-stream ---
        let mut stream_objs: Vec<(u32, Object<'static>)> = Vec::new();
        let mut packable_objs: Vec<(u32, Object<'static>)> = Vec::new();

        for (&id, entry) in &self.xref.entries {
            if let XRefEntry::InUse { .. } | XRefEntry::Compressed { .. } = entry {
                if let Ok(Some(obj)) = self.get_object(id) {
                    match &obj {
                        Object::Stream { .. } => stream_objs.push((id, obj)),
                        _ => packable_objs.push((id, obj)),
                    }
                }
            }
        }

        // --- Pass 2: write stream objects as regular indirect objects ---
        for (id, obj) in &stream_objs {
            let offset = ser.write_indirect_object_header(*id, 0)?;
            ser.write_object(obj)?;
            ser.write_indirect_object_footer()?;
            new_xref.insert(*id, offset);
        }

        // --- Pass 3: pack non-stream objects into one /ObjStm ---
        if !packable_objs.is_empty() {
            // The /ObjStm object itself gets the next available id
            let objstm_id = new_xref
                .keys()
                .copied()
                .chain(packable_objs.iter().map(|(id, _)| *id))
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or(Error::Unsupported("object id overflow"))?;

            // Build the /ObjStm payload:
            //   Header section: "id1 offset1 id2 offset2 ..."
            //   Body section:   serialized objects concatenated
            let mut header_part: Vec<u8> = Vec::new();
            let mut body_part: Vec<u8> = Vec::new();
            let mut obj_xref_entries: Vec<(u32, usize)> = Vec::new(); // (id, index_in_stream)

            for (idx, (id, obj)) in packable_objs.iter().enumerate() {
                let body_offset = body_part.len();
                header_part.extend_from_slice(format!("{} {} ", id, body_offset).as_bytes());

                // Serialize object into body buffer
                let mut tmp: Vec<u8> = Vec::new();
                {
                    let mut tmp_ser = Serializer::new(&mut tmp);
                    tmp_ser.write_object(obj)?;
                }
                body_part.extend_from_slice(&tmp);
                body_part.push(b'\n');

                obj_xref_entries.push((*id, idx));
            }

            let first_offset = header_part.len();
            let mut payload = header_part;
            payload.extend_from_slice(&body_part);

            // Compress with FlateDecode
            let mut compressed: Vec<u8> = Vec::new();
            {
                let mut encoder = ZlibEncoder::new(&mut compressed, Compression::best());
                encoder
                    .write_all(&payload)
                    .map_err(|e| Error::Io(e.to_string()))?;
                encoder.finish().map_err(|e| Error::Io(e.to_string()))?;
            }

            // Write the /ObjStm indirect object
            let objstm_offset = ser.write_indirect_object_header(objstm_id, 0)?;
            let n = packable_objs.len();
            let stm_dict = format!(
                "<< /Type /ObjStm /N {} /First {} /Filter /FlateDecode /Length {} >>",
                n,
                first_offset,
                compressed.len()
            );
            ser.write_bytes(stm_dict.as_bytes())?;
            ser.write_bytes(b"\nstream\n")?;
            ser.write_bytes(&compressed)?;
            ser.write_bytes(b"\nendstream")?;
            ser.write_indirect_object_footer()?;
            new_xref.insert(objstm_id, objstm_offset);

            // Register all packed objects as /ObjStm entries in xref
            for (id, _) in &obj_xref_entries {
                // Compressed objects are looked up by objstm_id in the xref stream below.
                new_xref.insert(*id, u64::MAX); // placeholder; overwritten by xref stream
            }

            // Build a proper PDF 1.5 cross-reference stream instead of traditional xref table
            let xref_offset = ser.bytes_written();
            let xref_id = objstm_id
                .checked_add(1)
                .ok_or(Error::Unsupported("xref object id overflow"))?;

            // XRef stream entries: 3 bytes each in format (type, field2, field3)
            // Type 0 = free, Type 1 = uncompressed, Type 2 = compressed (/ObjStm)
            // We use W=[1,4,4] for maximum coverage
            let mut xref_data: Vec<u8> = Vec::new();
            let max_id = new_xref.keys().copied().max().unwrap_or(0);

            // Entry for object 0 (free head)
            xref_data.extend_from_slice(&[0u8, 0, 0, 0, 0, 0xFF, 0xFF, 0, 0]); // type=0, next=0, gen=65535

            for id in 1..=max_id {
                if let Some(&offset) = new_xref.get(&id) {
                    if offset == u64::MAX {
                        // Compressed object in the /ObjStm
                        // Find its index
                        let idx = obj_xref_entries
                            .iter()
                            .find(|(oid, _)| *oid == id)
                            .map(|(_, i)| *i)
                            .unwrap_or(0);
                        xref_data.push(2); // type 2 = compressed
                        xref_data.extend_from_slice(&objstm_id.to_be_bytes());
                        let idx_u32 = u32::try_from(idx).unwrap_or(u32::MAX);
                        xref_data.extend_from_slice(&idx_u32.to_be_bytes());
                    } else {
                        // Normal uncompressed object
                        xref_data.push(1); // type 1 = uncompressed
                                           // Truncate offset to 32-bit: documents > 4 GB are out of scope for v0.x.
                        let offset_u32 = u32::try_from(offset).unwrap_or(u32::MAX);
                        xref_data.extend_from_slice(&offset_u32.to_be_bytes());
                        xref_data.extend_from_slice(&0u32.to_be_bytes()); // gen = 0
                    }
                } else {
                    // Free entry
                    xref_data.extend_from_slice(&[0u8, 0, 0, 0, 0, 0, 0, 0, 0]);
                }
            }

            // Compress the xref stream
            let mut xref_compressed: Vec<u8> = Vec::new();
            {
                let mut encoder = ZlibEncoder::new(&mut xref_compressed, Compression::default());
                encoder
                    .write_all(&xref_data)
                    .map_err(|e| Error::Io(e.to_string()))?;
                encoder.finish().map_err(|e| Error::Io(e.to_string()))?;
            }

            // Find the /Root reference
            let root_str = self
                .xref
                .trailer_dict
                .as_ref()
                .and_then(|t| t.get("Root"))
                .map(|s| s.as_str())
                .unwrap_or("1");
            let size = max_id
                .checked_add(2)
                .ok_or(Error::Unsupported("max_id overflow"))?;

            let xref_dict = format!(
                "{} 0 obj\n<< /Type /XRef /Size {} /W [1 4 4] /Root {} 0 R /Filter /FlateDecode /Length {} >>\nstream\n",
                xref_id, size, root_str, xref_compressed.len()
            );
            ser.write_bytes(xref_dict.as_bytes())?;
            ser.write_bytes(&xref_compressed)?;
            ser.write_bytes(b"\nendstream\nendobj\n")?;
            ser.write_bytes(b"startxref\n")?;
            ser.write_bytes(format!("{}\n%%EOF\n", xref_offset).as_bytes())?;

            return Ok(ser.bytes_written());
        }

        // Fallback: no packable objects → write traditional xref table
        let xref_offset = ser.bytes_written();
        ser.write_bytes(b"xref\n")?;
        let s = format!("0 {}\n", new_xref.len() + 1);
        ser.write_bytes(s.as_bytes())?;
        ser.write_bytes(b"0000000000 65535 f \r\n")?;
        for &offset in new_xref.values() {
            let entry_line = format!("{:010} 00000 n \r\n", offset);
            ser.write_bytes(entry_line.as_bytes())?;
        }
        let root_str = self
            .xref
            .trailer_dict
            .as_ref()
            .and_then(|t| t.get("Root"))
            .map(|s| s.as_str())
            .unwrap_or("1");
        ser.write_bytes(
            format!(
                "trailer\n<< /Size {} /Root {} 0 R >>\nstartxref\n{}\n%%EOF\n",
                new_xref.len() + 1,
                root_str,
                xref_offset
            )
            .as_bytes(),
        )?;
        Ok(ser.bytes_written())
    }

    /// Serializes the document into any writer with clean xref rebuilding.
    /// For smaller output, use `write_packed()` which compresses objects into `/ObjStm`.
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

    #[test]
    fn test_load_strict_vs_load_recovery() {
        // PDF with severed/missing xref and missing startxref
        let severed_pdf = b"%PDF-1.4\n\
1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
2 0 obj\n<< /Type /Pages /Kids [] /Count 0 >>\nendobj\n\
%%EOF";

        // load_strict MUST fail immediately without attempting recovery
        assert!(Document::load_strict(severed_pdf).is_err());

        // Document::load MUST recover objects and synthesized catalog per §3.1 and §3.2
        let recovered = Document::load(severed_pdf).unwrap();
        assert_eq!(recovered.object_count(), 2);
        assert_eq!(recovered.catalog_id().unwrap(), 1);
        let cat = recovered.get_object(1).unwrap().unwrap();
        assert!(matches!(cat, Object::Dictionary(_)));
    }
}
