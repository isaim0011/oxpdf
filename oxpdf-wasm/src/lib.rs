use oxpdf::Document;
use wasm_bindgen::prelude::*;

fn to_js_error(err: oxpdf::Error) -> JsValue {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Error::new(&err.to_string()).into()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = err;
        JsValue::NULL
    }
}

/// WebAssembly-compatible wrapper for an `oxpdf` document.
#[wasm_bindgen]
pub struct WasmDocument {
    data: Vec<u8>,
}

#[wasm_bindgen]
impl WasmDocument {
    /// Constructs a new `WasmDocument` by validating and ingesting raw PDF byte data.
    #[wasm_bindgen(constructor)]
    pub fn new(data: &[u8]) -> Result<WasmDocument, JsValue> {
        // Validate document integrity / xref structure upon ingestion
        Document::load(data).map_err(to_js_error)?;
        Ok(WasmDocument {
            data: data.to_vec(),
        })
    }

    /// Returns the total resolved page count of the document.
    #[wasm_bindgen]
    pub fn page_count(&self) -> Result<usize, JsValue> {
        let doc = Document::load(&self.data).map_err(to_js_error)?;
        doc.page_count().map_err(to_js_error)
    }

    /// Returns the array of indirect object IDs corresponding to document pages.
    #[wasm_bindgen]
    pub fn get_page_ids(&self) -> Result<Vec<u32>, JsValue> {
        let doc = Document::load(&self.data).map_err(to_js_error)?;
        doc.get_page_ids().map_err(to_js_error)
    }

    /// Extracts decoded text from a single page specified by its object ID.
    #[wasm_bindgen]
    pub fn extract_text(&self, page_id: u32) -> Result<String, JsValue> {
        let doc = Document::load(&self.data).map_err(to_js_error)?;
        doc.extract_text(page_id).map_err(to_js_error)
    }

    /// Extracts decoded text from all pages in sequence, returning a JavaScript array of strings.
    #[wasm_bindgen]
    pub fn extract_text_all(&self) -> Result<js_sys::Array, JsValue> {
        let doc = Document::load(&self.data).map_err(to_js_error)?;
        let pages = doc.extract_text_all().map_err(to_js_error)?;
        let array = js_sys::Array::new();
        for page_text in pages {
            array.push(&JsValue::from_str(&page_text));
        }
        Ok(array)
    }

    /// Returns the total number of objects in the cross-reference table.
    #[wasm_bindgen]
    pub fn object_count(&self) -> usize {
        match Document::load(&self.data) {
            Ok(doc) => doc.object_count(),
            Err(_) => 0,
        }
    }

    /// Packs and compresses the document into an optimized object stream (`/ObjStm`),
    /// returning the serialized PDF bytes.
    #[wasm_bindgen]
    pub fn pack(&self) -> Result<Vec<u8>, JsValue> {
        let doc = Document::load(&self.data).map_err(to_js_error)?;
        let mut output = Vec::new();
        doc.write_packed(&mut output).map_err(to_js_error)?;
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PDF: &[u8] = b"%PDF-1.4\n\
1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
3 0 obj\n<< /Type /Page /Parent 2 0 R /Contents 4 0 R >>\nendobj\n\
4 0 obj\n<< /Length 48 >>\nstream\n\
BT /F1 12 Tf 72 712 Td (Hello WebAssembly) Tj ET\n\
endstream\nendobj\n\
trailer\n\
<< /Size 5 /Root 1 0 R >>\n\
startxref\n\
0\n\
%%EOF";

    #[test]
    fn test_wasm_document_lifecycle() {
        let doc = match WasmDocument::new(SAMPLE_PDF) {
            Ok(d) => d,
            Err(_) => panic!("Failed to instantiate WasmDocument"),
        };

        assert_eq!(doc.object_count(), 4);

        let page_count = match doc.page_count() {
            Ok(c) => c,
            Err(_) => panic!("Failed to get page_count"),
        };
        assert_eq!(page_count, 1);

        let page_ids = match doc.get_page_ids() {
            Ok(ids) => ids,
            Err(_) => panic!("Failed to get get_page_ids"),
        };
        assert_eq!(page_ids, vec![3]);

        let text = match doc.extract_text(3) {
            Ok(t) => t,
            Err(_) => panic!("Failed to extract_text"),
        };
        assert!(text.contains("Hello WebAssembly"));

        let packed_bytes = match doc.pack() {
            Ok(b) => b,
            Err(_) => panic!("Failed to pack document"),
        };
        assert!(!packed_bytes.is_empty());
        assert!(packed_bytes.starts_with(b"%PDF-1.5"));

        let packed_doc = match WasmDocument::new(&packed_bytes) {
            Ok(d) => d,
            Err(_) => panic!("Failed to load packed document"),
        };
        assert_eq!(packed_doc.page_count().unwrap(), 1);
    }

    #[test]
    fn test_wasm_document_invalid_pdf() {
        let invalid_pdf = b"NOT A VALID PDF DATA";
        assert!(WasmDocument::new(invalid_pdf).is_err());
    }
}
