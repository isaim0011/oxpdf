# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.2] - 2026-10-02

### Added
- **Standard Security Handler** (`src/crypto/`):
  - Complete implementation of ISO 32000-1 & ISO 32000-2 encryption handler supporting V1–V6.
  - Pure-Rust RC4 stream cipher (40-bit and 128-bit key scheduling and stream PRGA).
  - AES-128-CBC and AES-256-CBC cipher support with 16-byte random IV extraction and PKCS#7 unpadding.
  - Password authentication supporting default empty string (`""`) and custom passwords via `Document::load_with_password()`.
  - ISO 32000-2 (R6) key derivation with 100,000 SHA-2 iterations and validation hashes.
- **Embedded Font Introspection** (`src/font/`):
  - Mojibake elimination fallback when `/ToUnicode` CMaps are absent.
  - Zero-copy TrueType (`/FontFile2`) binary table parsing: sfnt headers, `cmap` (Format 4 BMP, Format 12 UCS-4), and `post` (Format 1.0/2.0 standard Mac and Pascal glyph names).
  - Compact Font Format (`/FontFile3` CFF) binary parser: Header, Name INDEX, Top DICT, String INDEX, and Charsets 0–2.
  - Full Adobe Glyph List (`AGL`) normalization resolving PostScript glyph names to Unicode code points.
- **Bi-Level Compression Filters** (`src/filter/`):
  - Pure-Rust ITU-T T.4/T.6 CCITTFaxDecode run-length decoder (`src/filter/ccitt.rs`) supporting Group 3 1D/2D and Group 4 2D modes.
  - JBIG2 bi-level stream decoder with `/JBIG2Globals` stream dictionary resolution (`src/filter/jbig2.rs`).
  - Strict bounded decompression ceilings enforced across all codecs.
- **Spatial Geometry & Matrix Transformation Engine** (`src/geom/`):
  - Affine 3x3 transformation matrix math (`Matrix`, `Point`, `Rect`) supporting identity, multiply, point/rect transforms, and matrix inversion.
  - Graphics state stack tracker (`GraphicsStateTracker`) handling `q`, `Q`, and `cm` concatenation.
  - Text state tracker maintaining Text Matrix ($T_m$), Text Line Matrix ($T_{lm}$), leading, scaling, and device coordinates ($T_{\text{device}} = T_m \times CTM$).
  - `TextSpan<'static>` emission with spatial bounding box coordinates, font sizes, font names, and style flags.
  - Public `Document::extract_spans(page_id)` and `Document::extract_spans_all()` methods.
- **Expanded Adversarial & Fuzzing Hardening**:
  - 4 new continuous fuzz targets in `fuzz/fuzz_targets/`: `crypto`, `font`, `ccitt`, and `matrix`.
  - Integration adversarial suite in `tests/adversarial_suite.rs` validating memory boundaries, float safety, and recursion limits.
- **Global Cross-Stack Benchmark Suite**:
  - Comprehensive empirical comparison in `BENCHMARKS.md` evaluating `oxpdf` against Rust (`lopdf`, `pdf`), C/C++ (`MuPDF`, `Poppler`, `PDFium`, `QPDF`), Java (`PDFBox`, `Tika`), Python (`pypdf`, `pdfplumber`), Node (`pdfjs-dist`), and Go (`pdfcpu`).
  - 1.024 GB multi-gigabyte streaming document stress test demonstrating sub-millisecond load time (201 µs) and strictly bounded physical RAM (<3.1 MB RSS).

### Fixed
- **XRef Stream `/DecodeParms` Predictor Unfiltering** (`src/xref.rs`):
  - Added pure-Rust implementation of PNG (Sub, Up, Average, Paeth) and TIFF Horizontal Differencing predictors for binary cross-reference streams.
  - Cleared all 4 remaining failing files in the 5,820-file reference corpus, achieving a **100.00% (5,820 / 5,820) flawless pass rate**.

### Changed
- Removed decryption from Scope Exclusions in `README.md` and promoted encrypted file handling to fully supported.
- Updated crate versions and satellite crates to `1.0.2`.

## [1.0.1] - 2026-10-01

### Added
- **/ToUnicode CMap Font Stream Parser** (`src/cmap.rs`):
  - Full implementation of Adobe PostScript & PDF CMap syntax per ISO 32000-1 §9.10.
  - Supports `begincodespacerange` (1–4 byte codespaces), `beginbfchar` (scalar and multi-codepoint ligatures e.g. `fi`, `fl`, emoji surrogate pairs), and `beginbfrange` (both contiguous code mapping and destination array bracket syntax).
  - Deeply integrated into `TextExtractor` and `extract_page_text()` via automatic `/Resources -> /Font -> /ToUnicode` stream resolution.
- **Stage B Portable SIMD Structural Scanning** (`src/simd.rs`):
  - 16-byte and 32-byte chunked structural classification using fast SWAR bitmask vectorization on stable Rust across all architectures (`x86_64`, `aarch64`, `wasm32`).
  - Accelerates `skip_whitespace`, `skip_whitespace_and_comments`, `read_name`, and delimiter demarcation in both `Lexer` and `ContentLexer`.
- **`oxpdf-cli` Command-Line Tool** (`oxpdf-cli/`):
  - Standalone high-performance CLI utility providing `inspect`, `extract-text`, `pack` (QPDF-style stream compaction), and `bench` subcommands.
- **`oxpdf-wasm` WebAssembly Bindings** (`oxpdf-wasm/`):
  - Zero-copy browser and edge WebAssembly bindings exposing `WasmDocument` with text extraction, page counting, and object stream compaction for `wasm32-unknown-unknown`.
- **Continuous Fuzzing Infrastructure** (`fuzz/`):
  - `cargo-fuzz` integration with 4 dedicated `libfuzzer-sys` fuzz targets (`lexer`, `parser`, `xref`, `filter`) and seeded adversarial corpora.
- **Typst Ecosystem Positioning & Comparison**:
  - Comprehensive architectural analysis in `BENCHMARKS.md` and `DESIGN.md` establishing the complementary roles of Typst (typesetting compiler: markup -> PDF) and oxpdf (streaming parser and reconstructor: PDF -> AST/text/packed PDF).

### Fixed
- **Fault-Tolerant Recovery Slice Boundary** (`src/recover.rs`):
  - Fixed an issue where arbitrary 64-byte lookback slices starting on lone delimiters aborted the tokenization loop, ensuring all indirect objects are discovered and indexed during linear recovery passes.
- **WASM Clean Compilation** (`src/source.rs`):
  - Gated host filesystem imports behind `#[cfg(not(target_arch = "wasm32"))]`, eliminating all warnings on wasm32 compilation targets.

## [1.0.0] - 2026-10-01

### Added
- **Content Stream Parsing Engine** (`src/content/`):
  - Dedicated `ContentLexer` (`src/content/lexer.rs`) tuned specifically for graphics, paths, matrices, and text operator syntax.
  - Zero-allocation parsing for coordinates, single/double character operators (`cm`, `re`, `m`, `l`, `BT`, `ET`, `Tj`, `TJ`, `q`, `Q`), and inline images (`BI`..`ID`..`EI`).
  - Comprehensive operator parser (`ContentParser`) yielding structured `Operation<'a>` token streams.
- **Unicode Plaintext Text Extraction** (`src/text.rs`):
  - Decodes Latin-script encodings: `WinAnsiEncoding`, `StandardEncoding`, `MacRomanEncoding`, and `PdfDocEncoding`.
  - Full Unicode support for UTF-16BE and UTF-16LE strings with byte-order marks (`\xFE\xFF` and `\xFF\xFE`).
  - Font kerning displacement extraction: converts negative coordinate shifts in `TJ` arrays ($\le -100$) into proper natural word spacing.
  - Public `Document::extract_text(page_id)` and `Document::extract_text_all()` API methods.
- **Source Abstraction Layer** (`src/source.rs`):
  - `PdfSource` trait decoupling engine operations from physical storage.
  - `MmapSource` offloading virtual memory to the OS kernel page cache, enabling processing of multi-gigabyte PDFs with bounded physical RAM (<32 MB).
  - `BufferSource` zero-copy memory slice wrapper for in-memory and WASM environments.
- **Fault-Tolerant Reconstruction** (`src/recover.rs`):
  - PDFium/Chromium-inspired forward linear scanning engine for documents with truncated, shifted, or severed cross-reference tables.
  - Auto-synthesizes minimal trailer and `/Type /Catalog` pointers when structural trailers are destroyed.
  - `Document::load()` automatically attempts repair on structural failure, while `Document::load_strict()` enforces strict ISO 32000-1 compliance.
- **Empirical Benchmarks & System Design Specs**:
  - `BENCHMARKS.md`: Head-to-head empirical metrics across 5,820 corpus files; Criterion micro-benchmarks; RSS memory profiling demonstrating 99.04x speedup over `lopdf`.
  - `DESIGN.md`: Formal L0–L6 pipeline specifications, memory streaming models, and verbatim §9 permanent scope exclusion guarantees.
- **Adversarial Security Test Suite** (`tests/security_audit.rs`):
  - 15 hostile security tests verifying immunity to circular page trees, decompression bombs, integer overflows, cyclic object streams, and adversarial `/Length` bounds.

### Changed
- Promoted `oxpdf` to official **v1.0.0** stable release with full semantic stability guarantees.
- Replaced all unchecked arithmetic operations in `document.rs`, `writer.rs`, `recover.rs`, and `xref.rs` with `checked_add` and strict boundary guards.
- Expanded octal escape parsing accumulator to `u16` with modulo-256 masking to eliminate debug-mode overflow panics per ISO 32000-1 §7.3.4.2.
- Hardened ASCII-Hex and ASCII-85 stream decompressors to enforce the 256 MB maximum decompression limit.

### Performance
- **Throughput**: 2,348.62 MB/s (42,748 files/sec) across 5,820 corpus documents (**99.04x faster than lopdf**).
- **Latency**: $p_{50}$ = 0.007 ms, $p_{90}$ = 0.030 ms, $p_{99}$ = 0.159 ms.
- **Memory**: 108 KB net parser heap delta on a 10.87 MB document (**100.5x less memory than lopdf**).
- **Corpus Verification**: 0 panics across 5,820 files.

## [0.4.0] - 2026-09-29

### Fixed
- **Critical infinite loop / OOM** (`lexer.rs`): `read_regular_token()` returned a zero-length `Token::Keyword("")` when encountering a lone `>` (not part of `>>`) without advancing the lexer cursor. Any caller that looped on `Ok(Some(...))` — including `reconstruct_linear_scan` — would spin forever, growing a `Vec<Token>` until OOM. Fix: detect zero-advance (`start == self.pos`), advance by 1, and return `Error::SyntaxError`. Defense-in-depth: `reconstruct_linear_scan` now also caps per-match token vectors at 128 entries.
- **Critical OOM** (`xref.rs`): xref subsection `num_entries` was cast to u32 without bounds check — a malformed PDF claiming 3,221,225,472 entries caused a 12 GB allocation. Now capped at `file_len / 20` (physical maximum for traditional xref entries); returns `Error::SyntaxError` for oversized counts.
- **Critical OOM** (`parser.rs`): array and dict parsers had no element count limit — a `/Kids` array claiming billions of references OOM-crashed the process. Both now capped at 2,000,000 elements, returning `Error::Unsupported`.
- **Critical OOM** (`stream.rs`): `flate2::read_to_end()` had no output size limit — a zlib bomb expanded to 12 GB. Now capped at 256 MB via `.take()`, returning `Error::Unsupported` if exceeded.
- **Crash** (`xref.rs`): `startxref` byte offset was not validated against file length — a bogus offset caused a panic on `&data[offset..]`. Now returns `Error::SyntaxError` for out-of-bounds offsets.
- **Crash** (`document.rs`): page tree `worklist` had no size cap — a malformed page tree could grow unboundedly. Now returns `Error::Unsupported` if total nodes exceed 10,000,000.
- **Invalid output** (`writer.rs`): `Object::Stream` serialization wrote the stale `/Length` from the original dict instead of the actual data length, producing invalid PDFs. Fixed to always write `actual_data.len()`.

### Added
- **XRef Stream parsing** (`xref.rs`): full PDF 1.5+ binary XRef stream support. Parses `/W` field widths, `/Index` ranges, type-0 (free), type-1 (in-use), and type-2 (compressed) entries from FlateDecode or raw streams. Follows `/Prev` chain for incremental updates. Extracts `/Root` and `/Size` from the embedded trailer dict.
- **Encryption detection** (`document.rs`): fast `memchr` pre-scan of the trailer region plus parsed-trailer-dict check for `/Encrypt`. Returns `Error::Unsupported("encrypted: …")` immediately — no hanging, no garbled output.
- **QPDF-style object packing** (`document.rs`): `Document::write_packed()` groups non-stream objects into a single `FlateDecode /ObjStm`, writes a PDF 1.5+ binary XRef stream, and outputs a self-contained file that is 30–70% smaller than `write_to()` output.
- `Error::Unsupported(&'static str)` variant — all adversarial/unsupported cases return a named, inspectable error.

### Changed
- `Cargo.toml` description: removed false "SIMD-accelerated" claim → "memchr-accelerated scanning".
- `Cargo.toml` keywords: removed misleading `simd` keyword; added `fault-tolerant`.
- `README.md`: promoted XRef streams to ✅ Shipped; published real corpus numbers.
- `ROADMAP.md`: all statuses corrected to match actual implementation.

### Corpus (verified, v0.4.0)
- **2906 / 2906** veraPDF + Isartor PDFs — **100.00%** pass rate.
- Load time p50 = **0.05 ms**, p99 = **1.99 ms** (Windows x86-64 release build).
- Zero OOM crashes, zero panics.

## [0.3.0] - 2026-09-27

### Added
- PDF 1.5+ Compressed Object Streams (`/Type /ObjStm`) decompression and internal object index resolution with `Document` caching.
- `Object::Stream` AST variant with zero-copy stream dictionary and lazy payload slice.
- `Object::into_owned()` and helper methods (`as_name`, `as_dict`, `name`) for seamless static lifecycle handling.
- Linear stream payload lexer using `memchr::memmem` to find matching `endstream` delimiters without trailing byte corruption.

## [0.2.0] - 2026-09-27

### Added
- High-level, bounded-memory `Document` API (`oxpdf::Document`) enabling single-object random lookups without loading entire files into RAM.
- Linearizer and clean document re-serializer (`Document::write_to`) rebuilding unified XRef tables and trailers.
- Lazy document object resolution directly from memory-mapped slices.

## [0.1.0] - 2026-09-27

### Added
- Zero-copy streaming Lexer (`oxpdf::lexer::Lexer`) borrowing from byte slices with zero heap allocations for numeric tokens, keywords, and identifiers.
- Delimiter and whitespace classifier conforming strictly to ISO 32000-1 §7.2.2.
- Stack-safe Object Parser (`oxpdf::parser::Parser`) with configurable depth bounding to prevent stack-overflow DoS vulnerabilities.
- SmallVec inline memory optimization for literal strings (`( ... )`) and hexadecimal byte sequences (`< ... >`).
- Hybrid Cross-Reference table (`oxpdf::xref::XRefTable`) with backwards traversal and PDFium-style resilient forward linear reconstruction scanner for corrupted documents.
- Lazy Stream view (`oxpdf::stream::StreamView`) with on-demand Flate/Zlib decompression for multi-gigabyte files under constant bounded memory.
- Monotonic zero-allocation Serializer (`oxpdf::writer::Serializer`) inspired by Typst's `pdf-writer`.
- Cross-platform CI/CD testing workflow covering Linux, macOS, Windows, and WebAssembly (`wasm32-unknown-unknown`).
- Professional `oxpdf-maintainer` ecosystem skill for automated quality gating, benchmarking, and releases.
