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
- [x] OOM safety: hard caps on xref entry count, array/dict size (2M), decompressed size (256MB), page tree (10M nodes)
- [x] Filter/encoding pipelines: FlateDecode (implemented), ASCII85/ASCIIHex (implemented with 256MB cap)
- [x] XRef Streams (`/Type /XRef`, PDF 1.5+) — binary stream parsing with `/W`, `/Index`, types 0/1/2
- [x] Content-stream mini-language (`BT`, `ET`, `Tj`, `TJ`, `cm`, `Do`, etc.) in `src/content/`
- [x] UTF-16BE / UTF-16LE BOM text string decoding in `src/text.rs`
- [x] `PdfSource` abstraction supporting both memory slices and memory-mapped files (`memmap2`)
- [x] Peak RSS benchmarking on large files verifying bounded memory footprint (<32MB)
- [x] Truncated file recovery pass via `src/recover.rs`
- [x] Text extraction engine with Latin font encodings and kerning spacing in `src/text.rs`

### Pillar 4 — Stack-Safe Recursion Immunity
- [x] Configurable recursion depth limit (`Parser::with_max_depth`)
- [x] Visited object-id cycle detector for circular `/Kids` page trees
- [x] Worklist-based page tree traversal (no stack recursion)

### Pillar 5 — Cross-Platform & WASM32
- [x] Clean compilation on `wasm32-unknown-unknown` in CI
- [x] Native multi-platform matrix (Linux, macOS, Windows on stable & beta)

---

## Phase 3 — Testing & Quality Infrastructure

- [x] Unit tests per module (lexer, parser, xref, stream, writer, document) — 12 tests, 0 failures
- [x] Criterion benchmark harness (`benches/lexer_bench.rs`)
- [x] GitHub Actions automated format and clippy quality gates (`-D warnings`)
- [~] Public test corpora regression harness (veraPDF / pdf.js suite) — harness built, first run in progress
- [ ] `cargo fuzz` integration for adversarial input hardening

---

## Phase 4 — Documentation & Release Discipline

- [x] SemVer release pipeline (`v0.1.0`, `v0.2.0`, `v0.3.0` on crates.io)
- [x] `CHANGELOG.md` following Keep a Changelog
- [x] `README.md` with honest status table and known limitations section
- [x] Dual-verification maintainer protocol (`oxpdf-maintainer` skill)
- [ ] Architecture design document (`DESIGN.md`)
- [ ] Real-world benchmark report (`BENCHMARKS.md`) with measured corpus pass rate
