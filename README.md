# ⚡ oxpdf

[![crates.io](https://img.shields.io/crates/v/oxpdf.svg)](https://crates.io/crates/oxpdf)
[![docs.rs](https://docs.rs/oxpdf/badge.svg)](https://docs.rs/oxpdf)
[![CI](https://github.com/isaim0011/oxpdf/actions/workflows/ci.yml/badge.svg)](https://github.com/isaim0011/oxpdf/actions)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

**`oxpdf`** is a high-performance, pure-Rust streaming PDF engine engineered for bounded memory, zero-copy tokenization, fault-tolerant reconstruction, content stream parsing, and Unicode text extraction.

- **Blazing Fast**: **2,348.62 MB/s** throughput (**99.04x faster than lopdf**).
- **Sub-Millisecond Latency**: **0.007 ms** median latency ($p_{50}$) across 5,820 files.
- **Strictly Bounded Memory**: Net heap delta of **108 KB** on a 10.87 MB document (**100.5x less memory than lopdf**).
- **Corpus-Hardened & Panic-Free**: **0 panics** across 5,820 real-world & adversarial PDFs.

For architectural specifications, see [DESIGN.md](DESIGN.md). For detailed empirical benchmarks, see [BENCHMARKS.md](BENCHMARKS.md).

---

## 🎯 Architecture & Implementation Status (v1.0.0)

| Feature | Status | Capability Details |
|---|:---:|---|
| **Zero-Copy Streaming Lexer** | ✅ Shipped | Borrows tokens directly from byte slices; `SmallVec<[u8; 32]>` inline buffers; `memchr`-accelerated scanning. |
| **Bounded-Memory Object Lookup** | ✅ Shipped | `Document::load` indexes xref and trailers only. Random `get_object(id)` on demand. |
| **Fault-Tolerant Xref Repair** | ✅ Shipped | PDFium-inspired forward linear reconstruction pass for broken, shifted, or severed xref tables. |
| **Stack-Safe Parser** | ✅ Shipped | Depth-bounded recursive descent (max 256). Cycle detection on page trees and object streams. |
| **Object Streams (`/ObjStm`)** | ✅ Shipped | PDF 1.5+ compressed object streams with interior index caching to prevent redundant decompression. |
| **Binary XRef Streams** | ✅ Shipped | Full PDF 1.5+ binary XRef stream parsing (`/W` widths, `/Index` ranges, types 0/1/2, `/Prev` chains). |
| **Content Stream Lexer & Ops** | ✅ Shipped | Dedicated `content::lexer` and `content::ops` parser for graphics, paths, matrices, and text operators. |
| **Unicode Text Extraction** | ✅ Shipped | Latin encodings (WinAnsi, Standard, MacRoman, PdfDoc), UTF-16 BOM, kerning displacements, spacing. |
| **Zero-Copy `PdfSource`** | ✅ Shipped | `MmapSource` (OS page cache offloading for 10GB+ files) and `BufferSource` for memory/WASM. |
| **QPDF-Style Object Packing** | ✅ Shipped | `Document::write_packed()` packs objects into `/ObjStm` + binary XRef stream (30–70% size reduction). |
| **Adversarial Security Hardening** | ✅ Shipped | Checked arithmetic, 256 MB decompression ceiling, recursion guards, bounds-safe slice indexing. |

---

## 📊 Head-to-Head Performance (5,820-File Benchmark)

Empirical benchmark comparing `oxpdf v1.0.0` against `lopdf v0.36.0` on the complete 5,820-file reference corpus (319.75 MB total data):

| Metric | `oxpdf v1.0.0` | `lopdf v0.36.0` | Advantage |
|---|---|---|---|
| **Throughput (MB/s)** | **2,348.62 MB/s** | 23.71 MB/s | **99.04x faster** |
| **Speed (files/sec)** | **42,748.4 /s** | 431.6 /s | **99.04x faster** |
| **Median Latency ($p_{50}$)** | **0.007 ms** (7 µs) | 0.227 ms (227 µs) | **32.91x lower latency** |
| **90th Percentile ($p_{90}$)** | **0.030 ms** (30 µs) | 0.923 ms (923 µs) | **30.76x lower latency** |
| **99th Percentile ($p_{99}$)** | **0.159 ms** (159 µs) | 6.968 ms (6,968 µs) | **43.96x lower latency** |
| **Net Heap Delta (10.87 MB File)** | **108 KB** (0.10 MB) | 10,856 KB (10.60 MB) | **100.5x less memory** |
| **Corpus Pass Rate** | **99.93%** (5,816 / 5,820) | 99.55% (5,794 / 5,820) | **+22 more valid passes** |
| **Panics / Crashes** | **0 (Zero)** | 0 (Zero) | **100% Robust** |

*Measured on Windows 10 x86_64, Intel Core i5-6200U @ 2.30 GHz with Criterion 0.5 and cargo release profile. See [BENCHMARKS.md](BENCHMARKS.md) for full reproduction instructions and Criterion charts.*

---

## 📦 Installation

Add `oxpdf` to your `Cargo.toml`:

```toml
[dependencies]
oxpdf = "1.0.0"
```

To enable memory-mapped file loading on native platforms (enabled by default):

```toml
[dependencies]
oxpdf = { version = "1.0.0", features = ["std"] }
```

---

## 🛠️ Usage Examples

### 1. Plaintext Text Extraction

```rust
use oxpdf::Document;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read("sample.pdf")?;
    let doc = Document::load(&data)?;

    // Extract text from all pages
    let pages_text = doc.extract_text_all()?;
    for (idx, text) in pages_text.iter().enumerate() {
        println!("--- Page {} ---\n{}", idx + 1, text);
    }

    // Or extract a single page by object ID
    let page_ids = doc.get_page_ids()?;
    if let Some(&first_page) = page_ids.first() {
        let single_page_text = doc.extract_text(first_page)?;
        println!("First page content:\n{}", single_page_text);
    }

    Ok(())
}
```

### 2. Inspecting Pages & Object Graph

```rust
use oxpdf::Document;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read("document.pdf")?;
    let doc = Document::load(&data)?;

    println!("Objects indexed: {}", doc.object_count());
    println!("Total pages: {}", doc.page_count()?);

    for (i, page_id) in doc.get_page_ids()?.iter().enumerate() {
        println!("Page {}: Object ID {}", i + 1, page_id);
    }

    // Random lookup of any object without loading the rest of the document
    if let Some(obj) = doc.get_object(1)? {
        println!("Catalog root: {:?}", obj);
    }

    Ok(())
}
```

### 3. Parsing Content Stream Operators Directly

```rust
use oxpdf::content::{ContentParser, Operator};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stream_bytes = b"BT /F1 12 Tf 72 712 Td (Hello World) Tj ET";
    let mut parser = ContentParser::new(stream_bytes);

    for op in parser.parse()? {
        match op.operator {
            Operator::BeginText => println!("Text block begins"),
            Operator::EndText   => println!("Text block ends"),
            Operator::ShowText  => println!("Text operands: {:?}", op.operands),
            _ => {}
        }
    }

    Ok(())
}
```

### 4. QPDF-Style Object Stream Packing

```rust
use oxpdf::Document;
use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read("bloated.pdf")?;
    let doc = Document::load(&data)?;

    // Compacts non-stream objects into FlateDecode /ObjStm and outputs PDF 1.5+ XRef stream
    let mut output = File::create("optimized.pdf")?;
    let bytes_written = doc.write_packed(&mut output)?;
    println!("Wrote {} bytes of optimized PDF", bytes_written);

    Ok(())
}
```

---

## 🔒 Security & Adversarial Hardening

`oxpdf` has undergone a comprehensive, hostile audit (`tests/security_audit.rs`):
- **OOM Protection**: Hard 256 MB decompression ceiling on Flate, Ascii85, and AsciiHex filters prevents decompression bomb attacks.
- **Arithmetic Safety**: `checked_add` and strict integer ranges (`u32::MAX`, `u16::MAX`) on object IDs, generation counters, byte offsets, and octal shifts.
- **Recursion Limits**: Fixed ceilings on parser descent (256), content stream nesting (64), and indirect reference traversal (32).
- **Cycle Immunity**: Object stream graph cycles and circular page trees are detected and rejected with `Error::CyclicReference` instead of infinite loops or stack overflow.

---

## 🚫 What oxpdf Will Never Do (Permanent Exclusions)

Per §9 of the [System Design Specification](DESIGN.md), `oxpdf` maintains strict architectural boundaries:
1. **Rendering to Pixels**: `oxpdf` is a parser, indexer, and structural engine. Rasterization, raster font rendering, and anti-aliased compositing belong in downstream renderers.
2. **JavaScript & Forms Execution**: Active content, dynamic calculations, and form submission protocols are out of scope.
3. **Creation from Scratch**: Generating layouts, calculating typography, or typesetting arbitrary visual pages is better handled by engines like Typst.
4. **Encrypted PDF Decryption**: Password-protected PDFs are detected early and return `Error::Unsupported("encrypted")`.

---

## 📜 License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))
