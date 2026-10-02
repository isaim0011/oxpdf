# Plan: Implement Missing Compression Filters (§1.3 Phase 1)

## Goal
Implement pure-Rust CCITTFaxDecode (Group 3 1D/2D and Group 4 2D ITU-T T.6) and JBIG2 stream decoders in `src/filter/`, integrate into `src/stream.rs`, export in `src/lib.rs`, and verify with thorough tests and clippy.

## Components
1. `src/filter/ccitt.rs`:
   - Pure-Rust ITU-T T.4 and T.6 decoder.
   - Support parameters:
     - `K`: `< 0` -> Group 4 2D (T.6), `0` -> Group 3 1D (T.4), `> 0` -> Group 3 2D (T.4)
     - `Columns`: default 1728
     - `Rows`: default 0 (if 0 or unspecified, decode until EOF or EOL/EOFB)
     - `BlackIs1`: bool, default false (0 = white, 1 = black; if false, white is 0, black is 1)
     - `EncodedByteAlign`: bool, default false
     - `EndOfBlock`: bool, default true (Group 4 EOFB detection)
     - `EndOfLine`: bool, default false
   - Huffman tables for Modified Huffman (T.4 1D terminating & make-up codes for white and black runs).
   - 2D modes: Pass mode (P), Vertical modes (V(0), VL(1..3), VR(1..3)), Horizontal mode (H + white run + black run), Extension mode.
   - Enforce `StreamView::MAX_DECOMPRESS_BYTES = 256 * 1024 * 1024` (256 MB) to prevent zip-bomb / memory exhaustion.
   - BitReader with MSB-first bitstream reading.
   - Output bi-level 1-bit per pixel packed bytes (row aligned to byte boundaries per PDF spec: `(columns + 7) / 8` bytes per row).

2. `src/filter/jbig2.rs`:
   - JBIG2 stream decoder skeleton & segment parsing:
     - Header parsing (optional 8-byte file header vs embedded stream).
     - Segment parsing (segment number, flags, referred segments, page association, data length).
     - Global stream / dictionary support (`JBIG2Globals` resolution).
     - Standard 1-bit bitmap raster output.
     - Strict size check against `MAX_DECOMPRESS_BYTES`.

3. `src/filter/mod.rs`:
   - Expose `decode_ccitt_fax`, `decode_jbig2`, parameter structs `CcittParams`, `Jbig2Params`.

4. `src/stream.rs`:
   - Update `FilterKind` enum:
     - `CCITTFaxDecode { params: Option<CcittParams> }` (or `Box<CcittParams>`)
     - `JBIG2Decode { globals: Option<Vec<u8>> }`
   - Update `StreamView::decode()`:
     - Match and decode using `src/filter/ccitt.rs` and `src/filter/jbig2.rs`.
   - Update `extract_stream_filters` and `parse_filter_name` if needed in `text.rs` or `stream.rs`.

5. `src/lib.rs`:
   - Export `pub mod filter;`.

6. Testing:
   - Group 4 test vectors (all white, all black, alternating vertical stripes, horizontal blocks, checkerboard).
   - Mode tests: Pass mode, Vertical modes (V(0), VL(1), VR(2), etc.), Horizontal mode.
   - Edge cases: EncodedByteAlign, BlackIs1 flag inversion, partial bytes at row ends.
   - Decompression bomb safety: test that exceeding `MAX_DECOMPRESS_BYTES` returns `Error::Unsupported`.
   - JBIG2 basic structure / error handling tests.
