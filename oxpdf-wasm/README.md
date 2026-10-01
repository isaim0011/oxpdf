# oxpdf-wasm

WebAssembly bindings for [`oxpdf`](https://crates.io/crates/oxpdf), providing high-throughput, memory-bounded PDF parsing, text extraction, and file compaction in browser and Node.js environments via `wasm-bindgen`.

## Building

Build the WebAssembly package using `wasm-pack`:

```bash
# Build for web / ES modules
wasm-pack build --target web --release

# Build for Node.js
wasm-pack build --target nodejs --release

# Build for bundlers (Webpack, Vite, Rollup)
wasm-pack build --target bundler --release
```

## Usage Example (JavaScript / TypeScript)

```typescript
import init, { WasmDocument } from './pkg/oxpdf_wasm.js';

async function run() {
  await init();

  // Fetch or load PDF bytes
  const response = await fetch('/document.pdf');
  const buffer = new Uint8Array(await response.arrayBuffer());

  // Instantiate document with zero-copy buffer view
  const doc = new WasmDocument(buffer);

  console.log(`Objects indexed: ${doc.object_count()}`);
  console.log(`Page count: ${doc.page_count()}`);

  // Extract text from all pages
  const pages: string[] = doc.extract_text_all();
  pages.forEach((text, index) => {
    console.log(`--- Page ${index + 1} ---\n${text}`);
  });

  // Compact document with object streams
  const packedBytes: Uint8Array = doc.pack();
  console.log(`Compacted PDF size: ${packedBytes.length} bytes`);
}

run();
```

## API Reference

### `WasmDocument`

- **`new WasmDocument(data: Uint8Array)`**: Parses cross-reference tables and initializes document indexing. Throws on unrecoverable syntax errors.
- **`page_count(): number`**: Returns the total number of pages in the resolved page tree.
- **`get_page_ids(): number[]`**: Returns an array of indirect object IDs corresponding to document pages.
- **`extract_text(page_id: number): string`**: Extracts Unicode plaintext from the specified page object.
- **`extract_text_all(): string[]`**: Extracts Unicode plaintext from all pages in natural document order.
- **`object_count(): number`**: Returns the total count of indexed indirect objects.
- **`pack(): Uint8Array`**: Packs non-stream objects into compressed `/ObjStm` containers and returns a compacted PDF 1.5+ byte buffer.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
