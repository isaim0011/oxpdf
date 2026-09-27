# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
