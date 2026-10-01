# oxpdf-cli

`oxpdf-cli` is the high-performance, lightweight command-line companion to the `oxpdf` engine.

## Installation

```bash
cargo build --release --manifest-path oxpdf-cli/Cargo.toml
```

## Commands

### Inspect
Inspects PDF version, object count, page count, trailer /Root ID, xref structure (table vs stream), and linear recovery status:
```bash
oxpdf inspect document.pdf
```

### Extract Text
Extracts Unicode plaintext from all pages or a specific page:
```bash
# All pages
oxpdf extract-text document.pdf

# Specific 1-based page
oxpdf extract-text document.pdf --page 1
```

### Pack
Compacts non-stream objects into compressed `/ObjStm` object streams (PDF 1.5+) and re-encodes cross-references as stream xrefs:
```bash
oxpdf pack input.pdf output_packed.pdf
```

### Bench
Recursively benchmarks parsing of all PDFs in a corpus directory, measuring total throughput (MB/s), p50/p99 parsing latencies, and categorizing errors:
```bash
oxpdf bench ./corpus
```
