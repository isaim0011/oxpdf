use oxpdf::content::ContentParser;
use oxpdf::crypto::{
    aes_128_cbc_decrypt, aes_256_cbc_decrypt, Rc4, StandardSecurityHandler,
};
use oxpdf::filter::{decode_ccitt_fax, decode_jbig2, CcittParams, Jbig2Params};
use oxpdf::font::{CffFont, EmbeddedFont, TrueTypeFont};
use oxpdf::geom::{GraphicsStateTracker, Matrix, Point, Rect};
use oxpdf::parser::Parser;
use oxpdf::types::Object;
use oxpdf::{CMap, Document, FilterKind, Lexer, StreamView};

/// Fuzzing logic for the Lexer (§1 L1 zero-copy token stream).
/// Repeatedly calls `next_token()` until EOF (`None`) or parse error (`Err`).
/// Invariant: Must never panic on arbitrary byte sequences.
pub fn fuzz_lexer(data: &[u8]) {
    let mut lexer = Lexer::new(data);
    while let Ok(Some(_token)) = lexer.next_token() {
        // Continue consuming until stream ends or syntax error
    }
}

/// Fuzzing logic for the recursive-descent Parser (§1 L3 depth-bounded parser).
/// Repeatedly calls `parse_object()` until `None` or `Err`.
/// Invariant: Recursion depth limit (default 256) and collection bounds prevent stack overflows and panics.
pub fn fuzz_parser(data: &[u8]) {
    let mut parser = Parser::new(data);
    while let Ok(Some(_obj)) = parser.parse_object() {
        // Repeatedly parses next AST Object safely
    }
}

/// Fuzzing logic for Document loading and XRef resolution (§1 L4 xref table/stream and recovery pass).
/// Runs both strict loading and fault-tolerant linear reconstruction on arbitrary bytes.
/// Invariant: Recovery pass (`recover.rs`) must never panic on arbitrary, truncated, or corrupted inputs.
pub fn fuzz_xref(data: &[u8]) {
    // 1. Strict loading: validates strict xref parser against malformed bytes without recovery.
    let _ = Document::load_strict(data);

    // 2. Resilient loading: tests xref stream/table parsing and the linear recovery pass (recover.rs).
    if let Ok(doc) = Document::load(data) {
        let _ = doc.page_count();
        // Traverse bounded sample of objects to exercise indirect reference resolution & objstm
        for id in doc.xref.entries.keys().take(32) {
            let _ = doc.get_object(*id);
        }
    }
}

/// Fuzzing logic for stream decompression pipelines (§1 L5 stream view and decoders).
/// Exercises FlateDecode (zlib), Ascii85Decode, and AsciiHexDecode on adversarial inputs.
/// Invariant: Must respect 256MB decompression ceiling and never panic on invalid byte streams.
pub fn fuzz_filter(data: &[u8]) {
    // 1. FlateDecode (zlib decompressor)
    let flate_view = StreamView::new(data).with_filter(FilterKind::FlateDecode);
    let _ = flate_view.decode();

    // 2. Ascii85Decode (btoa decompressor)
    let a85_view = StreamView::new(data).with_filter(FilterKind::Ascii85Decode);
    let _ = a85_view.decode();

    // 3. AsciiHexDecode (hex decompressor)
    let ahex_view = StreamView::new(data).with_filter(FilterKind::AsciiHexDecode);
    let _ = ahex_view.decode();

    // 4. Chained pipeline (Ascii85 -> FlateDecode)
    let chained_view = StreamView::new(data)
        .with_filter(FilterKind::Ascii85Decode)
        .with_filter(FilterKind::FlateDecode);
    let _ = chained_view.decode();
}

/// Fuzzing logic for StandardSecurityHandler (§1.1 Standard Security Handler).
/// Fuzzes password verification, key derivation, and ciphertext decryption
/// on arbitrary, truncated, and corrupted encryption dictionaries and byte streams.
/// Invariant: Decryptor must never panic on corrupted ciphertexts, bad key lengths, or invalid padding.
pub fn fuzz_crypto(data: &[u8]) {
    // 1. Fuzz document loading with arbitrary encryption payload
    let _ = Document::load(data);
    let _ = Document::load_strict(data);

    // 2. Fuzz parser on encryption dictionary structure
    let mut parser = Parser::new(data);
    if let Ok(Some(Object::Dictionary(dict))) = parser.parse_object() {
        let _ = StandardSecurityHandler::from_encrypt_dict(&dict, None, b"");
        let _ = StandardSecurityHandler::from_encrypt_dict(&dict, Some(b"trailer_id_1234"), b"");
        let _ = StandardSecurityHandler::from_encrypt_dict(&dict, Some(b"trailer_id_1234"), b"owner");
    }

    // 3. Fuzz raw cipher implementations on corrupted ciphertexts
    if data.len() >= 16 {
        let key = &data[..16];
        let ciphertext = &data[16..];

        // RC4
        let rc4 = Rc4::new(key);
        let mut rc4_buf = ciphertext.to_vec();
        rc4.apply_keystream_in_place(&mut rc4_buf);

        // AES-128 CBC (handles random 16-byte IV prefix + PKCS#7)
        let _ = aes_128_cbc_decrypt(key, ciphertext);

        // AES-256 CBC (requires 32-byte key)
        if data.len() >= 32 {
            let key256 = &data[..32];
            let ciphertext256 = &data[32..];
            let _ = aes_256_cbc_decrypt(key256, ciphertext256);
        }
    }
}

/// Fuzzing logic for Content Stream Parser & Geometry State Machine (§1.4).
/// Exercises affine matrix operations, coordinate transformations, bounding boxes,
/// and q/Q graphics state stacking with arbitrary, NaN, Inf, and overflow floats.
/// Invariant: Matrix math and graphics state tracker must never panic and must respect stack depth cap.
pub fn fuzz_matrix(data: &[u8]) {
    // 1. Content stream parser & operator pipeline
    let mut parser = ContentParser::new(data);
    if let Ok(ops) = parser.parse() {
        let mut tracker = GraphicsStateTracker::new();
        let _ = tracker.process_operations(&ops);
    }

    // 2. Geometric affine matrix operations on floating point data
    if data.len() >= 24 {
        let read_f32 = |offset: usize| -> f32 {
            let bytes = [
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ];
            f32::from_ne_bytes(bytes)
        };

        let a = read_f32(0);
        let b = read_f32(4);
        let c = read_f32(8);
        let d = read_f32(12);
        let e = read_f32(16);
        let f = read_f32(20);

        let m1 = Matrix::new(a, b, c, d, e, f);
        let m2 = Matrix::identity();

        let _ = m1.multiply(&m2);
        let _ = m2.multiply(&m1);
        let _ = m1.inverse();
        let _ = m1.transform_point(10.0, 20.0);
        let _ = m1.transform_p(Point::new(a, b));

        let rect = Rect::new(c, d, e, f);
        let _ = rect.width();
        let _ = rect.height();
        let _ = m1.transform_rect(&rect);
    }

    // 3. Deep q/Q graphics state stack exhaustion guard
    let mut tracker = GraphicsStateTracker::new();
    let q_stream = vec![oxpdf::content::Operation::new(oxpdf::content::Operator::q, Vec::new()); data.len().min(1000)];
    let _ = tracker.process_operations(&q_stream);
    assert!(tracker.stack.len() <= GraphicsStateTracker::MAX_GRAPHICS_STATE_DEPTH);
}

/// Fuzzing logic for TrueType & CFF Font Table Parsers (§1.2).
/// Parses arbitrary, truncated, and corrupted sfnt / CFF binary streams.
/// Invariant: Must never panic on out-of-bounds table offsets, cyclic structures, or truncated data.
pub fn fuzz_font(data: &[u8]) {
    // 1. TrueType / OpenType font binary parser
    if let Ok(ttf) = TrueTypeFont::parse(data) {
        for gid in 0..64 {
            let _ = ttf.map_glyph_to_unicode(gid);
        }
        let _ = ttf.to_cmap();
    }

    // 2. Compact Font Format (CFF) binary parser
    if let Ok(cff) = CffFont::parse(data) {
        for gid in 0..64 {
            let _ = cff.map_glyph_to_unicode(gid);
        }
        let _ = cff.to_cmap();
    }

    // 3. EmbeddedFont unified wrappers
    let _ = EmbeddedFont::parse_truetype(data);
    let _ = EmbeddedFont::parse_cff(data);

    // 4. /ToUnicode CMap stream parser
    if let Ok(cmap) = CMap::parse(data) {
        let _ = cmap.lookup(0x41);
        let _ = cmap.lookup_char(0x41);
        let _ = cmap.decode_string(data);
    }
}

/// Fuzzing logic for CCITTFax and JBIG2 decompression engines (§1.3).
/// Fuzzes 1D/2D Modified Huffman and Group 4 2D run-length streams on arbitrary bytes.
/// Invariant: Must enforce 256MB decompression ceiling and never panic on invalid 2D codes.
pub fn fuzz_ccitt(data: &[u8]) {
    // 1. CCITT Group 4 2D (T.6) - K < 0
    let g4_params = CcittParams {
        k: -1,
        columns: 1728,
        rows: 0,
        ..Default::default()
    };
    let _ = decode_ccitt_fax(data, &g4_params);

    // 2. CCITT Group 3 1D (T.4) - K = 0
    let g3_1d_params = CcittParams {
        k: 0,
        columns: 1728,
        rows: 0,
        ..Default::default()
    };
    let _ = decode_ccitt_fax(data, &g3_1d_params);

    // 3. CCITT with EncodedByteAlign and BlackIs1
    let byte_aligned_params = CcittParams {
        k: -1,
        columns: 512,
        rows: 64,
        encoded_byte_align: true,
        black_is1: true,
        ..Default::default()
    };
    let _ = decode_ccitt_fax(data, &byte_aligned_params);

    // 4. StreamView integration for CCITTFaxDecode
    let stream = StreamView::new(data).with_filter(FilterKind::CCITTFaxDecode {
        params: Some(Box::new(g4_params)),
    });
    let _ = stream.decode();

    // 5. JBIG2 bi-level stream decoder skeleton
    let jbig2_params = Jbig2Params::default();
    let _ = decode_jbig2(data, &jbig2_params);
    let jbig2_stream = StreamView::new(data).with_filter(FilterKind::JBIG2Decode {
        params: Some(Box::new(jbig2_params)),
    });
    let _ = jbig2_stream.decode();
}
