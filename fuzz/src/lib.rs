use oxpdf::parser::Parser;
use oxpdf::{Document, FilterKind, Lexer, StreamView};

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
