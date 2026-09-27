# ⚡ oxpdf

[![crates.io](https://img.shields.io/crates/v/oxpdf.svg)](https://crates.io/crates/oxpdf)
[![docs.rs](https://docs.rs/oxpdf/badge.svg)](https://docs.rs/oxpdf)
[![CI](https://github.com/isaim0011/oxpdf/actions/workflows/ci.yml/badge.svg)](https://github.com/isaim0011/oxpdf/actions)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

**`oxpdf`** is a next-generation, pure-Rust streaming PDF engine engineered from scratch for extreme throughput, zero-copy parsing, SIMD acceleration, and bounded-memory multi-gigabyte document processing.

---

## 🎯 Architectural Mission & Pillars

1. **🚀 Zero-Copy Streaming Lexer**:
   Tokens borrow directly from memory-mapped slices or windowed buffers. Zero allocations for numeric tokens, keywords, and identifiers.
2. **🛡️ Memory Bounded Processing (<32MB RAM)**:
   Parse and inspect 500MB to 10GB+ PDF documents in bounded memory without loading full object trees into RAM.
3. **💥 Fault-Tolerant Reconstruction (Chromium PDFium style)**:
   Resilient against broken `/Prev` pointers, corrupt cross-reference streams, shifted offsets, and malformed trailer dictionaries.
4. **🔒 Stack-Safe Recursion Immunity**:
   Non-recursive loop-based object traversal immune to stack-overflow DoS attacks on deeply nested arrays or cyclic dictionaries.
5. **🌐 Cross-Platform & WASM32 Native**:
   First-class support for Linux, macOS, Windows, and WebAssembly (`wasm32-unknown-unknown` / OPFS environments).

---

## 📦 Installation

Add `oxpdf` to your `Cargo.toml`:

```toml
[dependencies]
oxpdf = "0.1.0"
```

---

## 🛠️ Quick Example: Streaming Lexer

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

at your option.
