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
| **OOM Safety** | ✅ Shipped | Hard caps on xref entry count, array/dict size, decompressed stream size (256 MB), page tree depth. Returns `Error::Unsupported` — never panics or OOM-crashes. |
| **Cross-Platform + WASM32** | ✅ Shipped | CI matrix: Linux, macOS, Windows (stable + beta) + `wasm32-unknown-unknown`. |
| **XRef Streams (PDF 1.5+ `/Type /XRef`)** | ⚠️ Partial | Falls back to linear scan — offsets recovered but less precise than stream parsing. |
| **Corpus Pass Rate** | 🔄 Measuring | veraPDF/Isartor harness in progress. Numbers will be published in v0.4.0. |

---

## 📦 Installation

```toml
[dependencies]
oxpdf = "0.3.0"
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

---

## 🚧 Known Limitations (v0.3.0)

These return `Error::Unsupported` — never a panic or OOM:

| Feature | Note |
|---|---|
| **Encryption** | `/Encrypt` dict not implemented. Encrypted content fails gracefully. |
| **JBIG2 / CCITTFax / LZW / RunLength filters** | Only `FlateDecode` and `Identity` implemented. Others return `Unsupported`. |
| **XRef Streams as sole xref** | Falls back to linear scan — works for most files, may miss some objects. |
| **Content stream parsing** | No text/graphic extraction (`BT`, `Tj`, `cm`, etc.) |
| **Digital signatures** | Not implemented. |
| **Font / image extraction** | Not implemented. |

---

## 📊 Benchmarks

```bash
cargo bench
```

Criterion harness in `benches/lexer_bench.rs`. Real corpus numbers (pass rate, p50/p99 load times) coming in v0.4.0.

---

## 📜 License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))
