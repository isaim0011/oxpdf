use oxpdf::content::{ContentParser, Operation, Operator};
use oxpdf::crypto::{aes_128_cbc_decrypt, aes_256_cbc_decrypt, Rc4, StandardSecurityHandler};
use oxpdf::error::Error;
use oxpdf::filter::{decode_ccitt_fax, decode_jbig2, CcittParams, Jbig2Params};
use oxpdf::font::{CffFont, TrueTypeFont};
use oxpdf::geom::{GraphicsStateTracker, Matrix, Point, Rect};
use oxpdf::types::Object;
use oxpdf::Document;
use std::borrow::Cow;
use std::collections::BTreeMap;

// =============================================================================
// 1. CCITT / JBIG2 Decompression Bomb Defense
// =============================================================================

#[test]
fn test_ccitt_decompression_bomb_cap() {
    // CCITT parameters specifying 2,000,000 rows with 1728 columns
    // Output size: 2_000_000 * ceil(1728 / 8) = 2_000_000 * 216 = 432,000,000 bytes (~412 MB)
    // Exceeds StreamView::MAX_DECOMPRESS_BYTES (256 MB) -> must return Error::Unsupported
    let params = CcittParams {
        columns: 1728,
        rows: 2_000_000,
        ..Default::default()
    };

    let fake_stream = [0u8; 32];
    let res = decode_ccitt_fax(&fake_stream, &params);
    assert!(
        matches!(res, Err(Error::Unsupported(msg)) if msg.contains("exceeds 256 MB safety limit")),
        "Expected unsupported error due to >256MB decompression bomb, got {:?}",
        res
    );
}

#[test]
fn test_jbig2_decompression_bomb_cap() {
    // Construct a JBIG2 stream segment that claims huge width and height (e.g. 100,000 x 50,000)
    // Segment header (ISO/IEC 14492 §7.2):
    // Segment number: 1 (4 bytes: 00 00 00 01)
    // Segment header flags: type 48 (Page Information), retain flag 0 (1 byte: 48)
    // Page association: 1 (1 byte, short format: 01)
    // Data length: 19 bytes (4 bytes: 00 00 00 13)
    // Data:
    //   Width: 100,000 (0x000186A0)
    //   Height: 50,000 (0x0000C350)
    //   Resolution X: 0x0000012C
    //   Resolution Y: 0x0000012C
    //   Flags: 0x00
    //   Combination op: 0x00, 0x00
    let mut jbig2_data = Vec::new();
    // Segment 1: Header
    jbig2_data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // Seg num 1
    jbig2_data.push(48); // Type 48: Page Information
    jbig2_data.push(0x01); // Page assoc 1
    jbig2_data.extend_from_slice(&(19u32.to_be_bytes())); // Length = 19
    // Segment 1: Data (Page info)
    jbig2_data.extend_from_slice(&(100_000u32.to_be_bytes())); // Width
    jbig2_data.extend_from_slice(&(50_000u32.to_be_bytes())); // Height
    jbig2_data.extend_from_slice(&[0x00, 0x00, 0x01, 0x2C]); // Res X
    jbig2_data.extend_from_slice(&[0x00, 0x00, 0x01, 0x2C]); // Res Y
    jbig2_data.extend_from_slice(&[0x00, 0x00, 0x00]); // flags & striping

    let params = Jbig2Params::default();
    let res = decode_jbig2(&jbig2_data, &params);
    assert!(
        matches!(res, Err(Error::Unsupported(msg)) if msg.contains("exceeds 256 MB safety limit")),
        "Expected unsupported error due to >256MB JBIG2 decompression bomb, got {:?}",
        res
    );
}

// =============================================================================
// 2. Matrix NaN / Inf Floating-Point Overflow Immunity
// =============================================================================

#[test]
fn test_matrix_nan_inf_immunity() {
    let nan_matrices = [
        Matrix::new(f32::NAN, 0.0, 0.0, 1.0, 0.0, 0.0),
        Matrix::new(1.0, f32::INFINITY, 0.0, 1.0, 0.0, 0.0),
        Matrix::new(1.0, 0.0, f32::NEG_INFINITY, 1.0, 0.0, 0.0),
        Matrix::new(1.0, 0.0, 0.0, f32::NAN, f32::MAX, f32::MIN),
        Matrix::new(f32::NAN, f32::NAN, f32::NAN, f32::NAN, f32::NAN, f32::NAN),
    ];

    let rect = Rect::new(-10.0, -20.0, 100.0, 200.0);
    let point = Point::new(42.0, -17.5);

    for m in &nan_matrices {
        // Multiplications must never panic
        let _ = m.multiply(&Matrix::identity());
        let _ = Matrix::identity().multiply(m);
        let _ = m.multiply(m);

        // Point transforms must never panic
        let (tx, ty) = m.transform_point(12.34, 56.78);
        let p_res = m.transform_p(point);
        let _ = format!("{tx}, {ty}, {p_res}");

        // Rect transforms must never panic
        let transformed_rect = m.transform_rect(&rect);
        let _ = transformed_rect.width();
        let _ = transformed_rect.height();

        // Inverting singular or NaN matrices must return None safely without panic
        let inv = m.inverse();
        let _ = inv;
    }

    // Feeding NaN/Inf operators into ContentParser and GraphicsStateTracker
    let content = b"q NaN 0 0 1 0 0 cm 1 0 0 1 100 200 Tm (Test) Tj Q";
    let mut parser = ContentParser::new(content);
    if let Ok(ops) = parser.parse() {
        let mut tracker = GraphicsStateTracker::new();
        let spans = tracker.process_operations(&ops);
        // Must complete safely without panic
        let _ = spans;
    }
}

// =============================================================================
// 3. Encryption Dictionary Cyclic References and Invalid Key Lengths
// =============================================================================

#[test]
fn test_encryption_dict_cyclic_and_invalid_keys() {
    // 3.1 Invalid /R revision (< 2 or missing)
    let mut dict_invalid_r = BTreeMap::new();
    dict_invalid_r.insert(Cow::Borrowed("Filter"), Object::Name(Cow::Borrowed("Standard")));
    dict_invalid_r.insert(Cow::Borrowed("V"), Object::Integer(1));
    dict_invalid_r.insert(Cow::Borrowed("R"), Object::Integer(1)); // Invalid: R must be >= 2
    let res_r = StandardSecurityHandler::from_encrypt_dict(&dict_invalid_r, None, b"");
    assert!(
        matches!(res_r, Err(Error::Decryption(_))),
        "Expected decryption error for R < 2, got {:?}",
        res_r
    );

    // 3.2 Invalid /Length (> 256 or non-multiple of 8)
    let mut dict_invalid_len = BTreeMap::new();
    dict_invalid_len.insert(Cow::Borrowed("Filter"), Object::Name(Cow::Borrowed("Standard")));
    dict_invalid_len.insert(Cow::Borrowed("V"), Object::Integer(2));
    dict_invalid_len.insert(Cow::Borrowed("R"), Object::Integer(3));
    dict_invalid_len.insert(Cow::Borrowed("Length"), Object::Integer(500)); // 500 bits > 256
    dict_invalid_len.insert(Cow::Borrowed("O"), Object::String(vec![0u8; 32].into()));
    dict_invalid_len.insert(Cow::Borrowed("U"), Object::String(vec![0u8; 32].into()));
    dict_invalid_len.insert(Cow::Borrowed("P"), Object::Integer(-4));
    let res_len = StandardSecurityHandler::from_encrypt_dict(&dict_invalid_len, None, b"");
    assert!(
        matches!(res_len, Err(Error::Decryption(_))),
        "Expected decryption error for invalid Length, got {:?}",
        res_len
    );

    // 3.3 Cyclic reference in /Encrypt dictionary during Document loading
    // Object 1 is Catalog, Object 2 is Encrypt pointing to 2 0 R
    let cyclic_pdf = b"%PDF-1.4\n\
1 0 obj\n<< /Type /Catalog >>\nendobj\n\
2 0 obj\n<< /Filter /Standard /V 1 /R 2 /O (12345678901234567890123456789012) /U (12345678901234567890123456789012) /P -4 /Encrypt 2 0 R >>\nendobj\n\
xref\n\
0 3\n\
0000000000 65535 f \r\n\
0000000009 00000 n \r\n\
0000000048 00000 n \r\n\
trailer\n\
<< /Size 3 /Root 1 0 R /Encrypt 2 0 R >>\n\
startxref\n\
175\n\
%%EOF";

    // Document load must handle cyclic reference gracefully without hanging
    let doc = Document::load(cyclic_pdf);
    // May succeed loading structure or error gracefully
    if let Ok(d) = doc {
        let _ = d.get_object(2);
    }

    // 3.4 Raw cipher decryption with invalid or corrupted keys / padding
    let key = [0x5au8; 16];
    let malformed_ct = [0x42u8; 7]; // Not a multiple of 16 for AES CBC
    let aes_res = aes_128_cbc_decrypt(&key, &malformed_ct);
    assert!(
        aes_res.is_err(),
        "Expected error for non-multiple-of-16 ciphertext, got {:?}",
        aes_res
    );

    let key32 = [0x33u8; 32];
    let aes256_res = aes_256_cbc_decrypt(&key32, &malformed_ct);
    assert!(
        aes256_res.is_err(),
        "Expected error for non-multiple-of-16 ciphertext, got {:?}",
        aes256_res
    );

    // RC4 stream decryption with empty buffer
    let rc4 = Rc4::new(&key);
    let mut empty_buf = Vec::new();
    rc4.apply_keystream_in_place(&mut empty_buf);
    assert!(empty_buf.is_empty());
}

// =============================================================================
// 4. Malformed TrueType / CFF Headers with Out-of-Bounds Offsets
// =============================================================================

#[test]
fn test_malformed_font_headers_oob() {
    // 4.1 TrueType font: Truncated headers (< 12 bytes)
    assert!(matches!(
        TrueTypeFont::parse(&[0, 1, 0, 0]),
        Err(Error::UnexpectedEof(_))
    ));

    // 4.2 TrueType font: Table record pointing past end of file buffer
    // 1 table (num_tables = 1), header length = 12 + 16 = 28 bytes
    let mut tt_data = vec![0u8; 28];
    tt_data[4] = 0;
    tt_data[5] = 1; // 1 table
    tt_data[12..16].copy_from_slice(b"cmap"); // tag
    // offset = 1000, length = 500 (beyond 28 bytes)
    tt_data[20..24].copy_from_slice(&1000u32.to_be_bytes());
    tt_data[24..28].copy_from_slice(&500u32.to_be_bytes());

    let tt_res = TrueTypeFont::parse(&tt_data);
    assert!(
        matches!(tt_res, Err(Error::SyntaxError { .. })),
        "Expected SyntaxError for table offset exceeding data bounds, got {:?}",
        tt_res
    );

    // 4.3 TrueType font: offset + length integer overflow
    let mut tt_overflow = vec![0u8; 28];
    tt_overflow[4] = 0;
    tt_overflow[5] = 1;
    tt_overflow[12..16].copy_from_slice(b"cmap");
    tt_overflow[20..24].copy_from_slice(&u32::MAX.to_be_bytes());
    tt_overflow[24..28].copy_from_slice(&100u32.to_be_bytes());

    let tt_overflow_res = TrueTypeFont::parse(&tt_overflow);
    assert!(
        matches!(tt_overflow_res, Err(Error::SyntaxError { .. })),
        "Expected SyntaxError for integer overflow offset, got {:?}",
        tt_overflow_res
    );

    // 4.4 CFF font: Truncated header (< 4 bytes)
    assert!(matches!(
        CffFont::parse(&[1, 0]),
        Err(Error::UnexpectedEof(_))
    ));

    // 4.5 CFF font: Header size claiming offset beyond buffer
    let cff_oob = [1, 0, 100, 4]; // hdr_size = 100 > buffer len 4
    let cff_res = CffFont::parse(&cff_oob);
    assert!(
        matches!(cff_res, Err(Error::SyntaxError { .. })),
        "Expected SyntaxError for CFF header size exceeding data length, got {:?}",
        cff_res
    );

    // 4.6 CFF font: Header size < 4 (invalid CFF spec)
    let cff_too_small = [1, 0, 2, 4];
    let cff_too_small_res = CffFont::parse(&cff_too_small);
    assert!(
        matches!(cff_too_small_res, Err(Error::SyntaxError { .. })),
        "Expected SyntaxError for CFF header size < 4, got {:?}",
        cff_too_small_res
    );
}

// =============================================================================
// 5. q/Q Graphics State Stack Depth Caps
// =============================================================================

#[test]
fn test_graphics_state_stack_depth_cap() {
    let mut tracker = GraphicsStateTracker::new();
    assert_eq!(tracker.stack.len(), 0);

    // Feed 500 consecutive `q` operators
    let q_op = Operation::new(Operator::q, vec![]);
    for _ in 0..500 {
        let mut spans = Vec::new();
        tracker.process_operation(&q_op, &mut spans);
    }

    // Must be capped strictly at MAX_GRAPHICS_STATE_DEPTH (256)
    assert_eq!(
        tracker.stack.len(),
        GraphicsStateTracker::MAX_GRAPHICS_STATE_DEPTH,
        "Graphics state stack depth must be capped at MAX_GRAPHICS_STATE_DEPTH (256)"
    );

    // Additional `q` operators must not increase the stack size
    for _ in 0..50 {
        let mut spans = Vec::new();
        tracker.process_operation(&q_op, &mut spans);
    }
    assert_eq!(
        tracker.stack.len(),
        GraphicsStateTracker::MAX_GRAPHICS_STATE_DEPTH
    );

    // Feed `Q` operators to pop the stack
    let pop_op = Operation::new(Operator::Q, vec![]);
    for _ in 0..100 {
        let mut spans = Vec::new();
        tracker.process_operation(&pop_op, &mut spans);
    }
    assert_eq!(tracker.stack.len(), 156);

    // Popping more times than pushed must not panic (graceful underflow handling)
    for _ in 0..200 {
        let mut spans = Vec::new();
        tracker.process_operation(&pop_op, &mut spans);
    }
    assert_eq!(tracker.stack.len(), 0);
}
