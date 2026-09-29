# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
