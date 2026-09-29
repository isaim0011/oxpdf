# ⚡ oxpdf

[![crates.io](https://img.shields.io/crates/v/oxpdf.svg)](https://crates.io/crates/oxpdf)
[![docs.rs](https://docs.rs/oxpdf/badge.svg)](https://docs.rs/oxpdf)
[![CI](https://github.com/isaim0011/oxpdf/actions/workflows/ci.yml/badge.svg)](https://github.com/isaim0011/oxpdf/actions)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

**`oxpdf`** is a pure-Rust streaming PDF parser engineered for zero-copy tokenization, bounded-memory object lookup, and fault-tolerant xref reconstruction. No unsafe allocations, no panic on corrupt input, no OOM on adversarial PDFs.

---

## 🎯 What Is Actually Implemented (Honest Status)

| Feature | Status | Reality |
|---|:---:|---|
| **Zero-Copy Streaming Lexer** | ✅ Shipped | Borrows tokens directly from `&[u8]`; `SmallVec<[u8; 32]>` inline strings; `memchr`-accelerated scanning. |
| **Bounded-Memory Object Lookup** | ✅ Shipped | `Document::load` indexes xref only — no full object tree materialized. Random `get_object(id)` by offset. |
| **Fault-Tolerant Xref** | ✅ Shipped | Backward `startxref` scanner + PDFium-style forward linear fallback for broken/shifted trailers. |
| **Stack-Safe Parser** | ✅ Shipped | Depth-bounded recursive descent (max 256). Loop-guarded page tree with cycle detection. |
| **Object Streams `/ObjStm`** | ✅ Shipped | PDF 1.5+ compressed object streams, `FlateDecode` only, with `RefCell` cache to avoid double-decompression. |
| **XRef Streams (PDF 1.5+)** | ✅ Shipped | Full binary XRef stream parsing: `/W` field widths, `/Index` ranges, type-0/1/2 entries, `/Prev` chain, FlateDecode. |
| **OOM Safety** | ✅ Shipped | Hard caps on xref entry count, array/dict size, decompressed stream size (256 MB), page tree depth. Returns `Error::Unsupported` — never panics or OOM-crashes. |
| **Encryption Detection** | ✅ Shipped | Trailer-region scan + trailer dict check. Returns `Error::Unsupported("encrypted")` immediately — never hangs. |
| **QPDF-Style Object Packing** | ✅ Shipped | `Document::write_packed()` groups non-stream objects into `FlateDecode /ObjStm`, outputs PDF 1.5+ XRef stream. 30–70% smaller than `write_to()`. |
| **Cross-Platform + WASM32** | ✅ Shipped | CI matrix: Linux, macOS, Windows (stable + beta) + `wasm32-unknown-unknown`. |
| **Corpus Pass Rate** | ✅ **100%** | **2906/2906** veraPDF + Isartor files. p50 = **0.05 ms**, p99 = **1.99 ms**. Measured on `v0.4.0`. |

---

## 📦 Installation

```toml
[dependencies]
oxpdf = "0.4.0"
```

---

## 🛠️ Usage

### Open a PDF and inspect pages

```rust
use oxpdf::Document;

let data = std::fs::read("document.pdf")?;
let doc = Document::load(&data)?;

println!("Objects in index: {}", doc.object_count());
println!("Pages: {}", doc.page_count()?);

for (i, page_id) in doc.get_page_ids()?.iter().enumerate() {
    println!("  Page {}: object id {}", i + 1, page_id);
}
```

### Zero-copy streaming lexer

```rust
use oxpdf::{Lexer, Token};

let bytes = b"%PDF-1.7\n1 0 obj\n<< /Type /Catalog >>\nendobj";
let mut lexer = Lexer::new(bytes);

while let Some(tok) = lexer.next_token()? {
    match tok {
        Token::Name(n)    => println!("/{n}"),
        Token::Integer(i) => println!("{i}"),
        Token::Keyword(k) => println!("{k}"),
        _ => {}
    }
}
```

### Fetch a specific object

```rust
use oxpdf::{Document, Object};

let data = std::fs::read("document.pdf")?;
let doc = Document::load(&data)?;

// Returns Object<'static> — no lifetime ties to `data`
if let Some(obj) = doc.get_object(1)? {
    println!("{obj:?}");
}
```

### Write a size-optimized PDF (QPDF-style packing)

```rust
use oxpdf::Document;

let data = std::fs::read("input.pdf")?;
let doc = Document::load(&data)?;

// Packs non-stream objects into FlateDecode /ObjStm — 30–70% smaller
let mut out = std::fs::File::create("packed.pdf")?;
doc.write_packed(&mut out)?;
```

---

## 📊 Real Corpus Numbers (v0.4.0)

Measured against the full **veraPDF + Isartor** test corpus (2906 PDFs) on Windows/x86-64 release build:

```
Total files:  2906
Pass rate:    100.00%  (2906 / 2906)
Failures:     0
p50 load:     0.05 ms
p99 load:     1.99 ms
Peak RSS:     < 10 MB
```

Run it yourself:

```bash
git clone https://github.com/nicowillis/verapdf-regression-corpus corpus/verapdf_repo
cargo build --example corpus_runner --release
./target/release/examples/corpus_runner corpus/verapdf_repo
```

---

## 🚧 Known Limitations (v0.4.0)

These return `Error::Unsupported` — never a panic or OOM:

| Feature | Note |
|---|---|
| **Encryption** | `/Encrypt` dict: detected and rejected early with a clear error. Not decryptable. |
| **JBIG2 / CCITTFax / LZW / RunLength filters** | Only `FlateDecode` and `Identity` implemented. Others return `Unsupported`. |
| **Content stream parsing** | No text/graphic extraction (`BT`, `Tj`, `cm`, etc.) |
| **Digital signatures** | Not implemented. |
| **Font / image extraction** | Not implemented. |

---

## 📜 License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))
