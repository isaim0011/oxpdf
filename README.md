# ⚡ oxpdf

[![crates.io](https://img.shields.io/crates/v/oxpdf.svg)](https://crates.io/crates/oxpdf)
[![docs.rs](https://docs.rs/oxpdf/badge.svg)](https://docs.rs/oxpdf)
[![CI](https://github.com/isaim0011/oxpdf/actions/workflows/ci.yml/badge.svg)](https://github.com/isaim0011/oxpdf/actions)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

**`oxpdf`** is a high-throughput, pure-Rust streaming PDF engine engineered from scratch for zero-copy tokenization, bounded-memory random lookups, and fault-tolerant xref reconstruction.

---

## 🎯 Architectural Status & Honest Reality

| Pillar | Status | Implemented Reality |
|---|:---:|---|
| **🚀 Zero-Copy Streaming Lexer** | **[x] Shipped** | Borrows tokens directly off `&[u8]`; `SmallVec` inline allocations for strings; `memchr`-accelerated. |
| **🛡️ Memory-Bounded Processing** | **[x] Shipped** | `Document::load` builds xref indexes without materializing the object tree; on-demand random object lookups. |
| **💥 Fault-Tolerant Reconstruction** | **[x] Shipped** | Standard backward `startxref` resolver + Chromium PDFium-style forward linear fallback scanner for broken trailers and shifted offsets. |
| **🔒 Stack-Safe Recursion Immunity** | **[x] Shipped** | Loop-guarded worklist page tree flattening with visited set cycle detection; depth-bounded parser preventing stack-overflow DoS (`lopdf#502`). |
| **🌐 Cross-Platform & WASM32** | **[x] Shipped** | Tested across Linux, macOS, Windows (stable + beta), and WebAssembly (`wasm32-unknown-unknown`). |

*See [ROADMAP.md](ROADMAP.md) for full engineering milestones (Object Streams `/ObjStm`, content stream operator parser, and test corpora).*

---

## 📦 Installation

Add `oxpdf` to your `Cargo.toml`:

```toml
[dependencies]
oxpdf = "0.2.0"
```

---

## 🛠️ Usage Examples

### 1. Opening a Document & Inspecting Pages

```rust
use oxpdf::Document;

let pdf_bytes = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n...";
let doc = Document::load(pdf_bytes)?;

println!("Total objects in index: {}", doc.object_count());
println!("Total pages: {}", doc.page_count()?);

let page_ids = doc.get_page_ids()?;
for (idx, page_id) in page_ids.iter().enumerate() {
    println!("Page #{}: Object ID {}", idx + 1, page_id);
}
```

### 2. Zero-Copy Streaming Lexer

```rust
use oxpdf::lexer::{Lexer, Token};

let pdf_bytes = b"%PDF-1.7\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj";
let mut lexer = Lexer::new(pdf_bytes);

while let Some(token) = lexer.next_token()? {
    match token {
        Token::Name(name) => println!("Found Name: /{name}"),
        Token::Integer(val) => println!("Found Integer: {val}"),
        Token::Keyword(kw) => println!("Found Keyword: {kw}"),
        _ => {}
    }
}
```

---

## 📊 Benchmarks

Run benchmarks locally:

```bash
cargo bench
```

---

## 📜 License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
