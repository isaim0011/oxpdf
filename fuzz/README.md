# Continuous Fuzzing Infrastructure for `oxpdf`

Automated, coverage-guided fuzz testing infrastructure for **`oxpdf`** using LLVM `libFuzzer` and [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz).

This fuzzing suite enforces the invariant safety caps, recursion limits, and panic-free error boundaries defined in **§1** and **§2** of the `oxpdf` v1.0 System Architecture Specification.

---

## 1. Fuzz Target Inventory

The fuzz targets reside in [`fuzz_targets/`](file:///fuzz_targets/) and isolate each layer of the $L_0$–$L_6$ processing pipeline:

| Target | Entrypoint Source | Layer | Invariant Safety Guarantees Verified |
|---|---|---|---|
| **`lexer`** | [`fuzz_targets/lexer.rs`](file:///fuzz_targets/lexer.rs) | $L_1$ Lexical Scanning | Runs `Lexer::new(data)` and exhausts `next_token()` until `None` or `Err`. Ensures `memchr` scanner never panics or loops infinitely on arbitrary, non-UTF8, or cyclic delimiter sequences (`<<`, `>>`, `[`, `]`, `()`, `<>`). |
| **`parser`** | [`fuzz_targets/parser.rs`](file:///fuzz_targets/parser.rs) | $L_3$ AST Parsing | Runs `Parser::new(data)` and repeatedly calls `parse_object()`. Enforces that the default depth ceiling (256) strictly catches adversarial nesting bombs (`[[[[...` and `<< << <<...`) and returns `Error::RecursionLimitExceeded` rather than stack-overflowing. |
| **`xref`** | [`fuzz_targets/xref.rs`](file:///fuzz_targets/xref.rs) | $L_4$ XRef & Repair | Runs both `Document::load_strict(data)` and `Document::load(data)`. Ensures `startxref` scanning, binary `/Type /XRef` stream decoding, and the fallback linear reconstruction pass (`recover.rs`) never panic on arbitrary byte mutations. |
| **`filter`** | [`fuzz_targets/filter.rs`](file:///fuzz_targets/filter.rs) | $L_5$ Stream Decoders | Exercises `FlateDecode` (zlib), `Ascii85Decode` (btoa), `AsciiHexDecode`, and multi-stage filter pipelines against adversarial payloads, enforcing the 256MB decompression ceiling (`StreamView::MAX_DECOMPRESS_BYTES`) to prevent zip-bomb OOM attacks. |

---

## 2. Prerequisites

`cargo-fuzz` requires a Rust toolchain with LLVM sanitizer instrumentation support.

### Installation
```bash
cargo install cargo-fuzz
```

> [!NOTE]
> Coverage-guided runtime execution via `cargo fuzz` requires a modern Clang/LLVM toolchain or MSVC on Windows.
> On Linux / macOS or Windows MSVC:
> ```bash
> rustup default nightly
> ```

---

## 3. Running the Fuzzers

### Execute a Fuzz Target
To start continuous coverage-guided fuzzing on a specific target:

```bash
# Fuzz the zero-copy token scanner
cargo fuzz run lexer

# Fuzz the recursive-descent AST parser
cargo fuzz run parser

# Fuzz document loading & linear reconstruction recovery
cargo fuzz run xref

# Fuzz stream decompression pipelines with 256MB bounds
cargo fuzz run filter
```

### Useful Fuzzing Options
```bash
# Run with a 10-second per-input timeout and 64KB max input size
cargo fuzz run xref -- -timeout=10 -max_len=65536

# Run multi-core parallel fuzzing across 8 worker processes
cargo fuzz run parser -- -jobs=8 -workers=8

# Run with address sanitizer (ASan) memory safety checks
cargo fuzz run lexer --sanitizer address
```

---

## 4. Corpus Management & Minimization

Seeded test inputs reside in [`corpus/<target>/`](file:///corpus/). When libFuzzer uncovers new coverage paths or crashes, artifacts are written to `fuzz/artifacts/` or added to the active corpus.

### Minimizing Corpus Size
To deduplicate and prune redundant inputs from the corpus while preserving 100% coverage:

```bash
cargo fuzz cmin lexer
cargo fuzz cmin parser
cargo fuzz cmin xref
cargo fuzz cmin filter
```

### Reproducing a Crash Artifact
If an artifact file `crash-0123456789abcdef` is generated:

```bash
cargo fuzz run <target> artifacts/<target>/crash-0123456789abcdef
```

---

## 5. Offline Seed & Sanity Verification Harness

To verify that all 4 fuzz targets compile cleanly and execute properly across adversarial edge cases without requiring a full libFuzzer runtime installation, run the integrated verification harness:

```bash
cargo run --example verify
```

Expected output:
```text
========================================================
oxpdf Continuous Fuzzing Verification Harness
========================================================
[1/4] Verifying 'lexer' fuzz target... OK (21 samples + 8KB noise)
[2/4] Verifying 'parser' fuzz target... OK (14 samples + recursion boundary tests)
[3/4] Verifying 'xref' fuzz target... OK (10 samples + 16KB random stream)
[4/4] Verifying 'filter' fuzz target... OK (13 samples)
--------------------------------------------------------
All 4 fuzz targets successfully verified in <10ms with 0 panics!
Continuous Fuzzing Infrastructure is READY.
========================================================
```
