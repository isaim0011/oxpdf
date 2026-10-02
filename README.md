# oxpdf

[![crates.io](https://img.shields.io/crates/v/oxpdf.svg)](https://crates.io/crates/oxpdf)
[![docs.rs](https://docs.rs/oxpdf/badge.svg)](https://docs.rs/oxpdf)
[![CI](https://github.com/isaim0011/oxpdf/actions/workflows/ci.yml/badge.svg)](https://github.com/isaim0011/oxpdf/actions)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

`oxpdf` is a pure-Rust streaming PDF parser, indexer, and reconstructor. It is designed for high-throughput ingestion, strictly bounded memory usage, fault-tolerant reconstruction of corrupted files, and Unicode text extraction.

- **Streaming architecture**: Direct indexing of cross-reference tables without loading the entire document into an in-memory document tree.
- **Bounded memory**: Operates within a bounded physical memory envelope (<32 MB RAM) via memory-mapped I/O (`memmap2`) even when processing multi-gigabyte files.
- **Fault-tolerant recovery**: Linear forward reconstruction pass (modeled after PDFium) recovers objects and synthesized trailers when cross-reference tables are damaged or severed.
- **Standard Security Handler**: Decrypts password-protected documents (V1–V6, RC4 40/128-bit, AES-128, AES-256 ISO 32000-1 Extension 3 & ISO 32000-2 R6 100k SHA-2 rounds).
- **Embedded Font Introspection**: Zero-copy TrueType (`/FontFile2`) and CFF (`/FontFile3`) binary table parsing (`cmap`, `post`, AGL) as automatic fallback when `/ToUnicode` CMaps are missing.
- **Bi-Level Compression Filters**: Pure-Rust ITU-T T.4/T.6 CCITTFaxDecode (Group 3/4) and JBIG2 stream decoders under strict 256 MB decompression ceiling.
- **Spatial Geometry & Layout Engine**: Full graphics state stack (`q`/`Q`), affine coordinate transformations ($T_{\text{device}} = T_m \times CTM$), and bounding box calculation (`TextSpan`).
- **Content stream parsing**: Dedicated operator tokenizer and parser for page appearance streams, graphics states, transformation matrices, and text showing operators.
- **Unicode text extraction**: Decodes standard font encodings (WinAnsi, Standard, MacRoman, PdfDoc), UTF-16 strings with byte-order marks, and `/ToUnicode` CMap streams with ligature support.
- **QPDF-style packing**: Re-serializes documents with compressed object streams (`/ObjStm`) and binary cross-reference streams, reducing file size by 30–70%.

For architectural specifications, see [DESIGN.md](DESIGN.md). For detailed empirical benchmarks and reproduction steps, see [BENCHMARKS.md](BENCHMARKS.md).

---

## Architecture and Feature Support

| Feature | Status | Specification & Implementation |
|---|:---:|---|
| **Zero-copy lexer** | Supported | Borrows tokens directly from input buffers; uses `SmallVec<[u8; 32]>` for short tokens; `memchr`-accelerated delimiter scanning. |
| **Stage B SIMD lexer** | Supported | 16-byte and 32-byte chunked structural classification using SWAR bitmask vectorization on stable Rust. |
| **Standard Security Handler** | Supported | ISO 32000-1 & ISO 32000-2 standard encryption: V1–V6, RC4 40/128-bit, AES-128-CBC, AES-256-CBC, and R6 100,000 SHA-2 iterations. |
| **Embedded font introspection** | Supported | Zero-copy TrueType (`cmap` format 4/12, `post`) and CFF charset parsing with Adobe Glyph List (AGL) normalization. |
| **Bi-level compression filters** | Supported | Pure-Rust ITU-T T.6 CCITTFaxDecode (Group 3 1D/2D, Group 4 2D) and JBIG2 stream decoding. |
| **Spatial geometry & `TextSpan`** | Supported | 3x3 affine transformation matrix engine, graphics state stack (`q`/`Q`), and coordinate bounding box calculation. |
| **/ToUnicode CMap parsing** | Supported | Implements ISO 32000-1 §9.10; supports `begincodespacerange`, `beginbfchar` (scalar, ligature, surrogate pair), and `beginbfrange`. |
| **On-demand object resolution** | Supported | `Document::load` builds an xref index; individual indirect objects are parsed on demand via `Document::get_object(id)`. |
| **Linear xref reconstruction** | Supported | Forward scanning pass detects `<id> <gen> obj` tokens across severed or truncated files to restore broken documents. |
| **Object streams (`/ObjStm`)** | Supported | PDF 1.5+ compressed indirect objects with decompressed stream caching. |
| **Binary cross-reference streams** | Supported | Full support for type 0 (free), type 1 (uncompressed), and type 2 (compressed) cross-reference stream entries. |
| **Content stream operators** | Supported | Dedicated `content::lexer` and `content::ops` modules supporting all standard ISO 32000-1 graphics and text operators. |
| **Unicode text extraction** | Supported | Font encoding mapping, CMap stream decoding, and kerning displacement analysis for word spacing. |
| **Source abstraction** | Supported | `PdfSource` trait with `MmapSource` (native file mapping) and `BufferSource` (in-memory slices and WebAssembly). |
| **Object compaction (`write_packed`)** | Supported | Re-packs non-stream objects into `/ObjStm` containers with a binary xref stream. |
| **Adversarial input defenses** | Supported | Recursion limits, cyclic reference detection, checked integer arithmetic, and a 256 MB decompression limit. |

---

## Empirical Benchmarks

### Global Cross-Stack Performance Matrix

Comprehensive empirical comparison across primary PDF engines and language runtimes (measured on Intel Core i5-6200U, release profile):

| Engine | Ecosystem / Runtime | Throughput | Median Latency ($p_{50}$) | Peak RSS (10MB+ File) | Memory Model | Memory Safety |
|---|---|:---:|:---:|:---:|---|:---:|
| **`oxpdf`** (v1.0.2) | **Pure Rust** | **1,532 – 2,348 MB/s** | **0.007 – 0.014 ms** | **<32 MB bounded** (108 KB net heap) | **Streaming Zero-Copy Index** | **Memory Safe (0 panics)** |
| **`lopdf`** (v0.36.0) | Pure Rust | 22.9 – 23.7 MB/s | 0.200 – 0.227 ms | ~24.9 MB (+10.6 MB heap) | In-Memory DOM Tree | Memory Safe |
| **`pdf` crate** (v0.9.0) | Pure Rust | ~45 – 65 MB/s | 0.140 – 0.180 ms | ~35.0 MB | Typed Struct Mapping | Memory Safe |
| **`MuPDF`** (v1.24) | C / Native | ~280 – 420 MB/s | 0.045 – 0.090 ms | ~45 – 80 MB | C Heap Allocator | Unsafe (C pointer ops) |
| **`Poppler`** (v24.08) | C++ / Native | ~150 – 230 MB/s | 0.080 – 0.150 ms | ~60 – 120 MB | C++ Object Graph | Unsafe (Historic CVEs) |
| **`PDFium`** (Chromium) | C++ / Native | ~210 – 340 MB/s | 0.060 – 0.110 ms | ~50 – 95 MB | C++ Streaming Reader | Unsafe (Sandboxed) |
| **`Apache PDFBox`** (v3.0) | Java / JVM | ~25 – 48 MB/s | 0.850 – 2.100 ms | ~180 – 350 MB (JVM Heap) | DOM Object Model | Managed (GC pauses) |
| **`pypdf`** (v4.3) | Python (CPython) | ~8 – 18 MB/s | 2.500 – 8.000 ms | ~85 – 160 MB | Python Object Graph | Managed (GIL bound) |

### Head-to-Head Reference Corpus Benchmark (5,820 Files)

The following measurements reflect the complete 5,820-file reference corpus (319.75 MB total data) evaluated against `lopdf v0.36.0`:

| Metric | `oxpdf v1.0.2` | `lopdf v0.36.0` | Relative Difference |
|---|---|---|---|
| **Throughput** | **1,532.85 – 2,348.62 MB/s** | 22.93 – 23.71 MB/s | **66.8x – 99.04x faster** |
| **File processing rate** | **27,900 – 42,748 files/s** | 417 – 431 files/s | **66.8x – 99.04x faster** |
| **Median latency ($p_{50}$)** | **0.007 – 0.014 ms** (7–14 µs) | 0.200 – 0.227 ms (200–227 µs) | **14.4x – 32.91x lower latency** |
| **90th percentile latency ($p_{90}$)** | **0.030 – 0.042 ms** (30–42 µs) | 0.841 – 0.923 ms (841–923 µs) | **20.0x – 30.76x lower latency** |
| **99th percentile latency ($p_{99}$)** | **0.159 – 0.202 ms** (159–202 µs) | 5.807 – 6.968 ms (5,807–6,968 µs) | **28.8x – 43.96x lower latency** |
| **Net heap allocation (10.87 MB document)** | **108 – 156 KB** | 10,856 KB | **69.7x – 100.5x lower memory footprint** |
| **Corpus pass rate** | **100.00%** (5,820 / 5,820) | 99.55% (5,794 / 5,820) | **+26 files successfully parsed** |
| **Panics / unexpected aborts** | **0** | 0 | Both crates panic-free on corpus |

*Full benchmark methodology, 1.024 GB streaming stress tests, and reproduction datasets are documented in [BENCHMARKS.md](BENCHMARKS.md).*

---

## Installation

Add `oxpdf` to your `Cargo.toml`:

```toml
[dependencies]
oxpdf = "1.0.2"
```

To use `oxpdf` in `no_std` environments (without memory-mapped file support):

```toml
[dependencies]
oxpdf = { version = "1.0.2", default-features = false }
```

---

## Usage Examples

### Extracting Text from All Pages

```rust
use oxpdf::Document;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read("input.pdf")?;
    let doc = Document::load(&data)?;

    let pages_text = doc.extract_text_all()?;
    for (page_number, text) in pages_text.iter().enumerate() {
        println!("--- Page {} ---\n{}", page_number + 1, text);
    }

    Ok(())
}
```

### Inspecting Document Structure

```rust
use oxpdf::Document;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read("input.pdf")?;
    let doc = Document::load(&data)?;

    println!("Total objects indexed: {}", doc.object_count());
    println!("Total pages: {}", doc.page_count()?);

    for (index, &page_id) in doc.get_page_ids()?.iter().enumerate() {
        println!("Page {}: Object ID {}", index + 1, page_id);
    }

    if let Some(catalog) = doc.catalog()? {
        println!("Document catalog dictionary: {:?}", catalog);
    }

    Ok(())
}
```

### Parsing Content Stream Operators

```rust
use oxpdf::content::{ContentParser, Operator};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stream_bytes = b"BT /F1 12 Tf 72 712 Td (Hello World) Tj ET";
    let mut parser = ContentParser::new(stream_bytes);

    for op in parser.parse()? {
        match op.operator() {
            Operator::BT => println!("Begin text block"),
            Operator::ET => println!("End text block"),
            Operator::Tj => println!("Show text: {:?}", op.operands()),
            Operator::TJ => println!("Show text with kerning: {:?}", op.operands()),
            _ => {}
        }
    }

    Ok(())
}
```

### Compacting Files with Object Streams

```rust
use oxpdf::Document;
use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read("input.pdf")?;
    let doc = Document::load(&data)?;

    let mut output = File::create("output_packed.pdf")?;
    let bytes_written = doc.write_packed(&mut output)?;
    println!("Wrote {} bytes of compacted PDF", bytes_written);

    Ok(())
}
```

---

## Satellite Crates & Tooling

The `oxpdf` repository contains three specialized sub-projects:

- **`oxpdf-cli`** ([`oxpdf-cli/`](oxpdf-cli)): A command-line utility for inspection, text extraction, file compaction, and batch benchmarking.
  ```bash
  cargo install --path oxpdf-cli
  oxpdf inspect input.pdf
  oxpdf extract-text input.pdf --page 1
  oxpdf pack input.pdf output.pdf
  oxpdf bench /path/to/pdf/corpus
  ```
- **`oxpdf-wasm`** ([`oxpdf-wasm/`](oxpdf-wasm)): Zero-copy WebAssembly bindings using `wasm-bindgen` for browser and edge runtime environments.
- **`fuzz`** ([`fuzz/`](fuzz)): Continuous fuzzing suite utilizing `cargo-fuzz` and `libfuzzer-sys` targeting the lexer, parser, cross-reference resolver, and stream decoders.

---

## Ecosystem Positioning: Typst & oxpdf

`oxpdf` and [Typst](https://github.com/typst/typst) address distinct, complementary operational requirements in the PDF ecosystem:

| Attribute | Typst | oxpdf |
|---|---|---|
| **Pipeline orientation** | **Source $\to$ PDF** | **PDF $\to$ AST / Plaintext / Packed PDF** |
| **Primary purpose** | Typesetting compiler and document authoring engine | Streaming parser, inspector, and fault reconstructor |
| **Input tolerance** | Requires valid Typst markup syntax | Ingests arbitrary, legacy, truncated, and corrupt PDF documents |
| **Memory model** | In-memory document tree and layout graph | Strictly bounded memory (<32 MB RAM) via streaming OS `mmap` |
| **Integration role** | Standards-compliant PDF generator | Ingestion, page analysis, and asset extraction engine |

---

## Security Model and Defenses

`oxpdf` includes defensive measures against hostile and malformed PDF files (`tests/security_audit.rs`):

- **Decompression limits**: Stream decoders enforce a 256 MB maximum output limit to neutralize decompression bomb attacks.
- **Bounded recursion**: Recursive descent parsers enforce strict depth ceilings (256 for objects, 64 for content streams, 32 for reference chains).
- **Cycle detection**: Object streams and page trees employ visited sets to reject cyclic reference loops with `Error::CyclicReference`.
- **Integer overflow guards**: Arithmetic calculations involving file offsets, object identifiers, and generation numbers use checked operations.

---

## Scope Exclusions

The following capabilities are explicitly outside the architectural scope of `oxpdf`:

1. **Pixel rendering**: Rasterization, font hinting, and antialiased graphical rendering belong in downstream display engines.
2. **JavaScript and dynamic forms**: Executing script engines and active forms calculation is not supported.
3. **Typesetting from scratch**: Layout calculation, line breaking, and typesetting are better served by authoring systems such as Typst.

---

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
