# oxpdf v1.0.1 & Advanced Horizons Master Plan

## 1. Goal
Complete the Phase 5 advanced ecosystem features and prepare `oxpdf v1.0.1`:
1. `/ToUnicode` CMap font parser for glyph-to-Unicode mapping (CJK and subsetted fonts).
2. Stage B Portable SIMD Lexer (vectorized chunk classification under `feature = "simd"`).
3. `oxpdf-cli` satellite crate (`inspect`, `extract-text`, `pack`, `bench`).
4. `oxpdf-wasm` satellite crate (`WasmDocument` for browser and edge JS runtimes).
5. `cargo-fuzz` continuous fuzzing infrastructure (`lexer`, `parser`, `xref`, `filter` fuzz targets).
6. Typst architectural and benchmark comparison analysis in `BENCHMARKS.md` and `DESIGN.md`.
7. Full regression suite verification, version bump to `v1.0.1`, and release.

## 2. Architecture & File Layout
```
oxpdf/
  src/
    text.rs           # enhanced with CMap /ToUnicode stream parser
    cmap.rs           # /ToUnicode CMap state machine and lookup tables
    simd.rs           # Stage B chunked classification (SIMD / SWAR)
    lexer.rs          # wired to fast SIMD classifier when enabled
  benches/
    lexer_bench.rs    # updated with SIMD benchmark comparison
  fuzz/
    Cargo.toml        # libfuzzer-sys configuration
    fuzz_targets/
      lexer.rs
      parser.rs
      xref.rs
      filter.rs
oxpdf-cli/            # Satellite CLI binary
  Cargo.toml
  src/main.rs
oxpdf-wasm/           # Satellite WebAssembly library
  Cargo.toml
  src/lib.rs
```

## 3. Subagent Delegation Matrix
- **Agent 1 (CMap & Unicode)**: Implement `cmap.rs` and wire into `src/text.rs`.
- **Agent 2 (SIMD Lexer)**: Implement `src/simd.rs` and fast classification path.
- **Agent 3 (oxpdf-cli)**: Implement standalone binary crate in `oxpdf-cli/`.
- **Agent 4 (oxpdf-wasm)**: Implement WebAssembly bindings in `oxpdf-wasm/`.
- **Agent 5 (cargo-fuzz)**: Implement fuzz harness suite in `fuzz/`.
- **Agent 6 (Typst & Benchmark)**: Produce architectural and performance comparison against Typst.

## 4. Verification & Release Protocol
- 100% pass on all unit tests, security audit tests, corpus harness tests, and WASM compilation.
- Zero warnings under `cargo check` and `-D warnings`.
- Unified version bump to `1.0.1`.
- Changelog update and release tag.

## 5. Execution Status: 100% Complete (Shipped & Verified)
- [x] **Goal 1 (`/ToUnicode` CMap font parser)**: Implemented in `src/cmap.rs`, supporting `beginbfchar`, `beginbfrange`, surrogate pairs, and multi-codepoint ligatures; wired into `src/text.rs`.
- [x] **Goal 2 (Stage B Portable SIMD Lexer)**: Implemented in `src/simd.rs` with SWAR portable bitmasks and vectorized 16/32-byte chunk classifiers; wired into `src/lexer.rs`.
- [x] **Goal 3 (`oxpdf-cli` satellite crate)**: Implemented in `oxpdf-cli/` (`inspect`, `extract-text`, `pack`, `bench`), published live on [crates.io/crates/oxpdf-cli](https://crates.io/crates/oxpdf-cli/1.0.1).
- [x] **Goal 4 (`oxpdf-wasm` satellite crate)**: Implemented in `oxpdf-wasm/` (`WasmDocument`), tested, published live on [crates.io/crates/oxpdf-wasm](https://crates.io/crates/oxpdf-wasm/1.0.1).
- [x] **Goal 5 (`cargo-fuzz` infrastructure)**: Implemented in `fuzz/` with 4 targets (`lexer`, `parser`, `xref`, `filter`) and verified build harnesses.
- [x] **Goal 6 (Typst & Comparative Analysis)**: Comprehensive architectural and benchmark comparison published in `BENCHMARKS.md` and `DESIGN.md`.
- [x] **Goal 7 (Verification, Release & CI Green)**: Full regression suite passing, zero clippy warnings across all platforms, GitHub Actions CI 8/8 jobs green, `v1.0.1` tag and GitHub Release published.

