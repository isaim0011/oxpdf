# oxpdf

[![crates.io](https://img.shields.io/crates/v/oxpdf.svg)](https://crates.io/crates/oxpdf)
[![docs.rs](https://docs.rs/oxpdf/badge.svg)](https://docs.rs/oxpdf)
[![CI](https://github.com/isaim0011/oxpdf/actions/workflows/ci.yml/badge.svg)](https://github.com/isaim0011/oxpdf/actions)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

`oxpdf` is a pure-Rust streaming PDF parser, indexer, and reconstructor. It is designed for high-throughput ingestion, strictly bounded memory usage, fault-tolerant reconstruction of corrupted files, and Unicode text extraction.

- **Streaming architecture**: Direct indexing of cross-reference tables without loading the entire document into an in-memory document tree.
- **Bounded memory**: Operates within a bounded physical memory envelope (<32 MB RAM) via memory-mapped I/O (`memmap2`) even when processing multi-gigabyte files.
- **Fault-tolerant recovery**: Linear forward reconstruction pass (modeled after PDFium) recovers objects and synthesized trailers when cross-reference tables are damaged or severed.
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

The following measurements reflect the complete 5,820-file reference corpus (319.75 MB total data) evaluated against `lopdf v0.36.0`:

| Metric | `oxpdf v1.0.1` | `lopdf v0.36.0` | Relative Difference |
|---|---|---|---|
| **Throughput** | **2,348.62 MB/s** | 23.71 MB/s | **99.04x faster** |
| **File processing rate** | **42,748.4 files/s** | 431.6 files/s | **99.04x faster** |
| **Median latency ($p_{50}$)** | **0.007 ms** (7 µs) | 0.227 ms (227 µs) | **32.91x lower latency** |
| **90th percentile latency ($p_{90}$)** | **0.030 ms** (30 µs) | 0.923 ms (923 µs) | **30.76x lower latency** |
| **99th percentile latency ($p_{99}$)** | **0.159 ms** (159 µs) | 6.968 ms (6,968 µs) | **43.96x lower latency** |
| **Net heap allocation (10.87 MB document)** | **108 KB** | 10,856 KB | **100.5x lower memory footprint** |
| **Corpus pass rate** | **99.93%** (5,816 / 5,820) | 99.55% (5,794 / 5,820) | **+22 files successfully parsed** |
| **Panics / unexpected aborts** | **0** | 0 | Both crates panic-free on corpus |

*Environment: Windows 10 x86_64, Intel Core i5-6200U @ 2.30 GHz, Rust 1.80+ release profile with Criterion 0.5. Full benchmark methodology and raw datasets are documented in [BENCHMARKS.md](BENCHMARKS.md).*

---

## Installation

Add `oxpdf` to your `Cargo.toml`:

```toml
[dependencies]
oxpdf = "1.0.1"
```

To use `oxpdf` in `no_std` environments (without memory-mapped file support):

```toml
[dependencies]
oxpdf = { version = "1.0.1", default-features = false }
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
4. **Decryption of password-protected files**: Password-protected PDFs return `Error::Unsupported("encrypted")`.

---

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
