use oxpdf::error::Error;
use oxpdf::parser::Parser;
use oxpdf::source::{BufferSource, PdfSource};
use oxpdf::stream::{FilterKind, StreamView};
use oxpdf::text::{decode_literal_escapes, extract_page_text};
use oxpdf::types::Object;
use oxpdf::xref::{XRefEntry, XRefTable};
use oxpdf::Document;
use oxpdf::Lexer;

#[test]
fn test_circular_page_tree_termination() {
    // Page tree with circular loop: Page 2 -> Kids [3 0 R], Page 3 -> Kids [2 0 R]
    let pdf = b"%PDF-1.4\n\
1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
3 0 obj\n<< /Type /Pages /Kids [2 0 R] /Count 1 >>\nendobj\n\
xref\n\
0 4\n\
0000000000 65535 f \r\n\
0000000009 00000 n \r\n\
0000000058 00000 n \r\n\
0000000115 00000 n \r\n\
trailer\n\
<< /Size 4 /Root 1 0 R >>\n\
startxref\n\
172\n\
%%EOF";

    let doc = Document::load(pdf).unwrap();
    // Must terminate cleanly without infinite loop or stack overflow
    let page_ids = doc.get_page_ids().unwrap();
    assert!(page_ids.is_empty());
}

#[test]
fn test_self_referential_page_node() {
    // Node pointing to itself in /Kids
    let pdf = b"%PDF-1.4\n\
1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
2 0 obj\n<< /Type /Pages /Kids [2 0 R] /Count 1 >>\nendobj\n\
xref\n\
0 3\n\
0000000000 65535 f \r\n\
0000000009 00000 n \r\n\
0000000058 00000 n \r\n\
trailer\n\
<< /Size 3 /Root 1 0 R >>\n\
startxref\n\
115\n\
%%EOF";

    let doc = Document::load(pdf).unwrap();
    let page_ids = doc.get_page_ids().unwrap();
    assert!(page_ids.is_empty());
}

#[test]
fn test_deeply_nested_arrays_recursion_limit() {
    // 300 nested open brackets: [[[[...
    let input = vec![b'['; 300];
    let mut parser = Parser::new(&input);
    let res = parser.parse_object();
    assert!(matches!(res, Err(Error::RecursionLimitExceeded(_))));
}

#[test]
fn test_deeply_nested_dicts_recursion_limit() {
    // Nesting dictionaries: << /A << /B << ...
    let mut input = Vec::new();
    for i in 0..300 {
        input.extend_from_slice(format!("<< /K{} ", i).as_bytes());
    }
    let mut parser = Parser::new(&input);
    let res = parser.parse_object();
    assert!(matches!(res, Err(Error::RecursionLimitExceeded(_))));
}

#[test]
fn test_circular_compressed_object_stream_prevention() {
    // Obj 10 is compressed inside Obj 10 (self-referential)
    let mut doc_bytes = Vec::new();
    doc_bytes.extend_from_slice(b"%PDF-1.5\n");
    let xref_offset = doc_bytes.len();
    doc_bytes.extend_from_slice(
        b"xref\n0 1\n0000000000 65535 f \r\ntrailer\n<< /Size 11 >>\nstartxref\n",
    );
    doc_bytes.extend_from_slice(format!("{}\n%%EOF", xref_offset).as_bytes());

    let mut doc = Document::load(&doc_bytes).unwrap();
    doc.xref.insert(
        10,
        XRefEntry::Compressed {
            stream_obj_id: 10,
            index: 0,
        },
    );

    // Calling get_object(10) must return an error (CyclicReference or Unsupported), NEVER hang or crash
    let res = doc.get_object(10);
    assert!(res.is_err());
}

#[test]
fn test_chained_compressed_object_stream_forbidden() {
    // Obj 10 is compressed in Obj 11; Obj 11 is compressed in Obj 12 (prohibited by PDF spec §7.5.7)
    let mut doc_bytes = Vec::new();
    doc_bytes.extend_from_slice(b"%PDF-1.5\n");
    let xref_offset = doc_bytes.len();
    doc_bytes.extend_from_slice(
        b"xref\n0 1\n0000000000 65535 f \r\ntrailer\n<< /Size 13 >>\nstartxref\n",
    );
    doc_bytes.extend_from_slice(format!("{}\n%%EOF", xref_offset).as_bytes());

    let mut doc = Document::load(&doc_bytes).unwrap();
    doc.xref.insert(
        10,
        XRefEntry::Compressed {
            stream_obj_id: 11,
            index: 0,
        },
    );
    doc.xref.insert(
        11,
        XRefEntry::Compressed {
            stream_obj_id: 12,
            index: 0,
        },
    );

    let res = doc.get_object(10);
    assert!(matches!(res, Err(Error::Unsupported(_))));
}

#[test]
fn test_malformed_startxref_past_eof() {
    let pdf = b"%PDF-1.4\n\
1 0 obj\n<< /Type /Catalog >>\nendobj\n\
startxref\n\
99999999999999999\n\
%%EOF";

    let res = XRefTable::find_startxref_offset(pdf);
    assert!(res.is_err());
}

#[test]
fn test_buffer_source_overflow_offset_len() {
    let buf = b"abcdefg";
    let src = BufferSource::new(buf);
    // offset + len overflows usize
    let res = src.read_at(usize::MAX - 2, 5);
    assert!(matches!(res, Err(Error::TruncatedFile { .. })));
}

#[test]
fn test_lexer_read_stream_payload_huge_explicit_len() {
    let data = b"stream\nPayload bytes\nendstream";
    let mut lexer = Lexer::new(data);
    let tok = lexer.next_token().unwrap();
    assert_eq!(tok, Some(oxpdf::Token::Keyword("stream")));
    // Explicit length is usize::MAX (adversarial /Length)
    let res = lexer.read_stream_payload(Some(usize::MAX));
    // Must gracefully fall back to endstream search, not panic on slice index or overflow
    assert!(res.is_ok());
    assert_eq!(res.unwrap(), b"Payload bytes");
}

#[test]
fn test_octal_escape_overflow_debug_mode() {
    // Octal values >= 256 (0o400 = 256, 0o777 = 511)
    // In unhardened code, oct_val was u8 and (oct_val << 3) panicked with overflow in debug mode!
    let data = b"(\\400 \\777)";
    let mut lexer = Lexer::new(data);
    let tok = lexer.next_token().unwrap().unwrap();
    match tok {
        oxpdf::Token::String(bytes) => {
            // High-order overflow ignored per ISO 32000-1 §7.3.4.2:
            // 0o400 = 256 -> 256 & 0xFF = 0x00
            // 0o777 = 511 -> 511 & 0xFF = 0xFF
            assert_eq!(bytes.as_slice(), &[0x00, b' ', 0xFF]);
        }
        _ => panic!("Expected Token::String"),
    }

    // Also test text decode_literal_escapes
    let escaped = b"\\400 \\777";
    let decoded = decode_literal_escapes(escaped);
    assert_eq!(decoded, vec![0x00, b' ', 0xFF]);
}

#[test]
fn test_ascii85_decompression_bomb_cap() {
    // Sequence of 'z' characters expands by 4x.
    // 70MB of 'z' would expand to 280MB of zeros, exceeding MAX_DECOMPRESS_BYTES (256MB).
    // Let's create an Ascii85 stream that attempts to produce > 256MB.
    let massive_z = vec![b'z'; 68 * 1024 * 1024]; // 68MB of 'z' -> 272MB of zeroes
    let stream = StreamView::new(&massive_z).with_filter(FilterKind::Ascii85Decode);
    let res = stream.decode();
    assert!(matches!(res, Err(Error::Unsupported(_))));
}

#[test]
fn test_ascii_hex_decompression_limit() {
    // Raw data exceeding 256MB
    let large_raw = vec![b'A'; StreamView::MAX_DECOMPRESS_BYTES + 10];
    let stream = StreamView::new(&large_raw);
    let res = stream.decode();
    assert!(matches!(res, Err(Error::Unsupported(_))));
}

#[test]
fn test_indirect_reference_bounds_sanitization() {
    // Negative object id: `-1 0 R`
    let input = b"-1 0 R";
    let mut parser = Parser::new(input);
    let obj = parser.parse_object().unwrap().unwrap();
    // Must NOT parse as Object::Reference with id: 4294967295!
    assert_eq!(obj, Object::Integer(-1));
}

#[test]
fn test_text_extraction_cyclic_contents_stream() {
    // Page dictionary pointing to cyclic Contents stream (Obj 10 -> 10 0 R)
    let pdf = b"%PDF-1.4\n\
1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
3 0 obj\n<< /Type /Page /Parent 2 0 R /Contents 10 0 R >>\nendobj\n\
10 0 obj\n10 0 R\nendobj\n\
xref\n\
0 5\n\
0000000000 65535 f \r\n\
0000000009 00000 n \r\n\
0000000058 00000 n \r\n\
0000000115 00000 n \r\n\
0000000192 00000 n \r\n\
trailer\n\
<< /Size 5 /Root 1 0 R >>\n\
startxref\n\
218\n\
%%EOF";

    let doc = Document::load(pdf).unwrap();
    let res = extract_page_text(&doc, 3);
    // Must return Err(CyclicReference) without infinite recursion
    assert!(matches!(res, Err(Error::CyclicReference { .. })));
}

#[test]
fn test_content_parser_recursion_limit() {
    // Deeply nested array inside content stream
    let mut nested = Vec::new();
    for _ in 0..100 {
        nested.push(b'[');
    }
    let mut parser = oxpdf::content::ContentParser::new(&nested);
    let res = parser.parse();
    assert!(matches!(res, Err(Error::RecursionLimitExceeded(_))));
}
