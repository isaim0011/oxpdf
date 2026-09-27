# oxpdf — Full Engineering & Professionalization Roadmap

A ground-up plan: PDF binary fundamentals → working engine → benchmarked, tested, documented, published, maintained product. Ordered so each phase only depends on what came before. Honest status markers: `[ ]` planned / `[~]` in progress / `[x]` verified with tests.

---

## Phase 0 — Foundations

### 0.1 The PDF format itself, at the byte level
- [x] Header parsing (`%PDF-1.x`) and binary marker detection
- [x] Body syntax (indirect objects: `N G obj ... endobj`)
- [x] Cross-reference table (`xref`) & trailer resolution (`startxref` + `%%EOF`)
- [x] Incremental updates: `/Prev` chain walking with cycle guards
- [x] Object streams (`/Type /ObjStm`, PDF 1.5+) — compressed objects packed inside streams
- [~] Filter/encoding pipelines: FlateDecode (implemented), ASCII85/ASCIIHex/RunLength (in progress)
- [ ] Content-stream mini-language (`BT`, `ET`, `Tj`, `cm`, `Do`, etc.)

### 0.2 Binary/bit-level skills
- [x] Zero-copy byte-slice parsing (`&[u8]`, `SmallVec` inline allocations)
- [x] Delimiter and whitespace scanning conforming to ISO 32000-1 §7.2.2
- [x] Numeric literal parser with sign, decimal, and exponent edge cases
- [x] Deflate/zlib stream decompression via `flate2`
- [ ] Fixed-width binary reads for embedded fonts (TrueType/OpenType)
- [ ] UTF-16BE BOM text string decoding

---

## Phase 1 — Core Object Model & Parser

- [x] `Object<'a>` enum: `Null, Boolean, Integer, Real, Name, String, Array, Dictionary, Stream, Reference`
- [x] Depth-bounded recursive-descent parser (`Parser`) immune to recursion DoS (`lopdf#502`)
- [x] Classic `xref` table backward scanner + trailer parser
- [x] Fault-tolerant linear fallback scanner (Chromium PDFium style) for broken/shifted offsets
- [x] Lazy stream views (`StreamView`) with on-demand Flate decompression
- [x] Compressed Object Streams (`/ObjStm`) decompression and internal index resolution with caching
- [x] Document Catalog & Page Tree Walker (`/Root -> /Pages -> /Kids`) with visited-set loop guards
- [x] Monotonic zero-allocation serializer (`Serializer`) writing clean xref tables

---

## Phase 2 — The Five Engineering Pillars

### Pillar 1 — Zero-Copy Streaming Lexer
- [x] Borrowed token slices (`&'a [u8]`, `&'a str`) without intermediate heap allocations
- [x] `memchr`-accelerated delimiter and whitespace scanning
- [~] Corpus fuzz testing harness (`cargo fuzz`)

### Pillar 2 — Memory-Bounded Processing
- [x] Lazy object resolution on-demand (`Document::get_object(id)`)
- [~] `PdfSource` abstraction supporting both memory slices and memory-mapped files (`memmap2`)
- [ ] Peak RSS benchmarking on 500MB–10GB files verifying `< 32MB` resident memory

### Pillar 3 — Fault-Tolerant Reconstruction
- [x] Fallback linear reconstruction scanner scanning `N G obj` tokens
- [x] Resilient startxref backward search from file trailer
- [ ] Truncated file recovery pass

### Pillar 4 — Stack-Safe Recursion Immunity
- [x] Configurable recursion depth limit (`Parser::with_max_depth`)
- [ ] Visited object-id cycle detector for circular `/Kids` and dictionary references

### Pillar 5 — Cross-Platform & WASM32
- [x] Clean compilation on `wasm32-unknown-unknown` in CI
- [x] Native multi-platform matrix (Linux, macOS, Windows on stable & beta)

---

## Phase 3 — Testing & Quality Infrastructure

- [x] Unit tests per module (lexer, parser, xref, stream, writer, document)
- [x] Criterion benchmark harness (`benches/lexer_bench.rs`)
- [x] GitHub Actions automated format and clippy quality gates (`-D warnings`)
- [ ] Public test corpora regression harness (veraPDF / pdf.js suite)

---

## Phase 4 — Documentation & Release Discipline

- [x] SemVer release pipeline (`v0.1.0`, `v0.2.0` on crates.io)
- [x] `CHANGELOG.md` following Keep a Changelog
- [x] Dual-verification maintainer protocol (`oxpdf-maintainer` skill)
- [ ] Architecture design document (`DESIGN.md`)
- [ ] Real-world benchmark report (`BENCHMARKS.md`)
